use anyhow::Error;
use clap::{Parser, Subcommand};
use clickhouse::Client;
use futures03::future::join_all;
use futures03::StreamExt;
use hyper_rustls::HttpsConnectorBuilder;
use loader::Cursor;
use pb::sf::substreams::v1::Package;
use url::Url;
use log::{info, error};

use prost::Message;
use std::collections::VecDeque;
use std::fs;
use std::{collections::HashMap, env, process::exit, sync::Arc, time::Duration};
use substreams::SubstreamsEndpoint;
use substreams_database_change::pb::database::Field;
use substreams_stream::{BlockResponse, SubstreamsStream};
use thiserror::Error;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

use crate::loader::DatabaseLoader;
use crate::table_info::{get_columns, get_table_information, DynamicTable};

mod fixed_string;
mod loader;
mod pb;
mod substreams;
mod substreams_stream;
mod table_info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Run {
        database_url: Url,
        id: String,
        #[arg(long, default_value = "substreams.spkg")]
        package_file: String,
        #[arg(long, default_value = "db_out")]
        module: String,
        #[arg(
            long,
            short,
            default_value = "https://mainnet.eth.streamingfast.io:443"
        )]
        endpoint_url: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "0")]
        start_block: i64,
        #[arg(long, default_value = "0")]
        end_block: u64,
    },
    Setup {
        database_url: Url,
        file_name: String,
    },
}

#[derive(Error, Debug)]
pub enum ElricError {
    #[error("Token not found")]
    TokenNotFound,
    #[error("Could not read package file")]
    PackageFileError(#[from] std::io::Error),
    #[error("Could not decode package")]
    PackageDecodeError(#[from] prost::DecodeError),
    #[error("Could not load cursor")]
    CursorError,
    #[error("Could not load schema")]
    LoadSchemaError,
    #[error("Could not insert cursor")]
    InsertCursorError,
    #[error("Could not insert row")]
    InsertRowError,
    #[error("Could not commit transaction")]
    CommitError,
    #[error("Could not find columns for database {0} table {1}")]
    ColumnNotFound(String, String),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Setup {
            database_url,
            file_name,
        } => {
            let client = load_database(database_url);
            setup_schema(&client, file_name).await?;
            info!("Schema setup complete");
        }
        Commands::Run {
            id,
            database_url,
            package_file,
            module: module_name,
            endpoint_url,
            token,
            start_block,
            end_block,
        } => {
            let client = load_database(database_url);
            let token = match env::var("SUBSTREAMS_API_TOKEN").ok() {
                Some(token) => token,
                None => token.ok_or(ElricError::TokenNotFound)?,
            };
            run(
                client,
                id,
                package_file,
                module_name,
                endpoint_url,
                token,
                start_block,
                end_block,
            )
            .await?;
        }
    }
    Ok(())
}

async fn run(
    client: clickhouse::Client,
    id: String,
    package_file: String,
    module: String,
    endpoint_url: String,
    token: String,
    start_block: i64,
    end_block: u64,
) -> Result<(), ElricError> {
    let package = read_package(&package_file)?;
    let endpoint = Arc::new(SubstreamsEndpoint::new(&endpoint_url, Some(token)));

    let cursor: Option<String> = load_persisted_cursor(&client, &id).await.map_err(|_| ElricError::CursorError)?;

    let mut stream = SubstreamsStream::new(
        endpoint.clone(),
        cursor,
        package.modules.clone(),
        module,
        start_block,
        end_block,
    );

    let table_info = get_table_information(&client).await?;

    let dynamic_tables = table_info
        .iter()
        .map(|table| async {
            let mut columns = get_columns(&client, &table.table_schema, &table.table_name)
                .await?;
            columns.sort();
            Ok(DynamicTable::new(&table.table_name, columns))
        })
        .collect::<Vec<_>>();
    let dynamic_tables = join_all(dynamic_tables).await.into_iter().collect::<Result<Vec<_>, ElricError>>()?;

    let mut loader = DatabaseLoader::new(id, client, dynamic_tables);

    let (stop_tx, mut stop_rx) = watch::channel(());

    tokio::spawn(async move {
        let mut sigterm = signal(SignalKind::terminate()).unwrap();
        let mut sigint = signal(SignalKind::interrupt()).unwrap();
        loop {
            select! {
                _ = sigterm.recv() => {},
                _ = sigint.recv() => {},
            };
            stop_tx.send(()).unwrap();
        }
    });

    loop {
        select! {
            biased;

            _ = stop_rx.changed() => break,
            stream_response = stream.next() => match stream_response {
                None => {
                    info!("Stream consumed");
                    break;
                }
                Some(Ok(BlockResponse::New(data))) => {
                    loader.process_block_scoped_data(data).await?;
                }
                Some(Ok(BlockResponse::Undo(undo_signal))) => {
                    let block_num_signal = undo_signal.last_valid_block.as_ref().unwrap().number;
                    loader.process_block_undo_signal(block_num_signal);
                }
                Some(Err(err)) => {
                    error!("Stream terminated with error");
                    error!("{:?}", err);
                    exit(1);
                }
            },
        }
    }

    info!("Gracefully shutting down...");
    loader.end().await;
    Ok(())
}

fn load_database(database_url: Url) -> Client {
    let username = database_url.username();
    let password = database_url.password().unwrap_or("");
    let database = database_url
        .path_segments()
        .map(|c| c.filter(|s| !s.is_empty()).collect::<VecDeque<_>>())
        .map(|mut v| v.pop_front().unwrap_or("default"))
        .unwrap_or("default");
    let url = format!(
        "{}://{}{}",
        database_url.scheme(),
        database_url.host_str().unwrap_or(""),
        database_url
            .port_or_known_default()
            .map(|p| format!(":{}", p))
            .unwrap_or("".to_string())
    );

    const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(2);


    let https = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();

    let client = hyper::Client::builder()
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
        .build::<_, hyper::Body>(https);

    let mut client = Client::with_http_client(client)
        .with_url(url)
        .with_user(username)
        .with_password(password)
        .with_database(database);
    for query in database_url.query_pairs() {
        client = client.with_option(query.0, query.1);
    }
    client
}

async fn setup_schema(client: &Client, file: String) -> Result<(), Error> {
    let schema = fs::read_to_string(file)?;
    for stmt in schema.split(";") {
        let stmt = stmt.trim();
        if !stmt.is_empty() {
            client.query(stmt).execute().await?;
        }
    }
    // client.query(&schema).execute().await?;
    Ok(())
}

fn convert_field_to_hash(fields: Vec<Field>) -> HashMap<String, String> {
    let mut field_map: HashMap<String, String> = HashMap::new();
    for field in fields {
        field_map.insert(field.name, field.new_value);
    }
    field_map
}

async fn load_persisted_cursor(
    client: &clickhouse::Client,
    id: &str,
) -> Result<Option<String>, anyhow::Error> {
    let cursor = client.query(&format!(
        "SELECT * FROM cursors WHERE id = '{}' ORDER BY block_num DESC",
        id
    ));
    let cursor = cursor.fetch_optional::<Cursor>().await?;

    Ok(cursor.map(|c| c.cursor().clone()))
}

fn read_package(file: &str) -> Result<Package, ElricError> {
    let content = std::fs::read(file)?;
    Ok(Package::decode(content.as_ref())?)
}

#[cfg(test)]
mod tests {

    use clickhouse::Row;
    use serde::Serialize;

    // use super::*;

    #[derive(Row, Serialize)]
    struct Test {
        contract: String,
    }

    // #[test]
    // fn check_encoders() -> Result<()> {
    //     let mut buffer = BytesMut::new();
    //     let column_info = vec![
    //         ColumnInfo {
    //             column_name: "contract_address".into(),
    //             data_type: ColumnType::FixedString(40),
    //         },
    //         ColumnInfo {
    //             column_name: "evt_tx_hash".into(),
    //             data_type: ColumnType::String,
    //         },
    //         ColumnInfo {
    //             column_name: "evt_index".into(),
    //             data_type: ColumnType::UInt32,
    //         },
    //         ColumnInfo {
    //             column_name: "evt_block_time".into(),
    //             data_type: ColumnType::DateTime,
    //         },
    //         ColumnInfo {
    //             column_name: "evt_block_number".into(),
    //             data_type: ColumnType::UInt32,
    //         },
    //         ColumnInfo {
    //             column_name: "from".into(),
    //             data_type: ColumnType::FixedString(40),
    //         },
    //         ColumnInfo {
    //             column_name: "to".into(),
    //             data_type: ColumnType::FixedString(40),
    //         },
    //         ColumnInfo {
    //             column_name: "value".into(),
    //             data_type: ColumnType::UInt256,
    //         },
    //     ];
    //     let date = "2023-08-04T13:53:29+00:00";
    //
    //     let mut data: HashMap<String, String> = HashMap::new();
    //     data.insert("contract_address".into(), "asdfasdfasdf".into());
    //     data.insert("evt_index".into(), "5".into());
    //     data.insert("evt_block_time".into(), date.into());
    //     data.insert("evt_block_number".into(), "1".into());
    //     data.insert("evt_tx_hash".into(), "asdfasdfasdf".into());
    //     data.insert("from".into(), "asdfasdfasdf".into());
    //     data.insert("to".into(), "asdfasdfasdf".into());
    //     data.insert("value".into(), "100".into());
    //     // let test = DynamicInsert::new(column_info, data);
    //     // ser::serialize_into(&mut buffer, &test)?;
    //     let mut buffer2 = BytesMut::new();
    //
    //     let date = chrono::DateTime::parse_from_rfc3339(date)
    //         .context("evt_block_time")?
    //         .timestamp();
    //
    //     let test = TransferEvent {
    //         contract_address: "asdfasdfasdf".into(),
    //         evt_block_time: date as i32,
    //         evt_index: 5,
    //         evt_block_number: 1,
    //         evt_tx_hash: "asdfasdfasdf".into(),
    //         from: "asdfasdfasdf".into(),
    //         to: "asdfasdfasdf".into(),
    //         value: U256::from_dec_str("100").unwrap(),
    //     };
    //     // ser::serialize_into(&mut buffer2, &test)?;
    //
    //     assert_eq!(buffer, buffer2);
    //     Ok(())
    // }
}

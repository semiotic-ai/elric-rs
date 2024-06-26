use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use clickhouse::{
    inserter::{Inserter, RowInserter, SchemaInserter},
    Client, Row,
};
use tracing::{debug, info, warn};
use prost::Message;
use serde::{Deserialize, Serialize};
use substreams_database_change::pb::database::{
    table_change::PrimaryKey, CompositePrimaryKey, DatabaseChanges, TableChange,
};

use crate::{
    convert_field_to_hash,
    pb::sf::substreams::rpc::v2::BlockScopedData,
    table_info::{DynamicInsert, DynamicTable},
    ElricError,
};

const BUFFER_LEN: usize = 12;

pub struct DatabaseLoader {
    id: String,
    tables: HashMap<String, DynamicTable>,
    inserters: HashMap<String, Inserter<SchemaInserter<DynamicTable>, DynamicTable>>,
    cursor: Inserter<RowInserter<Cursor>, Cursor>,
    buffer: VecDeque<BlockScopedData>,
}

#[derive(Debug, Row, Serialize, Deserialize)]
pub struct Cursor {
    id: String,
    cursor: String,
    block_num: u64,
    block_id: String,
}

impl Cursor {
    pub fn cursor(&self) -> &String {
        &self.cursor
    }
}

impl DatabaseLoader {
    pub fn new(id: String, client: Client, table: Vec<DynamicTable>) -> Self {
        let mut inserters = HashMap::new();

        table.iter().for_each(|table| {
            let table_name = table.table_name.clone();
            let inserter = client
                .inserter_with_schema(&table_name, table.clone())
                .expect("inserter")
                .with_timeouts(Some(Duration::from_secs(5)), Some(Duration::from_secs(20)))
                .with_period(Some(Duration::from_secs(15)));
            inserters.insert(table_name, inserter);
        });

        let tables = table
            .into_iter()
            .map(|t| (t.table_name.clone(), t))
            .collect();

        let cursor = client
            .inserter("cursors")
            .expect("error while creating cursors inserter")
            .with_timeouts(Some(Duration::from_secs(5)), Some(Duration::from_secs(20)))
            .with_period(Some(Duration::from_secs(15)));

        Self {
            id,
            tables,
            inserters,
            cursor,
            buffer: VecDeque::new(),
        }
    }

    fn get_final_blocks_from_buffer(&mut self, data: BlockScopedData) -> Vec<BlockScopedData> {
        let mut final_blocks = vec![];

        let final_block_index = self
            .buffer
            .iter()
            .rev()
            .position(|b| b.clock.as_ref().unwrap().number <= data.final_block_height)
            .map(|i| self.buffer.len() - i - 1);

        let is_full_capacity = self.buffer.len() >= BUFFER_LEN;

        if is_full_capacity || final_block_index.is_some() {
            let len = match final_block_index {
                Some(i) => i,
                None => self.buffer.len() - BUFFER_LEN,
            };

            final_blocks.extend(self.buffer.drain(0..=len));
        }

        if data.clock.as_ref().unwrap().number <= data.final_block_height {
            final_blocks.push(data);
        } else {
            self.buffer.push_back(data);
        }
        final_blocks
    }

    pub async fn process_block_scoped_data(
        &mut self,
        data: BlockScopedData,
    ) -> Result<(), ElricError> {
        for block in self.get_final_blocks_from_buffer(data) {
            let block_num = block.clock.as_ref().unwrap().number;
            let block_id = block.clock.as_ref().unwrap().id.clone();
            let cursor = block.cursor.clone();
            self.process_final_blocks(block).await?;
            self.persist_cursor(cursor, block_num, block_id)
                .await
                .map_err(|_| ElricError::InsertCursorError)?;
        }
        Ok(())
    }

    async fn process_final_blocks(&mut self, data: BlockScopedData) -> Result<(), ElricError> {
        let output = data.output.as_ref().unwrap().map_output.as_ref().unwrap();
        let database_changes = DatabaseChanges::decode(output.value.as_slice())?;
        let changes_length = database_changes.table_changes.len();

        let splitted_inserts = split_table_changes(database_changes.table_changes);

        for (table, changes) in splitted_inserts {
            let table_info = self
                .get_table_info(&table)
                .unwrap_or_else(|| panic!("It was not possible to find the table {}", table))
                .clone();
            let inserter = self.get_table_inserter(&table).unwrap();
            for change in changes {
                let mut fields = convert_field_to_hash(change.fields);

                match change.primary_key {
                    Some(PrimaryKey::CompositePk(CompositePrimaryKey { keys })) => {
                        fields.extend(keys);
                    }
                    Some(PrimaryKey::Pk(_)) => {}
                    None => {}
                };
                let dynamic_insert = DynamicInsert::new(table_info.clone(), fields);

                inserter
                    .write(&dynamic_insert)
                    .await
                    .map_err(|_| ElricError::InsertRowError)?;
            }

            inserter
                .commit()
                .await
                .map_err(|_| ElricError::CommitError)?;
        }

        let block_num = data.clock.as_ref().unwrap().number;
        info!(
            block_num,
            changes_length,
            "Block #{} - Payload {} ({} bytes)",
            block_num,
            output.type_url.replace("type.googleapis.com/", ""),
            output.value.len()
        );

        Ok(())
    }

    pub fn process_block_undo_signal(&mut self, block_num_signal: u64) {
        warn!(undo_block_num = block_num_signal, "Processing undo signal for block {}", block_num_signal);
        let final_block_index = self
            .buffer
            .iter()
            .rev()
            .position(|b| block_num_signal == b.clock.as_ref().unwrap().number)
            .map(|i| self.buffer.len() - i);

        if let Some(index) = final_block_index {
            let drained = self.buffer.drain(index..);
            for d in drained {
                let block_num = d.clock.as_ref().unwrap().number;
                debug!(block_num, ?d, "New block drained");
            }
        }
    }

    pub async fn persist_cursor(
        &mut self,
        cursor: String,
        block_num: u64,
        block_id: String,
    ) -> Result<(), anyhow::Error> {
        let cursor = Cursor {
            id: self.id.clone(),
            cursor,
            block_num,
            block_id,
        };
        self.cursor.write(&cursor).await?;
        self.cursor.commit().await?;
        Ok(())
    }

    fn get_table_inserter(
        &mut self,
        table_name: &str,
    ) -> Option<&mut Inserter<SchemaInserter<DynamicTable>, DynamicTable>> {
        self.inserters.get_mut(table_name)
    }

    fn get_table_info(&mut self, table_name: &str) -> Option<&DynamicTable> {
        self.tables.get(table_name)
    }

    pub async fn end(self) {
        for (_, inserter) in self.inserters {
            inserter.end().await.expect("end");
        }
        self.cursor.end().await.expect("cursor end");
    }
}

fn split_table_changes(table_changes: Vec<TableChange>) -> HashMap<String, Vec<TableChange>> {
    let mut table_map: HashMap<String, Vec<TableChange>> =
        HashMap::with_capacity(table_changes.len());

    for change in table_changes {
        // Check if the table name exists in the HashMap
        if let Some(changes_for_table) = table_map.get_mut(&change.table) {
            // If the table name exists, add the TableChange to its vector
            changes_for_table.push(change);
        } else {
            // If the table name does not exist, create a new vector and add the TableChange to it
            table_map.insert(change.table.clone(), vec![change]);
        }
    }

    table_map
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};

    use clickhouse::{test, Client, Row};
    use prost::Message;
    use prost_types::Any;
    use serde::Deserialize;
    use substreams_database_change::pb::database::{DatabaseChanges, Field, TableChange};
    use tracing_test::traced_test;

    use crate::{
        loader::BUFFER_LEN,
        pb::sf::substreams::{
            rpc::v2::{BlockScopedData, MapModuleOutput},
            v1::Clock,
        },
        table_info::{ColumnInfo, ColumnType, DynamicTable},
    };

    use super::DatabaseLoader;
    use anyhow::Result;

    #[tokio::test]
    async fn test_undo_block_signal() {
        let mut buffer = VecDeque::new();
        for i in 0..BUFFER_LEN {
            buffer.push_back(BlockScopedData {
                clock: Some(Clock {
                    number: i as u64,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        let mock = test::Mock::new();
        let client = Client::default().with_url(mock.url());
        let cursor = client.inserter("test").unwrap();
        let mut loader = DatabaseLoader {
            id: "test".into(),
            buffer,
            tables: HashMap::new(),
            inserters: HashMap::new(),
            cursor,
        };
        let v = 8;
        loader.process_block_undo_signal(v);
        let result = loader
            .buffer
            .iter()
            .map(|b| b.clock.as_ref().unwrap().number)
            .collect::<Vec<_>>();
        assert_eq!(result, (0..=v).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn test_buffer() {
        let mock = test::Mock::new();
        let client = Client::default().with_url(mock.url());
        let cursor = client.inserter("test").unwrap();
        let mut loader = DatabaseLoader {
            id: "test".into(),
            buffer: VecDeque::new(),
            tables: HashMap::new(),
            inserters: HashMap::new(),
            cursor,
        };
        for i in 0..10 {
            let data = BlockScopedData {
                clock: Some(Clock {
                    number: i as u64,
                    ..Default::default()
                }),
                final_block_height: 10,
                ..Default::default()
            };
            let final_blocks = loader.get_final_blocks_from_buffer(data);
            assert_eq!(final_blocks.len(), 1);
        }
        for i in 0..BUFFER_LEN {
            let data = BlockScopedData {
                clock: Some(Clock {
                    number: (i + 1) as u64,
                    ..Default::default()
                }),
                final_block_height: 0,
                ..Default::default()
            };
            let final_blocks = loader.get_final_blocks_from_buffer(data);
            assert_eq!(final_blocks.len(), 0);
        }
        let data = BlockScopedData {
            clock: Some(Clock {
                number: (BUFFER_LEN + 2) as u64,
                ..Default::default()
            }),
            final_block_height: 0,
            ..Default::default()
        };
        let final_blocks = loader.get_final_blocks_from_buffer(data);
        assert_eq!(final_blocks.len(), 1);
    }

    #[derive(Row, Debug, Deserialize, PartialEq)]
    #[allow(dead_code)]
    struct TestInsert {
        test: u64,
    }

    #[tokio::test]
    #[traced_test]
    async fn test_process_data() -> Result<()> {
        let mut mock = test::Mock::new();
        mock.non_exhaustive();
        let client = Client::default().with_url(mock.url());
        let table = vec![DynamicTable::new(
            "test",
            vec![ColumnInfo {
                column_name: "test".into(),
                data_type: ColumnType::UInt64,
            }],
        )];
        let mut loader = DatabaseLoader::new("test".into(), client, table);
        let changes = vec![
            TableChange {
                table: "test".into(),
                fields: vec![Field {
                    name: "test".into(),
                    old_value: "0".into(),
                    new_value: "1".into(),
                }],
                ..Default::default()
            },
            TableChange {
                table: "test".into(),
                fields: vec![Field {
                    name: "test".into(),
                    old_value: "0".into(),
                    new_value: "2".into(),
                }],
                ..Default::default()
            },
        ];
        let data = create_block_scoped_data(changes);
        let inserts_recording = mock.add(test::handlers::record());
        loader.process_final_blocks(data).await?;
        loader.end().await;
        let inserts: Vec<TestInsert> = inserts_recording.collect().await;
        assert_eq!(
            inserts,
            vec![TestInsert { test: 1 }, TestInsert { test: 2 }]
        );
        Ok(())
    }

    fn create_block_scoped_data(table_changes: Vec<TableChange>) -> BlockScopedData {
        let mut buffer = vec![];
        let _ = DatabaseChanges { table_changes }.encode(&mut buffer);
        BlockScopedData {
            output: Some(MapModuleOutput {
                map_output: Some(Any {
                    value: buffer,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            clock: Some(Clock {
                number: 0,
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

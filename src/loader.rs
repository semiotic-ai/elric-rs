use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use anyhow::{Context, Error};
use clickhouse::{insert::Insert, inserter::Inserter, Client, Row};
use prost::Message;
use serde::{Deserialize, Serialize};
use substreams_database_change::pb::database::{
    table_change::PrimaryKey, CompositePrimaryKey, DatabaseChanges, TableChange,
};

use crate::{
    convert_field_to_hash,
    pb::sf::substreams::rpc::v2::{BlockScopedData, BlockUndoSignal},
    table_info::{DynamicInsert, DynamicTable},
};

pub struct DatabaseLoader {
    client: Client,
    id: String,
    tables: HashMap<String, DynamicTable>,
    inserters: HashMap<String, Inserter<DynamicTable>>,
    cursor: Insert<Cursor>,
    touched_tables: HashSet<String>,
}

#[derive(Row, Serialize, Deserialize)]
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
                // .with_max_entries(750_000)
                .with_period(Some(Duration::from_secs(15)));
            inserters.insert(table_name, inserter);
        });

        let tables = table
            .into_iter()
            .map(|t| (t.table_name.clone(), t))
            .collect();

        let cursor = client
            .insert("cursors")
            .expect("error while creating cursors inserter")
            .with_timeouts(Some(Duration::from_secs(5)), Some(Duration::from_secs(20)));

        Self {
            client,
            id,
            tables,
            inserters,
            cursor,
            touched_tables: HashSet::new(),
        }
    }

    pub async fn process_block_scoped_data(&mut self, data: &BlockScopedData) -> Result<(), Error> {
        let output = data.output.as_ref().unwrap().map_output.as_ref().unwrap();
        let database_changes = DatabaseChanges::decode(output.value.as_slice())?;

        // split between tables
        let splitted_inserts = split_table_changes(database_changes.table_changes);
        // let mut loader = loader.lock().unwrap();

        for (table, changes) in splitted_inserts {
            let table_info = self
                .get_table_info(&table)
                .expect(&format!("It was not possible to find the table {}", table))
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
                    .context("inserter write")?;
            }

            inserter.commit().await.context("Inserter end")?;
        }

        println!(
            "Block #{} - Payload {} ({} bytes)",
            data.clock.as_ref().unwrap().number,
            output.type_url.replace("type.googleapis.com/", ""),
            output.value.len()
        );

        Ok(())
    }

    pub async fn process_block_undo_signal(
        &mut self,
        _undo_signal: &BlockUndoSignal,
    ) -> Result<(), anyhow::Error> {
        let block_num = _undo_signal.last_valid_block.as_ref().unwrap().number;
        for table in self.touched_tables.drain().collect::<Vec<_>>() {
            self.delete_table_block_greater_than(table, block_num)
                .await?;
        }
        Ok(())
    }

    async fn delete_table_block_greater_than(
        &mut self,
        table: String,
        block: u64,
    ) -> Result<(), anyhow::Error> {
        self.client
            .query(&format!(
                "ALTER TABLE {} DELETE WHERE block_num > {}",
                table, block
            ))
            .execute()
            .await?;

        Ok(())
    }

    pub async fn persist_cursor(
        self: &mut Self,
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
        Ok(())
    }

    fn get_table_inserter(&mut self, table_name: &str) -> Option<&mut Inserter<DynamicTable>> {
        self.inserters.get_mut(table_name)
    }

    fn get_table_info(&mut self, table_name: &str) -> Option<&DynamicTable> {
        self.touched_tables.insert(table_name.into());
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
    let mut table_map: HashMap<String, Vec<TableChange>> = HashMap::new();

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

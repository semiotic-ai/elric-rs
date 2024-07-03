use std::{
    collections::{HashMap, VecDeque},
    str::FromStr,
};

use clickhouse::{schema::Schema, Client, Row};
use primitive_types::U256;
use serde::{ser::SerializeTuple, Deserialize, Serialize};
use strum_macros::EnumString;

use crate::ElricError;

#[derive(Debug, Clone, PartialEq, Default, PartialOrd, Eq, EnumString)]
pub enum ColumnType {
    #[default]
    String,
    FixedString(usize),
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    UInt128,
    UInt256,
    Int8,
    Int16,
    Int32,
    Int64,
    Int128,
    Int256,
    Float32,
    Float64,
    DateTime,
    Date,
    Bool,
    LowCardinality,
    Decimal,
    Nullable(Box<ColumnType>),
}

pub struct DynamicInsert {
    data: HashMap<String, String>,
    table_info: DynamicTable,
}

#[derive(Clone)]
pub struct DynamicTable {
    pub table_name: String,
    column_info: Vec<ColumnInfo>,
}
impl DynamicTable {
    pub fn new(table_name: &str, column_info: Vec<ColumnInfo>) -> Self {
        Self {
            table_name: table_name.to_string(),
            column_info,
        }
    }
}

impl Schema for DynamicTable {
    fn get_columns(&self) -> Vec<&str> {
        self.column_info
            .iter()
            .map(|column| column.column_name.as_str())
            .collect::<Vec<&str>>()
    }
}

impl DynamicInsert {
    pub fn new(table_info: DynamicTable, data: HashMap<String, String>) -> Self {
        Self { table_info, data }
    }
}
impl Serialize for DynamicInsert {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serializer = serializer.serialize_tuple(self.table_info.column_info.len())?;
        for column in self.table_info.column_info.iter() {
            let data = self.data.get(&column.column_name);
            if let Some(data) = data {
                match column.data_type {
                    ColumnType::String => serializer.serialize_element(data)?,
                    ColumnType::Float32 => {
                        let value = &data.parse::<f32>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::Float64 => {
                        let value = &data.parse::<f64>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::UInt8 => {
                        let value = &data.parse::<u8>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::UInt16 => {
                        let value = &data.parse::<u16>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::UInt32 => {
                        let value = &data.parse::<u32>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::UInt64 => {
                        let value = &data.parse::<u64>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::UInt128 => {
                        let value = &data.parse::<u128>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::UInt256 => {
                        let value = U256::from_dec_str(data).unwrap().0;
                        serializer.serialize_element(&value)?;
                    }
                    ColumnType::Int8 => {
                        let value = &data.parse::<i8>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::Int16 => {
                        let value = &data.parse::<i16>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::Int32 => {
                        let value = &data.parse::<i32>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::Int64 => {
                        let value = &data.parse::<i64>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::Int128 => {
                        let value = &data.parse::<i128>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::Int256 => {
                        let value = U256::from_dec_str(data).unwrap().0;
                        serializer.serialize_element(&value)?;
                    }
                    ColumnType::FixedString(size) => {
                        let bytes = data.as_bytes();
                        for i in 0..size {
                            let v = if i < bytes.len() { bytes[i] } else { 0 };
                            serializer.serialize_element(&v)?;
                        }
                    }
                    ColumnType::Bool => {
                        let value = &data.parse::<bool>().unwrap();
                        serializer.serialize_element(value)?;
                    }
                    ColumnType::DateTime => {
                        let time = chrono::DateTime::parse_from_rfc3339(data)
                            .unwrap()
                            .timestamp() as i32;
                        serializer.serialize_element(&time)?;
                    }
                    ColumnType::Date
                    | ColumnType::Nullable(_)
                    | ColumnType::LowCardinality
                    | ColumnType::Decimal => {
                        unimplemented!("{:?} not implemented", column.data_type)
                    }
                }
            }
        }
        serializer.end()
    }
}

impl<'de> Deserialize<'de> for ColumnType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data_type: String = Deserialize::deserialize(deserializer)?;
        let data = data_type
            .split('(')
            .collect::<VecDeque<_>>()
            .pop_front()
            .unwrap();

        let column_type = ColumnType::from_str(data)
            .unwrap_or_else(|_| panic!("unimplemented column type {}", data_type));
        match column_type {
            ColumnType::FixedString(_) => {
                let size: usize = data_type
                    .replace("FixedString(", "")
                    .replace(')', "")
                    .parse()
                    .expect("could not get fixedstring usize");
                Ok(ColumnType::FixedString(size))
            }
            _ => Ok(column_type),
        }
    }
}

#[derive(Row, Deserialize, Debug, Clone, Eq, PartialEq, PartialOrd)]
pub struct ColumnInfo {
    pub column_name: String,
    pub data_type: ColumnType,
}

impl Ord for ColumnInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.column_name.cmp(&other.column_name)
    }
}

#[derive(Row, Deserialize, Debug)]
pub struct TableInfo {
    pub table_schema: String,
    pub table_name: String,
}

pub async fn get_columns(
    client: &Client,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnInfo>, ElricError> {
    let query = client.query(&format!(
        "
	SELECT
		column_name,
		data_type
	FROM
		information_schema.columns
	WHERE
		table_schema = '{}' AND
		table_name = '{}'
	ORDER BY
		column_name,
		data_type
                 ",
        database, table
    ));
    let result = query
        .fetch_all()
        .await
        .map_err(|_| ElricError::ColumnNotFound(database.into(), table.into()))?;
    Ok(result)
}

pub async fn get_table_information(client: &Client) -> Result<Vec<TableInfo>, ElricError> {
    let query = client.query(
        format!(
            "
            SELECT database AS table_schema,
                     name AS table_name
              FROM system.tables
              WHERE NOT is_temporary
                AND engine NOT LIKE '%View'
                AND engine NOT LIKE 'System%'
                AND has_own_data != 0
                AND database = '{}'
              ORDER BY database, name
                 ",
            client.database().unwrap_or("default")
        )
        .as_str(),
    );
    let result = query
        .fetch_all()
        .await
        .map_err(|e| ElricError::LoadSchemaError(e))?;
    Ok(result)
}

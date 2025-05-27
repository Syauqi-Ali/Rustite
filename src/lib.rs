// File: lib.rs
use napi::{Env, JsObject, Result, Error};
use napi_derive::napi;
use rusqlite::{Connection, Row, Statement, params, ToSql};
use serde_json::{Value, Map};
use std::sync::Arc;

#[napi]
#[derive(Clone)]
pub struct Database {
    conn: Arc<Connection>,
}

#[napi]
#[derive(Clone)]
pub struct Table {
    name: String,
    conn: Arc<Connection>,
}

#[napi]
#[derive(Clone)]
pub struct Record {
    id: i64,
    pub data: Value,
    conn: Arc<Connection>,
    table: String,
}

#[napi]
pub struct RecordList {
    records: Vec<Record>,
}

#[napi]
impl Database {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        Ok(Self { conn: Arc::new(Connection::open(path).map_err(to_napi_error)?) })
    }

    #[napi]
    pub fn table(&self, name: String) -> Table {
        Table { name, conn: self.conn.clone() }
    }

    #[napi]
    pub fn execute(&self, sql: String) -> Result<()> {
        self.conn.execute(&sql, []).map_err(to_napi_error)?;
        Ok(())
    }

    #[napi]
    pub fn query_all(&self, sql: String) -> Result<String> {
        query_json(&self.conn, &sql, &[])
            .and_then(|v| serde_json::to_string(&v).map_err(to_napi_error))
    }
}

#[napi]
impl Table {
    #[napi]
    pub fn find(&self, id: i64) -> Result<Option<Record>> {
        self.query_one("id = ?", &[&id])
    }

    #[napi]
    pub fn insert(&self, env: Env, js_data: JsObject) -> Result<i64> {
        let obj: Value = env.from_js_value(js_data.into_unknown())?;
        let map = obj.as_object().ok_or_else(|| Error::from_reason("Expected object"))?;

        let keys: Vec<_> = map.keys().cloned().collect();
        let values: Vec<_> = map.values().map(json_to_sql).collect::<Vec<_>>();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            self.name,
            keys.join(", "),
            vec!["?"; keys.len()].join(", ")
        );
        let params: Vec<&dyn ToSql> = values.iter().map(|v| v as &dyn ToSql).collect();

        self.conn.execute(&sql, &*params).map_err(to_napi_error)?;
        Ok(self.conn.last_insert_rowid())
    }

    #[napi]
    pub fn where_(&self, column: String, op: String, value: String) -> Result<RecordList> {
        let sql = format!("SELECT * FROM {} WHERE {} {} ?", self.name, column, op);
        Ok(RecordList { records: query_records(&self.conn, &self.name, &sql, &[&value])? })
    }

    #[napi]
    pub fn first(&self) -> Result<Option<Record>> {
        self.query_one("1=1 ORDER BY id ASC LIMIT 1", &[])
    }

    #[napi]
    pub fn last(&self) -> Result<Option<Record>> {
        self.query_one("1=1 ORDER BY id DESC LIMIT 1", &[])
    }

    fn query_one(&self, cond: &str, params: &[&dyn ToSql]) -> Result<Option<Record>> {
        let sql = format!("SELECT * FROM {} WHERE {}", self.name, cond);
        let mut list = query_records(&self.conn, &self.name, &sql, params)?;
        Ok(list.pop())
    }
}

#[napi]
impl RecordList {
    #[napi]
    pub fn first(&self) -> Option<Record> {
        self.records.first().cloned()
    }

    #[napi]
    pub fn last(&self) -> Option<Record> {
        self.records.last().cloned()
    }

    #[napi(getter)]
    pub fn data(&self) -> Vec<Value> {
        self.records.iter().map(|r| r.data.clone()).collect()
    }
}

#[napi]
impl Record {
    #[napi]
    pub fn update(&self, env: Env, js_data: JsObject) -> Result<Record> {
        let obj: Value = env.from_js_value(js_data.into_unknown())?;
        let map = obj.as_object().ok_or_else(|| Error::from_reason("Expected object"))?;

        let fields: Vec<_> = map.keys().map(|k| format!("{} = ?", k)).collect();
        let mut values: Vec<_> = map.values().map(json_to_sql).collect();
        values.push(rusqlite::types::Value::Integer(self.id));

        let sql = format!("UPDATE {} SET {} WHERE id = ?", self.table, fields.join(", "));
        let params: Vec<&dyn ToSql> = values.iter().map(|v| v as &dyn ToSql).collect();

        self.conn.execute(&sql, &*params).map_err(to_napi_error)?;
        Ok(self.clone())
    }

    #[napi]
    pub fn destroy(&self) -> Result<()> {
        let sql = format!("DELETE FROM {} WHERE id = ?", self.table);
        self.conn.execute(&sql, params![self.id]).map_err(to_napi_error)?;
        Ok(())
    }

    #[napi]
    pub fn delete(&self) -> Result<()> {
        self.destroy()
    }
}

fn query_json(conn: &Connection, sql: &str, params: &[&dyn ToSql]) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql).map_err(to_napi_error)?;
    let cols = get_column_names(&stmt);
    let mut rows = stmt.query(params).map_err(to_napi_error)?;
    let mut out = vec![];

    while let Some(row) = rows.next().map_err(to_napi_error)? {
        out.push(Value::Object(row_to_json(&row, &cols)));
    }
    Ok(out)
}

fn query_records(conn: &Arc<Connection>, table: &str, sql: &str, params: &[&dyn ToSql]) -> Result<Vec<Record>> {
    let mut stmt = conn.prepare(sql).map_err(to_napi_error)?;
    let cols = get_column_names(&stmt);
    let mut rows = stmt.query(params).map_err(to_napi_error)?;
    let mut out = vec![];

    while let Some(row) = rows.next().map_err(to_napi_error)? {
        let id: i64 = row.get("id").unwrap_or(0);
        out.push(Record {
            id,
            data: Value::Object(row_to_json(&row, &cols)),
            conn: Arc::clone(conn),
            table: table.to_string(),
        });
    }
    Ok(out)
}

fn row_to_json(row: &Row, columns: &[String]) -> Map<String, Value> {
    columns.iter().enumerate().map(|(i, k)| {
        let val = row.get(i).unwrap_or(rusqlite::types::Value::Null);
        let json = match val {
            rusqlite::types::Value::Null => Value::Null,
            rusqlite::types::Value::Integer(i) => Value::from(i),
            rusqlite::types::Value::Real(f) => Value::from(f),
            rusqlite::types::Value::Text(t) => Value::from(t),
            _ => Value::Null,
        };
        (k.clone(), json)
    }).collect()
}

fn get_column_names(stmt: &Statement) -> Vec<String> {
    (0..stmt.column_count())
        .map(|i| stmt.column_name(i).unwrap_or("col").to_string())
        .collect()
}

fn json_to_sql(v: &Value) -> rusqlite::types::Value {
    match v {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(b) => rusqlite::types::Value::Integer(*b as i64),
        Value::Number(n) => {
            n.as_i64()
                .map(rusqlite::types::Value::Integer)
                .or_else(|| n.as_f64().map(rusqlite::types::Value::Real))
                .unwrap_or(rusqlite::types::Value::Null)
        }
        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        _ => rusqlite::types::Value::Null,
    }
}

fn to_napi_error<E: std::fmt::Display>(e: E) -> Error {
    Error::from_reason(e.to_string())
}

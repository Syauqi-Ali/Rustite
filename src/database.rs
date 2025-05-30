use napi::{Env, JsObject, Result};
use napi_derive::napi;
use rusqlite::{Connection};
use std::sync::{Arc, Mutex};

use crate::extra::{row_to_object};
use crate::table::{Table};

#[napi]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[napi]
impl Database {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| napi::Error::from_reason(format!("Failed to open db: {}", e)))?;
        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    #[napi]
    pub fn execute(&self, sql: String) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(&sql)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(())
    }

    #[napi]
    pub fn query(&self, env: Env, sql: String) -> Result<Vec<JsObject>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        let column_names: Vec<String> =
            stmt.column_names().iter().map(|s| s.to_string()).collect();

        let rows = stmt
            .query_map([], |row| row_to_object(env, row, &column_names))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| napi::Error::from_reason(e.to_string()))?);
        }

        Ok(results)
    }

    #[napi]
    pub fn table(&self, name: String) -> Result<Table> {
        Ok(Table {
            name,
            conn: self.conn.clone(),
            //relations: vec![],
        })
    }
}


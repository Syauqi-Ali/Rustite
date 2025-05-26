use napi::Error;
use napi_derive::napi;
use rusqlite::{Connection};
use serde_json::{Value, Map};

#[napi]
pub struct Database {
    conn: Connection,
}

#[napi]
impl Database {
    #[napi(constructor)]
    pub fn new(path: String) -> napi::Result<Self> {
        let conn = Connection::open(path).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Database { conn })
    }

    #[napi]
    pub fn execute(&self, sql: String) -> napi::Result<()> {
        self.conn.execute(&sql, []).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(())
    }

    #[napi]
    pub fn query_all(&self, sql: String) -> napi::Result<String> {
        let mut stmt = self.conn.prepare(&sql).map_err(|e| Error::from_reason(e.to_string()))?;

        let column_count = stmt.column_count();
        let column_names: Vec<String> = (0..column_count)
            .map(|i| stmt.column_name(i).unwrap_or("col").to_string())
            .collect();

        let mut rows = stmt.query([]).map_err(|e| Error::from_reason(e.to_string()))?;
        let mut results = Vec::new();

        while let Some(row) = rows.next().map_err(|e| Error::from_reason(e.to_string()))? {
            let mut obj = Map::new();
            for (i, col_name) in column_names.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(i).unwrap_or(rusqlite::types::Value::Null);
                let json_val = match val {
                    rusqlite::types::Value::Null => Value::Null,
                    rusqlite::types::Value::Integer(i) => Value::from(i),
                    rusqlite::types::Value::Real(f) => Value::from(f),
                    rusqlite::types::Value::Text(t) => Value::from(t),
                    rusqlite::types::Value::Blob(_) => Value::Null,
                };
                obj.insert(col_name.clone(), json_val);
            }
            results.push(Value::Object(obj));
        }

        let json = serde_json::to_string(&results).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(json)
    }
}

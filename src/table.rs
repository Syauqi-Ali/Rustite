use napi::{Env, JsObject, JsUnknown, Result};
use napi_derive::napi;
use rusqlite::{Connection};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use crate::extra::{js_object_to_hashmap, js_unknown_to_rusqlite_value};
use crate::filtered_table::{FilteredTable};

#[napi]
pub struct Table {
    pub(crate) name: String,
    pub(crate) conn: Arc<Mutex<Connection>>,
}

#[napi]
impl Table {
    #[napi]
    pub fn first(&self, env: Env) -> Result<Option<JsObject>> {
        FilteredTable {
            table: self.clone(),
            column: "1".to_string(),
            operator: "=".to_string(),
            value: napi::Either::B(1),
            extra_conditions: vec![],
            order_by: Some(("id".to_string(), "ASC".to_string())),
        }.first(env)
    }

    #[napi]
    pub fn last(&self, env: Env) -> Result<Option<JsObject>> {
        FilteredTable {
            table: self.clone(),
            column: "1".to_string(),
            operator: "=".to_string(),
            value: napi::Either::B(1),
            extra_conditions: vec![],
            order_by: Some(("id".to_string(), "DESC".to_string())),
        }.first(env)
    }
    
    #[napi]
    pub fn find(&self, env: Env, id: napi::Either<String, i64>) -> Result<Option<JsObject>> {
        FilteredTable {
            table: self.clone(),
            column: "id".to_string(),
            operator: "=".to_string(),
            value: id,
            extra_conditions: vec![],
            order_by: None,
        }.first(env)
    }
    
    #[napi]
    pub fn get(&self, env: Env) -> Result<Vec<JsObject>> {
        self.all(env)
    }

    #[napi]
    pub fn all(&self, env: Env) -> Result<Vec<JsObject>> {
        FilteredTable {
            table: self.clone(),
            column: "1".to_string(),
            operator: "=".to_string(),
            value: napi::Either::B(1),
            extra_conditions: vec![],
            order_by: None,
        }.all(env)
    }

    #[napi]
    pub fn where_(
        &self,
        column: String,
        op_or_value: napi::Either<String, napi::Either<String, i64>>,
        value_opt: Option<napi::Either<String, i64>>,
    ) -> Result<FilteredTable> {
        let (operator, value) = if let Some(v) = value_opt {
            let op = match op_or_value {
                napi::Either::A(op) => op,
                _ => "=".to_string(),
            };
            (op, v)
        } else {
            let val = match op_or_value {
                napi::Either::B(v) => v,
                _ => {
                    return Err(napi::Error::from_reason(
                        "Invalid arguments for where".to_string(),
                    ))
                }
            };
            ("=".to_string(), val)
        };

        Ok(FilteredTable {
            table: self.clone(),
            column,
            operator,
            value,
            extra_conditions: vec![],
            order_by: None,
        })
    }
    
    #[napi]
    pub fn insert(&self, env: Env, data: JsUnknown) -> Result<()> {
        let rows: Vec<HashMap<String, JsUnknown>> = if data.is_array()? {
            let arr = data.coerce_to_object()?;
            let length = arr.get_array_length()?;
            let mut vec = Vec::with_capacity(length as usize);
            for i in 0..length {
                let item: JsUnknown = arr.get_element::<JsUnknown>(i)?;
                let obj = item.coerce_to_object()?;
                let map = js_object_to_hashmap(&env, &obj)?;
                vec.push(map);
            }
            vec
        } else {
            let obj = data.coerce_to_object()?;
            let map = js_object_to_hashmap(&env, &obj)?;
            vec![map]
        };

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| napi::Error::from_reason(e.to_string()))?;

        for mut row in rows {
            if row.is_empty() {
                continue;
            }
            let columns: Vec<String> = row.keys().cloned().collect();
            let placeholders = vec!["?"; columns.len()].join(", ");
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                self.name,
                columns.join(", "),
                placeholders
            );

            let mut stmt = tx.prepare(&sql).map_err(|e| napi::Error::from_reason(e.to_string()))?;

            let values: Vec<rusqlite::types::Value> = columns
                .iter()
                .map(|col| {
            let val = row
                .remove(col)
                .ok_or_else(|| napi::Error::from_reason(format!("Missing value for column {}", col)))?;
            js_unknown_to_rusqlite_value(val)
        })
        .collect::<Result<Vec<_>, _>>()?;

        stmt.execute(rusqlite::params_from_iter(values))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        }

        tx.commit().map_err(|e| napi::Error::from_reason(e.to_string()))?;

        Ok(())
    }

    #[napi]
    pub fn create(&self, env: Env, data: JsUnknown) -> Result<()> {
        self.insert(env, data)
    }
    
    #[napi]
    pub fn update(&self, id: napi::Either<String, i64>, data: JsObject) -> Result<()> {
        FilteredTable {
            table: self.clone(),
            column: "id".to_string(),
            operator: "=".to_string(),
            value: id,
            extra_conditions: vec![],
            order_by: None,
        }.update(data)
    }

    #[napi]
    pub fn order_by(&self, column: String, direction: Option<String>) -> Result<FilteredTable> {
        Ok(FilteredTable {
            table: self.clone(),
            column: "1".to_string(),
            operator: "=".to_string(),
            value: napi::Either::B(1),
            extra_conditions: vec![],
            order_by: Some((column, direction.unwrap_or("ASC".to_string()))),
        })
    }
    
    #[napi]
    pub fn destroy(&self, id: napi::Either<String, i64>) -> Result<()> {
        FilteredTable {
            table: self.clone(),
            column: "id".to_string(),
            operator: "=".to_string(),
            value: id,
            extra_conditions: vec![],
            order_by: None,
        }.destroy()
    }
}


impl Clone for Table {
    fn clone(&self) -> Self {
        Table {
            name: self.name.clone(),
            conn: self.conn.clone(),
            //relations: self.relations.clone(),
        }
    }
}

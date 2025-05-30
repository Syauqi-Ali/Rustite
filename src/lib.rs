use napi::{Env, JsObject, JsUnknown, Result, ValueType, JsString};
use napi_derive::napi;
use rusqlite::{Connection, Row};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

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
        })
    }
}

#[napi]
pub struct Table {
    name: String,
    conn: Arc<Mutex<Connection>>,
}

#[napi]
impl Table {
    #[napi]
    pub fn find(&self, env: Env, id: napi::Either<i64, String>) -> Result<Option<JsObject>> {
        let id_value = match id {
            napi::Either::A(num) => rusqlite::types::Value::Integer(num),
            napi::Either::B(text) => rusqlite::types::Value::Text(text),
        };

        let sql = format!("SELECT * FROM {} WHERE id = ?", self.name);

        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        let column_names: Vec<String> =
            stmt.column_names().iter().map(|s| s.to_string()).collect();

        let mut rows = stmt
            .query([id_value])
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        match rows.next() {
            Ok(Some(row)) => {
                let obj = row_to_object(env, &row, &column_names)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
                Ok(Some(obj))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(napi::Error::from_reason(e.to_string())),
        }
    }
    
    #[napi]
    pub fn get(&self, env: Env) -> Result<Vec<JsObject>> {
        self.all(env)
    }

    #[napi]
    pub fn all(&self, env: Env) -> Result<Vec<JsObject>> {
        let sql = format!("SELECT * FROM {}", self.name);

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
        for row_result in rows {
            results.push(row_result.map_err(|e| napi::Error::from_reason(e.to_string()))?);
        }

        Ok(results)
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
    pub fn insert(&self, data: JsUnknown) -> Result<()> {
        let rows: Vec<HashMap<String, JsUnknown>> = if data.is_array()? {
            let arr = data.coerce_to_object()?;
            let length = arr.get_array_length()?;
            let mut vec = Vec::with_capacity(length as usize);
            for i in 0..length {
                let item: JsUnknown = arr.get_element::<JsUnknown>(i)?;
                let obj = item.coerce_to_object()?;
                let map = js_object_to_hashmap(&obj)?;
                vec.push(map);
            }
            vec
        } else {
            let obj = data.coerce_to_object()?;
            let map = js_object_to_hashmap(&obj)?;
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
    pub fn create(&self, data: JsUnknown) -> Result<()> {
        self.insert(data)
    }
    
    #[napi]
    pub fn update(&self, id: napi::Either<String, i64>, data: JsObject) -> Result<()> {
        let filtered = FilteredTable {
            table: self.clone(),
            column: "id".to_string(),
            operator: "=".to_string(),
            value: id,
            extra_conditions: vec![],
            order_by: None,
        };
        filtered.update(data)
    }

    #[napi]
    pub fn order_by(&self, column: String, direction: Option<String>) -> Result<FilteredTable> {
        Ok(FilteredTable {
            table: self.clone(),
            column: "1".to_string(), // Always true dummy condition
            operator: "=".to_string(),
            value: napi::Either::B(1),
            extra_conditions: vec![],
            order_by: Some((column, direction.unwrap_or("ASC".to_string()))),
        })
    }
    
    #[napi]
    pub fn destroy(&self, id: napi::Either<String, i64>) -> Result<()> {
        let filtered = FilteredTable {
            table: self.clone(),
            column: "id".to_string(),
            operator: "=".to_string(),
            value: id,
            extra_conditions: vec![],
            order_by: None,
        };
        filtered.destroy()
    }
}

#[napi]
#[derive(Clone)]
pub struct FilteredTable {
    table: Table,
    column: String,
    operator: String,
    value: napi::Either<String, i64>,
    extra_conditions: Vec<(String, String, napi::Either<String, i64>)>,
    order_by: Option<(String, String)>,
}

#[napi]
impl FilteredTable {
    #[napi]
    pub fn first(&self, env: Env) -> Result<Option<JsObject>> {
        Ok(self.all(env)?.into_iter().next())
    }

    #[napi]
    pub fn last(&self, env: Env) -> Result<Option<JsObject>> {
        Ok(self.all(env)?.into_iter().last())
    }

    #[napi]
    pub fn order_by(&mut self, column: String, direction: Option<String>) -> Result<Self> {
        self.order_by = Some((column, direction.unwrap_or_else(|| "ASC".to_string())));
        Ok(self.clone())
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
            match op_or_value {
                napi::Either::B(v) => ("=".to_string(), v),
                _ => return Err(napi::Error::from_reason("Invalid arguments for where"))
            }
        };

        let mut extra = self.extra_conditions.clone();
        extra.push((self.column.clone(), self.operator.clone(), self.value.clone()));

        Ok(Self {
            table: self.table.clone(),
            column,
            operator,
            value,
            extra_conditions: extra,
            order_by: None,
        })
    }

    #[napi]
    pub fn get(&self, env: Env) -> Result<Vec<JsObject>> {
        self.all(env)
    }

    fn build_conditions(
        &self,
        sql: &mut String,
        params: &mut Vec<rusqlite::types::Value>,
    ) {
        let mut add_condition = |col: &str, op: &str, val: &napi::Either<String, i64>| {
            match op.to_uppercase().as_str() {
                "IS NULL" | "IS NOT NULL" => {
                    sql.push_str(&format!("{} {} AND ", col, op));
                }
                "IN" => {
                    let val_str = match val {
                        napi::Either::A(s) => s,
                        napi::Either::B(i) => &i.to_string(),
                    };
                    let items: Vec<&str> = val_str.split(',').map(str::trim).collect();
                    let placeholders = items.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                    sql.push_str(&format!("{} IN ({}) AND ", col, placeholders));
                    for item in items {
                        params.push(rusqlite::types::Value::Text(item.to_string()));
                    }
                }
                _ => {
                    sql.push_str(&format!("{} {} ? AND ", col, op));
                    let p = match val {
                        napi::Either::A(s) => rusqlite::types::Value::Text(s.clone()),
                        napi::Either::B(i) => rusqlite::types::Value::Integer(*i),
                    };
                    params.push(p);
                }
            }
        };

        add_condition(&self.column, &self.operator, &self.value);
        for (col, op, val) in &self.extra_conditions {
            add_condition(col, op, val);
        }

        sql.truncate(sql.trim_end_matches(" AND ").len());
    }

    #[napi]
    pub fn all(&self, env: Env) -> Result<Vec<JsObject>> {
        let mut sql = format!("SELECT * FROM {} WHERE ", self.table.name);
        let mut params = Vec::new();

        self.build_conditions(&mut sql, &mut params);

        if let Some((ref col, ref dir)) = self.order_by {
            sql.push_str(&format!(" ORDER BY {} {}", col, dir));
        }

        let conn = self.table.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let column_names = stmt.column_names().iter().map(|s| s.to_string()).collect::<Vec<_>>();

        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            row_to_object(env, row, &column_names)
        }).map_err(|e| napi::Error::from_reason(e.to_string()))?;

        let results = rows
            .map(|res| res.map_err(|e| napi::Error::from_reason(e.to_string())))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    #[napi]
    pub fn destroy(&self) -> Result<()> {
        let mut sql = format!("DELETE FROM {} WHERE ", self.table.name);
        let mut params = Vec::new();

        self.build_conditions(&mut sql, &mut params);

        let conn = self.table.conn.lock().unwrap();
        conn.execute(&sql, rusqlite::params_from_iter(params))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(())
    }

    #[napi]
    pub fn update(&self, data: JsObject) -> Result<()> {
        let conn = self.table.conn.lock().unwrap();
        let mut keys = Vec::new();
        let mut values = Vec::new();

        let props = data.get_property_names()?;
        for i in 0..props.get_array_length()? {
            let key_js = props.get_element::<JsString>(i)?;
            let key = key_js.into_utf8()?.as_str()?.to_string();
            let value = data.get_named_property::<JsUnknown>(&key)?;
            let value_type = value.get_type()?;

            let sql_value = match value_type {
                ValueType::String => format!("'{}'", value.coerce_to_string()?.into_utf8()?.as_str()?),
                ValueType::Number => format!("{}", value.coerce_to_number()?.get_double()?),
                ValueType::Boolean => format!("{}", value.coerce_to_bool()?.get_value()? as i32),
                _ => return Err(napi::Error::from_reason("Unsupported value type in update")),
            };

            keys.push(key);
            values.push(sql_value);
        }

        let set_clause = keys.iter().zip(values.iter())
            .map(|(k, v)| format!("{} = {}", k, v))
            .collect::<Vec<_>>().join(", ");

        let mut where_clause = format!("{} {} ?", self.column, self.operator);
        for (col, op, _) in &self.extra_conditions {
            where_clause.push_str(&format!(" AND {} {} ?", col, op));
        }

        let sql = format!("UPDATE {} SET {} WHERE {}", self.table.name, set_clause, where_clause);

        let mut params = vec![match &self.value {
            napi::Either::A(s) => rusqlite::types::Value::Text(s.clone()),
            napi::Either::B(i) => rusqlite::types::Value::Integer(*i),
        }];

        for (_, _, v) in &self.extra_conditions {
            params.push(match v {
                napi::Either::A(s) => rusqlite::types::Value::Text(s.clone()),
                napi::Either::B(i) => rusqlite::types::Value::Integer(*i),
            });
        }

        conn.execute(&sql, rusqlite::params_from_iter(params))
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        Ok(())
    }
}



impl Clone for Table {
    fn clone(&self) -> Self {
        Table {
            name: self.name.clone(),
            conn: self.conn.clone(),
        }
    }
}

fn row_to_object(env: Env, row: &Row, columns: &[String]) -> rusqlite::Result<JsObject> {
    let mut obj = env.create_object().unwrap();

    for (i, col) in columns.iter().enumerate() {
        let val: rusqlite::types::Value = row.get(i)?;
        match val {
            rusqlite::types::Value::Integer(v) => {
                obj.set(col.as_str(), v).unwrap();
            }
            rusqlite::types::Value::Real(v) => {
                obj.set(col.as_str(), v).unwrap();
            }
            rusqlite::types::Value::Text(v) => {
                obj.set(col.as_str(), v).unwrap();
            }
            rusqlite::types::Value::Blob(v) => {
                obj.set(col.as_str(), v).unwrap();
            }
            rusqlite::types::Value::Null => {
                obj.set(col.as_str(), env.get_undefined().unwrap()).unwrap();
            }
        }
    }

    Ok(obj)
}


fn js_object_to_hashmap(obj: &JsObject) -> Result<HashMap<String, JsUnknown>> {
    let property_names = obj.get_property_names()?;
    let length = property_names.get_array_length()?;
    let mut map = HashMap::new();
    for i in 0..length {
        let key = property_names.get_element::<JsUnknown>(i)?
            .coerce_to_string()?
            .into_utf8()?
            .as_str()?
            .to_owned();
        let value = obj.get::<_, JsUnknown>(&key)?.expect("Property missing");
        map.insert(key, value);
    }
    Ok(map)
}

fn js_unknown_to_rusqlite_value(val: JsUnknown) -> napi::Result<rusqlite::types::Value> {
    let val_type = val.get_type()?;

    match val_type {
        ValueType::Null | ValueType::Undefined => Ok(rusqlite::types::Value::Null),

        ValueType::Boolean => {
            let bool_val = val.coerce_to_bool()?.get_value()?;
            Ok(rusqlite::types::Value::Integer(if bool_val { 1 } else { 0 }))
        }

        ValueType::Number => {
            let num_val = val.coerce_to_number()?.get_double()?;
            Ok(rusqlite::types::Value::Real(num_val))
        }

        ValueType::String => {
            let str_val = val.coerce_to_string()?.into_utf8()?;
            Ok(rusqlite::types::Value::Text(str_val.as_str()?.to_owned()))
        }

        _ => Ok(rusqlite::types::Value::Null),
    }
}

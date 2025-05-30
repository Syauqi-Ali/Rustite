use napi::{Env, JsObject, JsUnknown, Result, ValueType, JsString};
use napi_derive::napi;

use crate::extra::{row_to_object};
use crate::table::{Table};

#[napi]
#[derive(Clone)]
pub struct FilteredTable {
    pub(crate) table: Table,
    pub(crate) column: String,
    pub(crate) operator: String,
    pub(crate) value: napi::Either<String, i64>,
    pub(crate) extra_conditions: Vec<(String, String, napi::Either<String, i64>)>,
    pub(crate) order_by: Option<(String, String)>,
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
    .map(|res| {
            let obj = res.map_err(|e| napi::Error::from_reason(e.to_string()))?;
            Ok(obj)
        })
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

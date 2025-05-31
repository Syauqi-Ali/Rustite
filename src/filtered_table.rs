use napi::{Env, JsObject, JsUnknown, Result, ValueType, JsString};
use napi_derive::napi;

use crate::extra::{row_to_object};
use crate::table::{Table};

use napi::{CallContext, JsUndefined};
use napi_derive::js_function;

#[js_function(1)]
fn update_callback(ctx: CallContext) -> Result<JsUndefined> {
    let this = ctx.this_unchecked::<JsObject>();
    let filter = ctx.env.unwrap::<FilteredTable>(&this)?;
    let data = ctx.get::<JsObject>(0)?;
    filter.update(data)?;
    ctx.env.get_undefined()
}

#[js_function(1)]
fn destroy_callback(ctx: CallContext) -> Result<JsUndefined> {
    let this = ctx.this_unchecked::<JsObject>();
    let filter = ctx.env.unwrap::<FilteredTable>(&this)?;
    filter.destroy()?;
    ctx.env.get_undefined()
}

fn attach_ops(env: Env, obj: JsObject, filter: FilteredTable) -> Result<JsObject> {
    let mut wrapped = env.create_object()?;
    let keys = obj.get_property_names()?;
    let len = keys.get_array_length()?;

    for i in 0..len {
        let key = keys.get_element::<JsString>(i)?;
        let key_utf8 = key.into_utf8()?;
        let key_str = key_utf8.as_str()?;
        let value = obj.get_named_property::<JsUnknown>(key_str)?;
        wrapped.set_named_property(key_str, value)?;
    }

    env.wrap(&mut wrapped, filter)?;

    let update = env.create_function("update", update_callback)?;
    let destroy = env.create_function("destroy", destroy_callback)?;

    wrapped.set_named_property("update", &update)?;
    wrapped.set_named_property("destroy", &destroy)?;
    wrapped.set_named_property("delete", &destroy)?;

    Ok(wrapped)
}

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
        let mut filtered = self.clone();
        filtered.order_by = Some((
            self.order_by
                .as_ref()
                .map(|(col, _)| col.clone())
                .unwrap_or_else(|| "rowid".to_string()), // default order by
            "ASC".to_string(),
        ));
        let mut iter = filtered.all(env)?.into_iter();
        Ok(iter.next().map(|obj| attach_ops(env, obj, self.clone())).transpose()?)
    }

    #[napi]
    pub fn last(&self, env: Env) -> Result<Option<JsObject>> {
        let mut filtered = self.clone();
        filtered.order_by = Some((
            self.order_by
                .as_ref()
                .map(|(col, _)| col.clone())
                .unwrap_or_else(|| "rowid".to_string()),
            "DESC".to_string(),
        ));
        let mut iter = filtered.all(env)?.into_iter();
        Ok(iter.next().map(|obj| attach_ops(env, obj, self.clone())).transpose()?)
    }

    #[napi]
    pub fn order_by(&mut self, column: String, direction: Option<String>) -> Result<Self> {
        self.order_by = Some((column, direction.unwrap_or_else(|| "ASC".into())));
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
                _ => "=".into(),
            };
            (op, v)
        } else {
            match op_or_value {
                napi::Either::B(v) => ("=".into(), v),
                _ => return Err(napi::Error::from_reason("Invalid arguments for where")),
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

    fn build_conditions(&self, sql: &mut String, params: &mut Vec<rusqlite::types::Value>) {
        let mut append_condition = |col: &str, op: &str, val: &napi::Either<String, i64>| {
            match op.to_uppercase().as_str() {
                "IS NULL" | "IS NOT NULL" => {
                    sql.push_str(&format!("{col} {op} AND "));
                }
                "IN" => {
                    let val_str = match val {
                        napi::Either::A(s) => s,
                        napi::Either::B(i) => &i.to_string(),
                    };
                    let items: Vec<&str> = val_str.split(',').map(str::trim).collect();
                    sql.push_str(&format!(
                        "{} IN ({}) AND ",
                        col,
                        std::iter::repeat("?").take(items.len()).collect::<Vec<_>>().join(", ")
                    ));
                    params.extend(items.into_iter().map(|item| rusqlite::types::Value::Text(item.to_string())));
                }
                _ => {
                    sql.push_str(&format!("{col} {op} ? AND "));
                    params.push(match val {
                        napi::Either::A(s) => rusqlite::types::Value::Text(s.clone()),
                        napi::Either::B(i) => rusqlite::types::Value::Integer(*i),
                    });
                }
            }
        };

        append_condition(&self.column, &self.operator, &self.value);
        for (col, op, val) in &self.extra_conditions {
            append_condition(col, op, val);
        }

        if sql.ends_with(" AND ") {
            sql.truncate(sql.len() - 5);
        }
    }

    #[napi]
    pub fn all(&self, env: Env) -> Result<Vec<JsObject>> {
        let mut sql = format!("SELECT * FROM {} WHERE ", self.table.name);
        let mut params = Vec::new();
        self.build_conditions(&mut sql, &mut params);

        if let Some((ref col, ref dir)) = self.order_by {
            sql.push_str(&format!(" ORDER BY {} {}", col, dir));
        }

        let conn = self.table.conn.lock().map_err(|e| napi::Error::from_reason(format!("Lock poisoned: {}", e)))?;
        let mut stmt = conn.prepare(&sql)
            .map_err(|e| napi::Error::from_reason(format!("Prepare failed: {}", e)))?;

        let column_names = stmt
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let rows = stmt
            .query_map(rusqlite::params_from_iter(params), |row| {
                row_to_object(env, row, &column_names)
            })
            .map_err(|e| napi::Error::from_reason(format!("Query failed: {}", e)))?;

        rows.map(|res| res.map_err(|e| napi::Error::from_reason(format!("Row failed: {}", e))))
            .collect()
    }

    #[napi]
    pub fn destroy(&self) -> Result<()> {
        let mut sql = format!("DELETE FROM {} WHERE ", self.table.name);
        let mut params = Vec::new();
        self.build_conditions(&mut sql, &mut params);

        let conn = self.table.conn.lock().map_err(|e| napi::Error::from_reason(format!("Lock poisoned: {}", e)))?;
        conn.execute(&sql, rusqlite::params_from_iter(params))
            .map_err(|e| napi::Error::from_reason(format!("Execute failed: {}", e)))?;
        Ok(())
    }

    #[napi]
    pub fn update(&self, data: JsObject) -> Result<()> {
        let conn = self.table.conn.lock().map_err(|e| napi::Error::from_reason(format!("Lock poisoned: {}", e)))?;

        let props = data.get_property_names()?;
        let mut keys = Vec::new();
        let mut values = Vec::new();
        let mut placeholders = Vec::new();

        for i in 0..props.get_array_length()? {
            let key = props.get_element::<JsString>(i)?.into_utf8()?.as_str()?.to_owned();
            let value = data.get_named_property::<JsUnknown>(&key)?;
            let val = match value.get_type()? {
                ValueType::String => rusqlite::types::Value::Text(
                    value.coerce_to_string()?.into_utf8()?.as_str()?.to_string(),
                ),
                ValueType::Number => rusqlite::types::Value::Real(
                    value.coerce_to_number()?.get_double()?,
                ),
                ValueType::Boolean => rusqlite::types::Value::Integer(
                    value.coerce_to_bool()?.get_value()? as i64,
                ),
                _ => return Err(napi::Error::from_reason("Unsupported value type in update")),
            };

            keys.push(key);
            values.push(val);
            placeholders.push("?");
        }

        let set_clause = keys.iter().zip(placeholders.iter())
            .map(|(k, p)| format!("{k} = {p}"))
            .collect::<Vec<_>>()
            .join(", ");

        let mut sql = format!("UPDATE {} SET {} WHERE ", self.table.name, set_clause);
        let mut where_params = Vec::new();
        self.build_conditions(&mut sql, &mut where_params);

        values.extend(where_params);
        conn.execute(&sql, rusqlite::params_from_iter(values))
            .map_err(|e| napi::Error::from_reason(format!("Execute failed: {}", e)))?;
        Ok(())
    }
}

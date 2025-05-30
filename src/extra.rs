use napi::{Env, JsObject, JsUnknown, Result, ValueType};
use rusqlite::{Row};
use std::collections::HashMap;

fn id_value_to_string(val: &rusqlite::types::Value) -> String {
    match val {
        rusqlite::types::Value::Text(s) => s.clone(),
        rusqlite::types::Value::Integer(i) => i.to_string(),
        _ => "".to_string(),
    }
}

pub fn row_to_object(env: Env, row: &Row, columns: &[String]) -> rusqlite::Result<JsObject> {
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

pub fn js_object_to_hashmap(env: &Env, obj: &JsObject) -> Result<HashMap<String, JsUnknown>> {
    let property_names = obj.get_property_names()?;
    let length = property_names.get_array_length()?;
    let mut map = HashMap::new();

    let global = env.get_global()?;
    let json = global.get_named_property::<JsObject>("JSON")?;
    let stringify = json.get_named_property::<napi::JsFunction>("stringify")?;

    for i in 0..length {
        let key = property_names
            .get_element::<JsUnknown>(i)?
            .coerce_to_string()?
            .into_utf8()?
            .as_str()?
            .to_owned();

        let Some(value) = obj.get::<_, JsUnknown>(&key)? else {
            continue;
        };

        match value.get_type()? {
            ValueType::Object => {
                let serialized = stringify
                    .call(None, &[value])?
                    .coerce_to_string()?
                    .into_unknown();
                map.insert(key, serialized);
            }
            ValueType::Undefined | ValueType::Null => {
                continue;
            }
            _ => {
                map.insert(key, value);
            }
        }
    }

    Ok(map)
}


pub fn js_unknown_to_rusqlite_value(val: JsUnknown) -> napi::Result<rusqlite::types::Value> {
    let val_type = val.get_type()?;

    match val_type {
        ValueType::Null | ValueType::Undefined => Ok(rusqlite::types::Value::Null),

        ValueType::Boolean => {
            let bool_val = val.coerce_to_bool()?.get_value()?;
            Ok(rusqlite::types::Value::Integer(if bool_val { 1 } else { 0 }))
        }

        ValueType::Number => {
            let num_val = val.coerce_to_number()?.get_double()?;
            if num_val.fract() == 0.0 {
        // Bilangan bulat
                Ok(rusqlite::types::Value::Integer(num_val as i64))
            } else {
        // Bilangan desimal
                Ok(rusqlite::types::Value::Real(num_val))
            }
        }

        ValueType::String => {
            let str_val = val.coerce_to_string()?.into_utf8()?;
            Ok(rusqlite::types::Value::Text(str_val.as_str()?.to_owned()))
        }

        _ => Ok(rusqlite::types::Value::Null),
    }
}

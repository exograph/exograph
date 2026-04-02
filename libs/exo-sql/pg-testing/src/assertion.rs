use anyhow::{Result, bail};
use serde_json::Value;
use tokio_postgres::Row;

/// Compare actual query results against expected values.
///
/// When `json_aggregate` is true, the result is expected to be a single text column
/// containing a JSON array (from `Selection::Json` mode).
pub(crate) fn compare_results(
    test_name: &str,
    actual_rows: &[Row],
    expected: &Value,
    unordered_paths: &[String],
    json_aggregate: bool,
) -> Result<()> {
    let actual = if json_aggregate {
        let json_text: String = actual_rows
            .first()
            .and_then(|row| row.get::<_, Option<String>>(0))
            .unwrap_or_else(|| "[]".to_string());
        serde_json::from_str(&json_text)
            .map_err(|e| anyhow::anyhow!("[{test_name}] Failed to parse JSON result: {e}"))?
    } else {
        Value::Array(actual_rows.iter().map(row_to_json).collect())
    };
    compare_values(test_name, &[], &actual, expected, unordered_paths)
}

fn compare_values(
    test_name: &str,
    path: &[String],
    actual: &Value,
    expected: &Value,
    unordered_paths: &[String],
) -> Result<()> {
    match (actual, expected) {
        (Value::Array(actual_arr), Value::Array(expected_arr)) => {
            if actual_arr.len() != expected_arr.len() {
                bail!(
                    "[{test_name}] {}: array length mismatch: got {}, expected {}",
                    format_path(path),
                    actual_arr.len(),
                    expected_arr.len()
                );
            }

            let path_str = format!("/{}", path.join("/"));
            let is_unordered = unordered_paths.contains(&path_str);

            if is_unordered {
                compare_array_unordered(test_name, path, actual_arr, expected_arr, unordered_paths)
            } else {
                for (i, (a, e)) in actual_arr.iter().zip(expected_arr.iter()).enumerate() {
                    let mut elem_path = path.to_vec();
                    elem_path.push(i.to_string());
                    compare_values(test_name, &elem_path, a, e, unordered_paths)?;
                }
                Ok(())
            }
        }
        (Value::Object(actual_map), Value::Object(expected_map)) => {
            for (key, expected_val) in expected_map {
                let mut field_path = path.to_vec();
                field_path.push(key.clone());
                match actual_map.get(key) {
                    Some(actual_val) => {
                        compare_values(
                            test_name,
                            &field_path,
                            actual_val,
                            expected_val,
                            unordered_paths,
                        )?;
                    }
                    None => {
                        bail!(
                            "[{test_name}] {}: missing. Available: {:?}",
                            format_path(&field_path),
                            actual_map.keys().collect::<Vec<_>>()
                        );
                    }
                }
            }
            Ok(())
        }
        _ => {
            if actual != expected {
                bail!(
                    "[{test_name}] {}: expected {expected}, got {actual}",
                    format_path(path)
                );
            }
            Ok(())
        }
    }
}

fn compare_array_unordered(
    test_name: &str,
    path: &[String],
    actual: &[Value],
    expected: &[Value],
    unordered_paths: &[String],
) -> Result<()> {
    if actual.len() != expected.len() {
        bail!(
            "[{test_name}] {}: array length mismatch: got {}, expected {}",
            format_path(path),
            actual.len(),
            expected.len()
        );
    }

    let mut remaining: Vec<&Value> = actual.iter().collect();

    for (i, expected_item) in expected.iter().enumerate() {
        let found = remaining.iter().position(|actual_item| {
            let mut elem_path = path.to_vec();
            elem_path.push(i.to_string());
            compare_values(
                test_name,
                &elem_path,
                actual_item,
                expected_item,
                unordered_paths,
            )
            .is_ok()
        });

        match found {
            Some(idx) => {
                remaining.remove(idx);
            }
            None => {
                bail!(
                    "[{test_name}] {}: could not find {expected_item} in actual array",
                    format_path(path)
                );
            }
        }
    }

    Ok(())
}

fn format_path(path: &[String]) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path.join("/"))
    }
}

fn row_to_json(row: &Row) -> Value {
    let mut map = serde_json::Map::new();
    for (i, column) in row.columns().iter().enumerate() {
        let name = column.name().to_string();
        let value = parse_column(row, i, column.type_());
        map.insert(name, value);
    }
    Value::Object(map)
}

fn parse_column(row: &Row, index: usize, pg_type: &tokio_postgres::types::Type) -> Value {
    use tokio_postgres::types::Type;

    fn convert<T>(opt: Option<T>, f: impl FnOnce(T) -> Value) -> Value {
        match opt {
            Some(v) => f(v),
            None => Value::Null,
        }
    }

    match *pg_type {
        Type::INT2 => convert(row.get::<_, Option<i16>>(index), |v| {
            Value::Number((v as i64).into())
        }),
        Type::INT4 => convert(row.get::<_, Option<i32>>(index), |v| {
            Value::Number((v as i64).into())
        }),
        Type::INT8 => convert(row.get::<_, Option<i64>>(index), |v| {
            Value::Number(v.into())
        }),
        Type::TEXT | Type::VARCHAR => convert(row.get::<_, Option<String>>(index), Value::String),
        Type::BOOL => convert(row.get::<_, Option<bool>>(index), Value::Bool),
        Type::FLOAT4 => convert(row.get::<_, Option<f32>>(index), |v| {
            serde_json::Number::from_f64(v as f64)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }),
        Type::FLOAT8 => convert(row.get::<_, Option<f64>>(index), |v| {
            serde_json::Number::from_f64(v)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }),
        _ => convert(
            row.try_get::<_, Option<String>>(index).ok().flatten(),
            Value::String,
        ),
    }
}

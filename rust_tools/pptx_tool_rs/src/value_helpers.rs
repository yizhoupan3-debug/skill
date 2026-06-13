// Value extraction helper functions.


pub fn value_array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

pub fn joined_array_chars(value: &Value, key: &str) -> usize {
    value_string_array(value, key).join("").chars().count()
}

pub fn value_string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| item.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

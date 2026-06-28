/// 内部：解析 JSON → Option<Object>，供两个公开函数共用
fn parse_to_object(json: &Option<String>) -> Option<serde_json::Map<String, serde_json::Value>> {
    let s = json.as_deref()?;
    let val = serde_json::from_str::<serde_json::Value>(s).ok()?;
    match val {
        serde_json::Value::Object(map) => Some(map),
        _ => None,
    }
}

/// 解析 JSON 字符串为有序键值对（保留插入顺序）
pub fn parse_json_map(json: &Option<String>) -> Vec<(String, String)> {
    parse_to_object(json)
        .map(|m| m.into_iter()
            .map(|(k, v)| (k, v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())))
            .collect())
        .unwrap_or_default()
}

/// 解析 JSON 字符串为 serde_json::Map（保留插入顺序）
pub fn parse_json_object(json: &Option<String>) -> serde_json::Map<String, serde_json::Value> {
    parse_to_object(json).unwrap_or_default()
}

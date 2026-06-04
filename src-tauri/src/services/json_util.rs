/// 解析 JSON 字符串为有序键值对（保留插入顺序）
pub fn parse_json_map(json: &Option<String>) -> Vec<(String, String)> {
    let Some(s) = json else { return vec![] };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(s) else { return vec![] };
    match val {
        serde_json::Value::Object(map) => map.into_iter()
            .map(|(k, v)| {
                let s = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
                (k, s)
            })
            .collect(),
        _ => vec![],
    }
}

/// 解析 JSON 字符串为 serde_json::Map（保留插入顺序）
pub fn parse_json_object(json: &Option<String>) -> serde_json::Map<String, serde_json::Value> {
    let Some(s) = json else { return serde_json::Map::new(); };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(s) else { return serde_json::Map::new(); };
    match val {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    }
}

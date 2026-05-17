use crate::AppState;
use crate::services::crypto::CryptoService;

#[tauri::command]
pub async fn chat_stream(messages: Vec<serde_json::Value>, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let (provider, model_id, base_url, api_key_enc) = {
        let db = state.db.lock();
        let cfg = db.query_row(
            "SELECT provider, model_id, base_url, api_key_encrypted FROM ai_model_configs WHERE is_active=1",
            [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, String>(3)?)),
        ).ok().ok_or("没有可用的 AI 模型配置".to_string())?;
        cfg
    };

    let api_key = CryptoService::decrypt(&api_key_enc).unwrap_or_else(|_| api_key_enc.to_string());
    let client = reqwest::Client::new();

    match provider.as_str() {
        "openai" => {
            let api_base = base_url.as_deref().unwrap_or("https://api.openai.com/v1");
            let resp = client.post(format!("{}/chat/completions", api_base.trim_end_matches('/')))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&serde_json::json!({"model": model_id, "messages": messages, "temperature": 0.7, "stream": false}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(body["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
        }
        "anthropic" => {
            let api_base = base_url.as_deref().unwrap_or("https://api.anthropic.com/v1");
            let resp = client.post(format!("{}/messages", api_base.trim_end_matches('/')))
                .header("x-api-key", api_key).header("anthropic-version", "2023-06-01")
                .json(&serde_json::json!({"model": model_id, "max_tokens": 2048, "messages": messages}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(body["content"][0]["text"].as_str().unwrap_or("").to_string())
        }
        p => Err(format!("不支持的 AI provider: {}", p)),
    }
}

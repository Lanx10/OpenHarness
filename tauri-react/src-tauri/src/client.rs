use serde_json::Value;
use std::time::Duration;

pub async fn query_llm(
    provider: &str,
    model_id: &str,
    prompt: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        "Anthropic-compatible" => {
            let key = api_key.ok_or("Anthropic API key is required")?;
            let url = base_url.unwrap_or("https://api.anthropic.com/v1/messages");
            let body = serde_json::json!({
                "model": model_id,
                "max_tokens": 2048,
                "messages": [{"role": "user", "content": prompt}]
            });
            let res = client
                .post(url)
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Network request failed: {}", e))?;

            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("Anthropic API error: {}", err_text));
            }

            let json: Value = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
            let content = json["content"][0]["text"]
                .as_str()
                .unwrap_or("No content in response");
            Ok(content.to_string())
        }
        "Ollama" => {
            let default_url = "http://localhost:11434/api/chat";
            let url = base_url.unwrap_or(default_url);
            let body = serde_json::json!({
                "model": model_id,
                "messages": [{"role": "user", "content": prompt}],
                "stream": false
            });
            let mut req = client.post(url).json(&body);
            if let Some(key) = api_key.filter(|key| !key.trim().is_empty()) {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
            let res = req
                .send()
                .await
                .map_err(|e| format!("Failed to connect to Ollama (is it running?): {}", e))?;

            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("Ollama returned error: {}", err_text));
            }

            let json: Value = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse Ollama JSON: {}", e))?;
            let content = json["message"]["content"]
                .as_str()
                .unwrap_or("No reply from Ollama");
            Ok(content.to_string())
        }
        // OpenAI-compatible, OpenRouter, Google AI Studio
        _ => {
            let key = api_key.ok_or_else(|| format!("API key is required for {}", provider))?;
            let url = if let Some(custom) = base_url {
                custom.to_string()
            } else if provider == "OpenRouter" {
                "https://openrouter.ai/api/v1/chat/completions".to_string()
            } else if provider == "Google AI Studio" {
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                    .to_string()
            } else {
                "https://api.openai.com/v1/chat/completions".to_string()
            };

            let body = serde_json::json!({
                "model": model_id,
                "messages": [{"role": "user", "content": prompt}]
            });

            let res = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", key))
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Network request failed: {}", e))?;

            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("{} returned error: {}", provider, err_text));
            }

            let json: Value = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
            let content = json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("No content in response choices");
            Ok(content.to_string())
        }
    }
}

use futures_util::StreamExt;
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
        _ => {
            let key = api_key.ok_or_else(|| format!("API key is required for {}", provider))?;
            let default_url = match provider {
                "OpenRouter" => "https://openrouter.ai/api/v1/chat/completions",
                "Google AI Studio" => {
                    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                }
                _ => return Err(format!("Unsupported provider: {}", provider)),
            };
            let url = base_url.unwrap_or(default_url);

            let body = serde_json::json!({
                "model": model_id,
                "messages": [{"role": "user", "content": prompt}]
            });

            let res = client
                .post(url)
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

pub async fn query_llm_stream(
    provider: &str,
    model_id: &str,
    prompt: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    on_token: impl Fn(String) + Send + Sync + 'static,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        "Ollama" => {
            let url = base_url.unwrap_or("http://localhost:11434/api/chat");
            let body = serde_json::json!({
                "model": model_id,
                "messages": [{"role": "user", "content": prompt}],
                "stream": true
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
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                for line in String::from_utf8_lossy(&chunk).lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<Value>(line)
                        && let Some(content) = json["message"]["content"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                    {
                        accumulated.push_str(content);
                        on_token(content.to_string());
                    }
                }
            }
            if accumulated.is_empty() {
                let content = "No reply from Ollama";
                on_token(content.to_string());
                Ok(content.to_string())
            } else {
                Ok(accumulated)
            }
        }
        _ => {
            let key = api_key.ok_or_else(|| format!("API key is required for {}", provider))?;
            let default_url = match provider {
                "OpenRouter" => "https://openrouter.ai/api/v1/chat/completions",
                "Google AI Studio" => {
                    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                }
                _ => return Err(format!("Unsupported provider: {}", provider)),
            };
            let url = base_url.unwrap_or(default_url);
            let body = serde_json::json!({
                "model": model_id,
                "messages": [{"role": "user", "content": prompt}],
                "stream": true
            });
            let res = client
                .post(url)
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
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].to_string();
                    buffer.drain(..=pos);
                    let line = line.trim();
                    if !line.starts_with("data:") {
                        continue;
                    }
                    let data = line[5..].trim();
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<Value>(data)
                        && let Some(content) = json["choices"][0]["delta"]["content"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                    {
                        accumulated.push_str(content);
                        on_token(content.to_string());
                    }
                }
            }
            if accumulated.is_empty() {
                let content = "No content in response choices";
                on_token(content.to_string());
                Ok(content.to_string())
            } else {
                Ok(accumulated)
            }
        }
    }
}

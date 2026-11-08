use futures_util::StreamExt;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
}

impl ModelMessage {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            role: match role {
                "assistant" => "assistant",
                "system" => "system",
                _ => "user",
            }
            .to_string(),
            content: content.to_string(),
        }
    }
}

const MAX_ERROR_BODY_CHARS: usize = 512;
const MAX_RESPONSE_BYTES: usize = 1_000_000;

fn safe_error_body(body: &str) -> String {
    body.chars().take(MAX_ERROR_BODY_CHARS).collect()
}

fn messages_json(messages: &[ModelMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| serde_json::json!({"role": message.role, "content": message.content}))
        .collect()
}

fn endpoint_url(provider: &str, custom: Option<&str>) -> Result<String, String> {
    let default_url = match provider {
        "Ollama" => "http://localhost:11434/api/chat",
        "OpenRouter" => "https://openrouter.ai/api/v1/chat/completions",
        "Google AI Studio" => {
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        }
        _ => return Err(format!("Unsupported provider: {}", provider)),
    };
    let Some(url) = custom else {
        return Ok(default_url.to_string());
    };
    let parsed = reqwest::Url::parse(url).map_err(|_| format!("Invalid endpoint URL: {}", url))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "Endpoint URL must include a host".to_string())?;
    let private_or_local = match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            let octets = ip.octets();
            ip.is_loopback()
                || ip.is_multicast()
                || octets[0] == 10
                || octets[0] == 127
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 169 && octets[1] == 254)
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            ip.is_loopback()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
        Err(_) => host == "localhost" || host.ends_with(".local"),
    };
    match parsed.scheme() {
        "http" if private_or_local && provider == "Ollama" => Ok(parsed.to_string()),
        "https" => match (private_or_local, provider == "Ollama") {
            (true, false) => {
                Err("Non-Ollama providers cannot use private or internal endpoints".to_string())
            }
            _ => Ok(parsed.to_string()),
        },
        _ => Err(format!(
            "Endpoint URL must use HTTPS; Ollama local endpoints may use HTTP. Provided: {}",
            parsed
        )),
    }
}

pub fn validated_endpoint_for_test(provider: &str, custom: Option<&str>) -> Result<String, String> {
    endpoint_url(provider, custom)
}

pub async fn query_llm(
    provider: &str,
    model_id: &str,
    messages: &[ModelMessage],
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        "Ollama" => {
            let url = endpoint_url(provider, base_url)?;
            let body = serde_json::json!({
                "model": model_id,
                "messages": messages_json(messages),
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
                return Err(format!(
                    "Ollama returned error: {}",
                    safe_error_body(&err_text)
                ));
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
            let url = endpoint_url(provider, base_url)?;
            let body = serde_json::json!({
                "model": model_id,
                "messages": messages_json(messages)
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
                return Err(format!(
                    "{} returned error: {}",
                    provider,
                    safe_error_body(&err_text)
                ));
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
    messages: &[ModelMessage],
    api_key: Option<&str>,
    base_url: Option<&str>,
    on_token: impl Fn(String) + Send + Sync + 'static,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        "Ollama" => {
            let url = endpoint_url(provider, base_url)?;
            let body = serde_json::json!({
                "model": model_id,
                "messages": messages_json(messages),
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
                return Err(format!(
                    "Ollama returned error: {}",
                    safe_error_body(&err_text)
                ));
            }
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                if accumulated.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err("Provider response exceeded the maximum allowed size".to_string());
                }
                let payload = String::from_utf8_lossy(&chunk);
                for line in payload.lines() {
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
            let url = endpoint_url(provider, base_url)?;
            let body = serde_json::json!({
                "model": model_id,
                "messages": messages_json(messages),
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
                return Err(format!(
                    "{} returned error: {}",
                    provider,
                    safe_error_body(&err_text)
                ));
            }
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            let mut buffer = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                if accumulated.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err("Provider response exceeded the maximum allowed size".to_string());
                }
                let payload = String::from_utf8_lossy(&chunk);
                buffer.push_str(&payload);
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

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
    let mut normalized: Vec<Value> = Vec::new();
    for (i, message) in messages.iter().enumerate() {
        if message.role == "assistant"
            && let Ok(val) = serde_json::from_str::<Value>(&message.content)
            && val.get("tool_calls").is_some()
        {
            normalized.push(val);
            continue;
        }

        let role = if message.role == "system" {
            if i == 0 {
                "system"
            } else {
                "user"
            }
        } else if message.role == "assistant" {
            "assistant"
        } else {
            "user"
        };

        if let Some(last) = normalized.last_mut()
            && last["role"] == role
            && last.get("tool_calls").is_none()
        {
            let existing = last["content"].as_str().unwrap_or("").to_string();
            last["content"] = serde_json::json!(format!("{}\n\n{}", existing, message.content));
            continue;
        }
        normalized.push(serde_json::json!({
            "role": role,
            "content": message.content
        }));
    }
    normalized
}

fn endpoint_url(provider: &str, custom: Option<&str>) -> Result<String, String> {
    let default_url = match provider {
        "Ollama" => "http://localhost:11434/api/chat",
        "OpenRouter" => "https://openrouter.ai/api/v1/chat/completions",
        "Google AI Studio" => {
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
        }
        "OpenAI Compatible" => "https://api.openai.com/v1/chat/completions",
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
    let normalized = if provider == "OpenAI Compatible" {
        let path = parsed.path().trim_end_matches('/');
        let mut modified = parsed.clone();
        if path.is_empty() || path == "/v1" {
            modified.set_path("/v1/chat/completions");
        } else if !path.ends_with("/chat/completions") {
            modified.set_path(&format!("{}/chat/completions", path));
        } else {
            modified.set_path(path);
        }
        modified
    } else {
        parsed
    };

    match normalized.scheme() {
        "http" if private_or_local && (provider == "Ollama" || provider == "OpenAI Compatible") => {
            Ok(normalized.to_string())
        }
        "https" if provider == "OpenAI Compatible" => Ok(normalized.to_string()),
        "https" => match (private_or_local, provider == "Ollama") {
            (true, false) => {
                Err("Non-Ollama providers cannot use private or internal endpoints".to_string())
            }
            _ => Ok(normalized.to_string()),
        },
        _ => Err(format!(
            "Endpoint URL must use HTTPS; Ollama and OpenAI Compatible local endpoints may use HTTP. Provided: {}",
            normalized
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
            let key_opt = api_key.filter(|k| !k.trim().is_empty());
            if key_opt.is_none() && provider != "OpenAI Compatible" {
                return Err(format!("API key is required for {}", provider));
            }
            let url = endpoint_url(provider, base_url)?;
            let body = serde_json::json!({
                "model": model_id,
                "messages": messages_json(messages)
            });

            let mut req = client
                .post(url)
                .header("content-type", "application/json")
                .json(&body);
            if let Some(key) = key_opt {
                req = req.header("Authorization", format!("Bearer {}", key));
            }

            let res = req
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
            let choice = &json["choices"][0];
            if let Some(content) = choice["message"]["content"].as_str().filter(|s| !s.is_empty()) {
                Ok(content.to_string())
            } else if let Some(tool_calls) = choice["message"]["tool_calls"].as_array().filter(|a| !a.is_empty()) {
                let mut formatted = String::new();
                for (idx, tc) in tool_calls.iter().enumerate() {
                    let call_id = tc["id"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("call_{}", idx + 1));
                    let name = tc["function"]["name"].as_str().unwrap_or("");
                    let args = &tc["function"]["arguments"];
                    let parsed_args = if let Some(s) = args.as_str() {
                        serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        args.clone()
                    };
                    formatted.push_str(&format!(
                        "<tool_call>\n{}\n</tool_call>\n",
                        serde_json::json!({
                            "id": call_id,
                            "name": name,
                            "arguments": parsed_args
                        })
                    ));
                }
                Ok(formatted)
            } else {
                Err("Provider returned empty response choices without text or tool calls.".to_string())
            }
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
    query_llm_stream_with_tools(provider, model_id, messages, api_key, base_url, &[], on_token).await
}

pub async fn query_llm_stream_with_tools(
    provider: &str,
    model_id: &str,
    messages: &[ModelMessage],
    api_key: Option<&str>,
    base_url: Option<&str>,
    tools: &[Value],
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
            let mut body_map = serde_json::Map::new();
            body_map.insert("model".to_string(), serde_json::json!(model_id));
            body_map.insert("messages".to_string(), serde_json::json!(messages_json(messages)));
            body_map.insert("stream".to_string(), serde_json::json!(true));
            if !tools.is_empty() {
                body_map.insert("tools".to_string(), serde_json::json!(tools));
            }
            let body = Value::Object(body_map);
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
            let mut buffer = String::new();
            let mut native_calls = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                if accumulated.len() + buffer.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err("Provider response exceeded the maximum allowed size".to_string());
                }
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer.drain(..=pos);
                    if line.is_empty() { continue; }
                    if let Ok(json) = serde_json::from_str::<Value>(&line) {
                        if let Some(content) = json["message"]["content"].as_str().filter(|s| !s.is_empty()) {
                            accumulated.push_str(content);
                            on_token(content.to_string());
                        }
                        if let Some(calls) = json["message"]["tool_calls"].as_array() {
                            native_calls.extend(calls.iter().cloned());
                        }
                    }
                }
            }
            if !buffer.trim().is_empty()
                && let Ok(json) = serde_json::from_str::<Value>(buffer.trim())
            {
                if let Some(content) = json["message"]["content"].as_str().filter(|s| !s.is_empty()) {
                    accumulated.push_str(content);
                    on_token(content.to_string());
                }
                if let Some(calls) = json["message"]["tool_calls"].as_array() {
                    native_calls.extend(calls.iter().cloned());
                }
            }
            for (idx, call) in native_calls.iter().enumerate() {
                if let Some(parsed) = super::harness::ToolDispatcher::native_tool_call(call, idx + 1) {
                    accumulated.push_str(&format!(
                        "<tool_call>\n{}\n</tool_call>\n",
                        serde_json::json!({
                            "id": parsed.id,
                            "name": parsed.name,
                            "arguments": parsed.arguments
                        })
                    ));
                }
            }
            if accumulated.is_empty() {
                Err("Ollama stream completed without text or tool calls.".to_string())
            } else {
                Ok(accumulated)
            }
        }
        _ => {
            let key_opt = api_key.filter(|k| !k.trim().is_empty());
            if key_opt.is_none() && provider != "OpenAI Compatible" {
                return Err(format!("API key is required for {}", provider));
            }
            let url = endpoint_url(provider, base_url)?;
            let mut body_map = serde_json::Map::new();
            body_map.insert("model".to_string(), serde_json::json!(model_id));
            body_map.insert("messages".to_string(), serde_json::json!(messages_json(messages)));
            body_map.insert("stream".to_string(), serde_json::json!(true));
            if !tools.is_empty() {
                body_map.insert("tools".to_string(), serde_json::json!(tools));
            }
            let body = Value::Object(body_map);
            let mut req = client
                .post(url)
                .header("content-type", "application/json")
                .json(&body);
            if let Some(key) = key_opt {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
            let res = req
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
            let mut stream_error = None;
            let mut tool_accumulator: std::collections::BTreeMap<usize, (String, String, String)> =
                std::collections::BTreeMap::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                if accumulated.len() + buffer.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err("Provider response exceeded the maximum allowed size".to_string());
                }
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].to_string();
                    buffer.drain(..=pos);
                    let line = line.trim();
                    if !line.starts_with("data:") { continue; }
                    let data = line[5..].trim();
                    if data == "[DONE]" { continue; }
                    if let Ok(json) = serde_json::from_str::<Value>(data) {
                        if let Some(content) = json["choices"][0]["delta"]["content"].as_str().filter(|s| !s.is_empty()) {
                            accumulated.push_str(content);
                            on_token(content.to_string());
                            continue;
                        }
                        if let Some(content) = json["choices"][0]["message"]["content"].as_str().filter(|s| !s.is_empty()) {
                            accumulated.push_str(content);
                            on_token(content.to_string());
                            continue;
                        }
                        let tool_calls = json["choices"][0]["delta"]["tool_calls"]
                            .as_array()
                            .or_else(|| json["choices"][0]["message"]["tool_calls"].as_array());
                        if let Some(tool_calls) = tool_calls.filter(|calls| !calls.is_empty()) {
                            for (fallback_idx, tc) in tool_calls.iter().enumerate() {
                                let index = tc.get("index")
                                    .and_then(|v| v.as_u64())
                                    .map(|v| v as usize)
                                    .unwrap_or(fallback_idx);
                                let entry = tool_accumulator.entry(index).or_insert_with(|| (String::new(), String::new(), String::new()));
                                if let Some(id) = tc.get("id").and_then(|v| v.as_str())
                                    && !id.is_empty()
                                {
                                    entry.0 = id.to_string();
                                }
                                if let Some(name) = tc.pointer("/function/name").and_then(|v| v.as_str())
                                    && !name.is_empty()
                                {
                                    entry.1 = name.to_string();
                                }
                                if let Some(args_chunk) = tc.pointer("/function/arguments").and_then(|v| v.as_str()) {
                                    entry.2.push_str(args_chunk);
                                } else if let Some(args_obj) = tc.pointer("/function/arguments").and_then(|v| v.as_object()) {
                                    entry.2 = serde_json::to_string(args_obj).unwrap_or_default();
                                }
                            }
                            continue;
                        }
                        if let Some(reason) = json["choices"][0]["finish_reason"].as_str() {
                            if reason == "length" {
                                stream_error = Some("Model response was truncated (finish_reason=length). Retry with a shorter prompt.".to_string());
                            } else if reason == "content_filter" {
                                stream_error = Some("Model response was blocked by the provider's content filter.".to_string());
                            }
                        }
                        if let Some(error) = json.get("error") {
                            stream_error = Some(format!("{} returned error: {}", provider, safe_error_body(&error.to_string())));
                        }
                    }
                }
            }
            if !tool_accumulator.is_empty() {
                for (idx, (id, name, arguments)) in tool_accumulator {
                    let call_id = if id.is_empty() {
                        format!("call_{}", idx + 1)
                    } else {
                        id
                    };
                    let parsed_args = if arguments.trim().is_empty() {
                        serde_json::json!({})
                    } else {
                        serde_json::from_str::<Value>(&arguments).unwrap_or_else(|_| {
                            serde_json::json!({ "raw": arguments })
                        })
                    };
                    accumulated.push_str(&format!(
                        "<tool_call>\n{}\n</tool_call>\n",
                        serde_json::json!({
                            "id": call_id,
                            "name": name,
                            "arguments": parsed_args
                        })
                    ));
                }
            }
            if let Some(error) = stream_error { return Err(error); }
            if !buffer.trim().is_empty() {
                let line = buffer.trim();
                if let Some(data) = line.strip_prefix("data:")
                && data.trim() != "[DONE]"
                && let Ok(json) = serde_json::from_str::<Value>(data.trim())
                && let Some(content) = json["choices"][0]["delta"]["content"].as_str().filter(|s| !s.is_empty())
            {
                accumulated.push_str(content);
                on_token(content.to_string());
            }
            }
            if accumulated.is_empty() {
                Err("Provider stream completed without text. The model may have returned tool calls or an unsupported response format.".to_string())
            } else {
                Ok(accumulated)
            }
        }
    }
}

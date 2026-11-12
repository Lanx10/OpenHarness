use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
    pub attachments: Vec<String>,
}

pub struct ModelRequest<'a> {
    pub provider: &'a str,
    pub model_id: &'a str,
    pub messages: &'a [ModelMessage],
    pub api_key: Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub max_output_tokens: u32,
    pub attachments: &'a [String],
    pub supports_vision: bool,
}

pub struct StreamingModelRequest<'a> {
    pub request: ModelRequest<'a>,
    pub tools: &'a [Value],
}

pub struct ModelCallbacks<Token, Usage, Activity = fn(String)> {
    pub on_token: Token,
    pub on_usage: Usage,
    pub on_activity: Activity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub provider: String,
    pub model_id: String,
    pub base_url: Option<String>,
    pub custom_endpoint: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_context_tokens")]
    pub max_context_tokens: u32,
    #[serde(default = "default_output_tokens")]
    pub max_output_tokens: u32,
    pub supports_vision: bool,
}

fn default_context_tokens() -> u32 {
    128_000
}

fn default_output_tokens() -> u32 {
    4_096
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
            attachments: Vec::new(),
        }
    }

    pub fn with_attachments(mut self, attachments: &[String]) -> Self {
        self.attachments = attachments.to_vec();
        self
    }
}

const MAX_ERROR_BODY_CHARS: usize = 512;
const MAX_RESPONSE_BYTES: usize = 1_000_000;

pub fn is_image_attachment(attachment: &str) -> bool {
    attachment.starts_with("![") && attachment.contains("](data:image/")
}

const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;

fn attachment_data_url(attachment: &str) -> Result<Option<&str>, String> {
    let url = if attachment.starts_with("![") {
        let Some(start) = attachment.find("](") else {
            return Err("Malformed image attachment: expected Markdown data URL".into());
        };
        let start = start + 2;
        let Some(end) = attachment[start..].find(')') else {
            return Err("Malformed image attachment: missing closing parenthesis".into());
        };
        &attachment[start..start + end]
    } else if attachment.starts_with("data:image/") {
        attachment
    } else {
        return Ok(None);
    };
    if !url.starts_with("data:image/") {
        return Err("Image attachment must use a base64 data URL".into());
    }
    Ok(Some(url))
}

fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn image_data_urls(attachments: &[String]) -> Result<Vec<String>, String> {
    attachments
        .iter()
        .map(|attachment| {
            let Some(url) = attachment_data_url(attachment)? else {
                return Ok(None);
            };
            let (header, data) = url
                .split_once(',')
                .ok_or("Malformed image attachment: missing data separator")?;
            let mime = header
                .strip_prefix("data:")
                .and_then(|value| value.strip_suffix(";base64"))
                .ok_or("Malformed image attachment: expected base64 encoding")?;
            if !matches!(
                mime,
                "image/png" | "image/jpeg" | "image/webp" | "image/gif"
            ) {
                return Err(format!(
                    "Unsupported image format: {mime}. Use PNG, JPEG, WebP, or GIF."
                ));
            }
            if data.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
                return Err("Image attachment exceeds the 4 MB limit".into());
            }
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
                .map_err(|_| "Invalid image attachment: malformed base64 data".to_string())?;
            if bytes.is_empty() {
                return Err("Invalid image attachment: image data is empty".into());
            }
            if bytes.len() > MAX_IMAGE_BYTES {
                return Err("Image attachment exceeds the 4 MB limit".into());
            }
            if sniff_image_mime(&bytes) != Some(mime) {
                return Err("Image MIME type does not match its file content".into());
            }
            Ok(Some(url.to_string()))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|items| items.into_iter().flatten().collect())
}

fn attach_images_to_user(
    payload: &mut [Value],
    images: &[String],
    ollama: bool,
) -> Result<(), String> {
    if images.is_empty() {
        return Ok(());
    }
    let user = payload
        .iter_mut()
        .rev()
        .find(|message| message["role"] == "user")
        .ok_or_else(|| "Image attachments require a current user message".to_string())?;
    if ollama {
        user["images"] = serde_json::json!(
            images
                .iter()
                .filter_map(|image| image.split_once(',').map(|(_, data)| data.to_string()))
                .collect::<Vec<_>>()
        );
    } else {
        let prompt = user["content"].as_str().unwrap_or("").to_string();
        let mut content = vec![serde_json::json!({"type": "text", "text": prompt})];
        content.extend(
            images
                .iter()
                .map(|image| serde_json::json!({"type": "image_url", "image_url": {"url": image}})),
        );
        user["content"] = serde_json::json!(content);
    }
    Ok(())
}

fn safe_error_body(body: &str) -> String {
    body.chars().take(MAX_ERROR_BODY_CHARS).collect()
}

fn add_response_bytes(total: &mut usize, chunk_len: usize) -> Result<(), String> {
    *total = total
        .checked_add(chunk_len)
        .filter(|size| *size <= MAX_RESPONSE_BYTES)
        .ok_or_else(|| "Provider response exceeded the maximum allowed size".to_string())?;
    Ok(())
}

async fn read_capped_body(response: reqwest::Response) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("Provider response exceeded the maximum allowed size".to_string());
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    let mut total = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        add_response_bytes(&mut total, chunk.len())?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn read_capped_error(response: reqwest::Response) -> String {
    match read_capped_body(response).await {
        Ok(body) => safe_error_body(&String::from_utf8_lossy(&body)),
        Err(_) => "Error response exceeded the maximum allowed size".to_string(),
    }
}

#[cfg(test)]
fn messages_json(messages: &[ModelMessage]) -> Vec<Value> {
    messages_json_checked(messages, false).expect("valid test fixture")
}

fn messages_json_checked(messages: &[ModelMessage], ollama: bool) -> Result<Vec<Value>, String> {
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
            if i == 0 { "system" } else { "user" }
        } else if message.role == "assistant" {
            "assistant"
        } else {
            "user"
        };

        let mut item = serde_json::json!({"role": role, "content": message.content});
        if !message.attachments.is_empty() {
            let images = image_data_urls(&message.attachments)?;
            if ollama {
                item["images"] = serde_json::json!(
                    images
                        .iter()
                        .filter_map(|image| {
                            image.split_once(',').map(|(_, data)| data.to_string())
                        })
                        .collect::<Vec<_>>()
                );
            } else {
                let mut content =
                    vec![serde_json::json!({"type": "text", "text": message.content})];
                content.extend(images.into_iter().map(|image| {
                    serde_json::json!({
                        "type": "image_url", "image_url": {"url": image}
                    })
                }));
                item["content"] = serde_json::Value::Array(content);
            }
            normalized.push(item);
        } else if let Some(last) = normalized.last_mut()
            && last["role"] == role
            && last.get("tool_calls").is_none()
            && last["content"].is_string()
        {
            let existing = last["content"].as_str().unwrap_or("").to_string();
            last["content"] = serde_json::json!(format!("{}\n\n{}", existing, message.content));
        } else {
            normalized.push(item);
        }
    }
    Ok(normalized)
}

fn private_ip(ip: std::net::IpAddr) -> bool {
    let private_v4 = |ip: std::net::Ipv4Addr| {
        let [a, b, ..] = ip.octets();
        ip.is_loopback()
            || ip.is_link_local()
            || ip.is_multicast()
            || ip.is_unspecified()
            || a == 10
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
    };
    match ip {
        std::net::IpAddr::V4(ip) => private_v4(ip),
        std::net::IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                private_v4(ip)
            } else {
                ip.is_loopback()
                    || ip.is_multicast()
                    || ip.is_unique_local()
                    || ip.is_unicast_link_local()
                    || ip.is_unspecified()
            }
        }
    }
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
    let host_ip = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::IpAddr>();
    let private_or_local = match host_ip {
        Ok(ip) => private_ip(ip),
        Err(_) => {
            host == "localhost"
                || host.ends_with(".localhost")
                || host.ends_with(".local")
                || host.ends_with(".internal")
                || host.ends_with(".test")
                || host.ends_with(".invalid")
        }
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

pub async fn query_llm(request: ModelRequest<'_>) -> Result<String, String> {
    let ModelRequest {
        provider,
        model_id,
        messages,
        api_key,
        base_url,
        max_output_tokens,
        attachments,
        supports_vision,
    } = request;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        "Ollama" => {
            let images = image_data_urls(attachments)?;
            if !images.is_empty() && !supports_vision {
                return Err("Model is not configured for image input".into());
            }
            let url = endpoint_url(provider, base_url)?;
            let mut payload = messages_json_checked(messages, provider == "Ollama")?;
            attach_images_to_user(&mut payload, &images, true)?;
            let body = serde_json::json!({
                "model": model_id,
                "messages": payload,
                "stream": false,
                "options": {"num_predict": max_output_tokens}
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
                let error = read_capped_error(res).await;
                return Err(format!("Ollama returned error: {}", error));
            }

            let body = read_capped_body(res).await?;
            let json: Value = serde_json::from_slice(&body)
                .map_err(|e| format!("Failed to parse Ollama JSON: {}", e))?;
            let content = json["message"]["content"]
                .as_str()
                .unwrap_or("No reply from Ollama");
            Ok(content.to_string())
        }
        "OpenRouter" | "Google AI Studio" | "OpenAI Compatible" => {
            let images = image_data_urls(attachments)?;
            if !images.is_empty() && !supports_vision {
                return Err("Model is not configured for image input".into());
            }
            let key_opt = api_key.filter(|k| !k.trim().is_empty());
            if key_opt.is_none() && provider != "OpenAI Compatible" {
                return Err(format!("API key is required for {}", provider));
            }
            let url = endpoint_url(provider, base_url)?;
            let mut payload = messages_json_checked(messages, provider == "Ollama")?;
            attach_images_to_user(&mut payload, &images, false)?;
            let body = serde_json::json!({
                "model": model_id,
                "messages": payload,
                "max_tokens": max_output_tokens
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
                let error = read_capped_error(res).await;
                return Err(format!("{} returned error: {}", provider, error));
            }

            let body = read_capped_body(res).await?;
            let json: Value = serde_json::from_slice(&body)
                .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
            let choice = &json["choices"][0];
            if let Some(content) = choice["message"]["content"]
                .as_str()
                .filter(|s| !s.is_empty())
            {
                Ok(content.to_string())
            } else if let Some(tool_calls) = choice["message"]["tool_calls"]
                .as_array()
                .filter(|a| !a.is_empty())
            {
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
                Err(
                    "Provider returned empty response choices without text or tool calls."
                        .to_string(),
                )
            }
        }
        other => Err(format!("Unsupported provider: {}", other)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ProviderUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    #[serde(default)]
    pub total_is_derived: bool,
}

fn usage_count(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64)
}

fn first_count(usage: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| usage.pointer(key).and_then(Value::as_u64))
}

fn openai_usage(usage: &Value) -> Option<ProviderUsage> {
    let input_tokens = first_count(usage, &["/prompt_tokens", "/input_tokens"]);
    let output_tokens = first_count(usage, &["/completion_tokens", "/output_tokens"]);
    let total_tokens = usage_count(usage.get("total_tokens"));
    let cached_input_tokens = first_count(
        usage,
        &[
            "/prompt_tokens_details/cached_tokens",
            "/input_tokens_details/cached_tokens",
            "/prompt_token_count_details/cached_tokens",
        ],
    );
    let reasoning_tokens = first_count(
        usage,
        &[
            "/completion_tokens_details/reasoning_tokens",
            "/output_tokens_details/reasoning_tokens",
        ],
    );
    (input_tokens.is_some()
        || output_tokens.is_some()
        || total_tokens.is_some()
        || cached_input_tokens.is_some()
        || reasoning_tokens.is_some())
    .then_some(ProviderUsage {
        input_tokens,
        output_tokens,
        total_tokens,
        cached_input_tokens,
        reasoning_tokens,
        total_is_derived: false,
    })
}

fn google_usage(usage: &Value) -> Option<ProviderUsage> {
    let usage = usage.get("usageMetadata").unwrap_or(usage);
    let input_tokens = first_count(usage, &["/promptTokenCount"]);
    let output_tokens = first_count(usage, &["/candidatesTokenCount"]);
    let total_tokens = first_count(usage, &["/totalTokenCount"]);
    let cached_input_tokens = first_count(usage, &["/cachedContentTokenCount"]);
    let reasoning_tokens = first_count(usage, &["/thoughtsTokenCount"]);
    (input_tokens.is_some()
        || output_tokens.is_some()
        || total_tokens.is_some()
        || cached_input_tokens.is_some()
        || reasoning_tokens.is_some())
    .then_some(ProviderUsage {
        input_tokens,
        output_tokens,
        total_tokens,
        cached_input_tokens,
        reasoning_tokens,
        total_is_derived: false,
    })
}

fn ollama_usage(usage: &Value) -> Option<ProviderUsage> {
    let input_tokens = usage_count(usage.get("prompt_eval_count"));
    let output_tokens = usage_count(usage.get("eval_count"));
    (input_tokens.is_some() || output_tokens.is_some()).then_some(ProviderUsage {
        input_tokens,
        output_tokens,
        total_tokens: input_tokens
            .zip(output_tokens)
            .map(|(input, output)| input.saturating_add(output)),
        cached_input_tokens: None,
        reasoning_tokens: None,
        total_is_derived: input_tokens.is_some() && output_tokens.is_some(),
    })
}

fn provider_usage(provider: &str, usage: &Value) -> Option<ProviderUsage> {
    match provider {
        "Ollama" => ollama_usage(usage),
        "Google AI Studio" => google_usage(usage),
        "OpenRouter" | "OpenAI Compatible" => openai_usage(usage),
        _ => None,
    }
}

fn with_derived_total(mut usage: ProviderUsage) -> ProviderUsage {
    if usage.total_tokens.is_none() {
        usage.total_tokens = usage
            .input_tokens
            .zip(usage.output_tokens)
            .map(|(input, output)| input.saturating_add(output));
        usage.total_is_derived = usage.total_tokens.is_some();
    }
    usage
}

fn provider_usage_envelope(provider: &str, usage: &Value) -> Option<ProviderUsage> {
    provider_usage(provider, usage).map(with_derived_total)
}

#[cfg(test)]
fn normalize_test_usage(provider: &str, usage: &Value) -> Option<ProviderUsage> {
    provider_usage_envelope(provider, usage)
}

#[cfg(test)]
fn cached_input_tokens(usage: &Value) -> Option<u64> {
    openai_usage(usage)
        .or_else(|| google_usage(usage))?
        .cached_input_tokens
}

pub async fn query_llm_stream(
    request: ModelRequest<'_>,
    callbacks: ModelCallbacks<
        impl Fn(String) + Send + Sync + 'static,
        impl Fn(ProviderUsage) + Send + Sync + 'static,
    >,
) -> Result<String, String> {
    let ModelCallbacks {
        on_token, on_usage, ..
    } = callbacks;
    query_llm_stream_with_tools(
        StreamingModelRequest {
            request,
            tools: &[],
        },
        ModelCallbacks {
            on_token,
            on_usage,
            on_activity: |_| {},
        },
    )
    .await
}

pub async fn query_llm_stream_with_tools(
    request: StreamingModelRequest<'_>,
    callbacks: ModelCallbacks<
        impl Fn(String) + Send + Sync + 'static,
        impl Fn(ProviderUsage) + Send + Sync + 'static,
        impl Fn(String) + Send + Sync + 'static,
    >,
) -> Result<String, String> {
    let StreamingModelRequest { request, tools } = request;
    let ModelRequest {
        provider,
        model_id,
        messages,
        api_key,
        base_url,
        max_output_tokens,
        attachments,
        supports_vision,
    } = request;
    let ModelCallbacks {
        on_token,
        on_usage,
        on_activity,
    } = callbacks;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        "Ollama" => {
            let images = image_data_urls(attachments)?;
            if !images.is_empty() && !supports_vision {
                return Err("Model is not configured for image input".into());
            }
            let url = endpoint_url(provider, base_url)?;
            let mut payload = messages_json_checked(messages, provider == "Ollama")?;
            if !images.is_empty()
                && let Some(last) = payload
                    .last_mut()
                    .filter(|message| message["role"] == "user")
            {
                last["images"] = serde_json::json!(
                    images
                        .iter()
                        .filter_map(|image| image.split_once(',').map(|(_, data)| data.to_string()))
                        .collect::<Vec<_>>()
                );
            }
            let mut body_map = serde_json::Map::new();
            body_map.insert("model".to_string(), serde_json::json!(model_id));
            body_map.insert("messages".to_string(), serde_json::json!(payload));
            body_map.insert("stream".to_string(), serde_json::json!(true));
            body_map.insert(
                "options".to_string(),
                serde_json::json!({"num_predict": max_output_tokens}),
            );
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
                let error = read_capped_error(res).await;
                return Err(format!("Ollama returned error: {}", error));
            }
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            let mut buffer = String::new();
            let mut native_calls = Vec::new();
            let mut received_bytes = 0;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                add_response_bytes(&mut received_bytes, chunk.len())?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer.drain(..=pos);
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<Value>(&line) {
                        if let Some(content) = json["message"]["content"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                        {
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
                if let Some(content) = json["message"]["content"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                {
                    accumulated.push_str(content);
                    on_token(content.to_string());
                }
                if let Some(calls) = json["message"]["tool_calls"].as_array() {
                    native_calls.extend(calls.iter().cloned());
                }
            }
            for (idx, call) in native_calls.iter().enumerate() {
                if let Some(parsed) =
                    super::harness::ToolDispatcher::native_tool_call(call, idx + 1)
                {
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
        "OpenRouter" | "Google AI Studio" | "OpenAI Compatible" => {
            let images = image_data_urls(attachments)?;
            if !images.is_empty() && !supports_vision {
                return Err("Model is not configured for image input".into());
            }
            let key_opt = api_key.filter(|k| !k.trim().is_empty());
            if key_opt.is_none() && provider != "OpenAI Compatible" {
                return Err(format!("API key is required for {}", provider));
            }
            let url = endpoint_url(provider, base_url)?;
            let mut payload = messages_json_checked(messages, provider == "Ollama")?;
            attach_images_to_user(&mut payload, &images, false)?;
            let mut body_map = serde_json::Map::new();
            body_map.insert("model".to_string(), serde_json::json!(model_id));
            body_map.insert("messages".to_string(), serde_json::json!(payload));
            body_map.insert("stream".to_string(), serde_json::json!(true));
            body_map.insert(
                "stream_options".to_string(),
                serde_json::json!({"include_usage": true}),
            );
            body_map.insert(
                "max_tokens".to_string(),
                serde_json::json!(max_output_tokens),
            );
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
                let error = read_capped_error(res).await;
                return Err(format!("{} returned error: {}", provider, error));
            }
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            let mut buffer = String::new();
            let mut stream_error = None;
            let mut tool_accumulator: std::collections::BTreeMap<usize, (String, String, String)> =
                std::collections::BTreeMap::new();
            let mut received_bytes = 0;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                add_response_bytes(&mut received_bytes, chunk.len())?;
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
                    if let Ok(json) = serde_json::from_str::<Value>(data) {
                        if let Some(raw_usage) =
                            json.get("usage").or_else(|| json.get("usageMetadata"))
                            && let Some(usage) = provider_usage_envelope(provider, raw_usage)
                        {
                            on_usage(usage);
                        } else if provider == "Google AI Studio"
                            && let Some(usage) = provider_usage_envelope(provider, &json)
                        {
                            on_usage(usage);
                        } else if provider == "Ollama"
                            && let Some(usage) = provider_usage_envelope(provider, &json)
                        {
                            on_usage(usage);
                        }
                        if let Some(reasoning) = json["choices"][0]["delta"]["reasoning"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                        {
                            on_activity(format!("[agent-step]{}", reasoning));
                        }
                        if let Some(content) = json["choices"][0]["delta"]["content"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                        {
                            accumulated.push_str(content);
                            on_token(content.to_string());
                            continue;
                        }
                        if let Some(content) = json["choices"][0]["message"]["content"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                        {
                            accumulated.push_str(content);
                            on_token(content.to_string());
                            continue;
                        }
                        let tool_calls = json["choices"][0]["delta"]["tool_calls"]
                            .as_array()
                            .or_else(|| json["choices"][0]["message"]["tool_calls"].as_array());
                        if let Some(tool_calls) = tool_calls.filter(|calls| !calls.is_empty()) {
                            for (fallback_idx, tc) in tool_calls.iter().enumerate() {
                                let index = tc
                                    .get("index")
                                    .and_then(|v| v.as_u64())
                                    .map(|v| v as usize)
                                    .unwrap_or(fallback_idx);
                                let entry = tool_accumulator.entry(index).or_insert_with(|| {
                                    (String::new(), String::new(), String::new())
                                });
                                if let Some(id) = tc.get("id").and_then(|v| v.as_str())
                                    && !id.is_empty()
                                {
                                    entry.0 = id.to_string();
                                }
                                if let Some(name) =
                                    tc.pointer("/function/name").and_then(|v| v.as_str())
                                    && !name.is_empty()
                                {
                                    entry.1 = name.to_string();
                                }
                                if let Some(args_chunk) =
                                    tc.pointer("/function/arguments").and_then(|v| v.as_str())
                                {
                                    entry.2.push_str(args_chunk);
                                } else if let Some(args_obj) = tc
                                    .pointer("/function/arguments")
                                    .and_then(|v| v.as_object())
                                {
                                    entry.2 = serde_json::to_string(args_obj).unwrap_or_default();
                                }
                            }
                            continue;
                        }
                        if let Some(reason) = json["choices"][0]["finish_reason"].as_str() {
                            if reason == "length" {
                                stream_error = Some("Model response was truncated (finish_reason=length). Retry with a shorter prompt.".to_string());
                            } else if reason == "content_filter" {
                                stream_error = Some(
                                    "Model response was blocked by the provider's content filter."
                                        .to_string(),
                                );
                            }
                        }
                        if let Some(error) = json.get("error") {
                            stream_error = Some(format!(
                                "{} returned error: {}",
                                provider,
                                safe_error_body(&error.to_string())
                            ));
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
                        serde_json::from_str::<Value>(&arguments)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": arguments }))
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
            if let Some(error) = stream_error {
                return Err(error);
            }
            if !buffer.trim().is_empty() {
                let line = buffer.trim();
                if let Some(data) = line.strip_prefix("data:")
                    && data.trim() != "[DONE]"
                    && let Ok(json) = serde_json::from_str::<Value>(data.trim())
                    && let Some(content) = json["choices"][0]["delta"]["content"]
                        .as_str()
                        .filter(|s| !s.is_empty())
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
        other => Err(format!("Unsupported provider: {}", other)),
    }
}

#[cfg(test)]
mod image_payload_tests {
    use super::{
        ModelMessage, attach_images_to_user, image_data_urls, is_image_attachment, messages_json,
        messages_json_checked,
    };

    #[test]
    #[ignore = "manual release-mode microbenchmark"]
    fn benchmark_context_message_serialization() {
        use std::hint::black_box;
        use std::time::Instant;

        let messages = (0..128)
            .map(|index| {
                ModelMessage::new(
                    "user",
                    &format!("Context message {index}: {}", "x".repeat(512)),
                )
            })
            .collect::<Vec<_>>();
        let started = Instant::now();
        for _ in 0..1_000 {
            black_box(messages_json(black_box(&messages)));
        }
        eprintln!("messages_json: 1000 iterations, {:?}", started.elapsed());
    }

    #[test]
    fn rejects_private_ip_literals_and_private_dns_names() {
        for endpoint in [
            "https://127.0.0.1/v1",
            "https://169.254.169.254/latest/meta-data",
            "https://[::ffff:127.0.0.1]/v1",
        ] {
            assert!(
                super::validated_endpoint_for_test("OpenRouter", Some(endpoint)).is_err(),
                "accepted private endpoint {endpoint}"
            );
        }
        assert!(
            super::validated_endpoint_for_test("OpenRouter", Some("https://api.openrouter.ai/v1"))
                .is_ok()
        );
    }

    #[test]
    fn counts_streamed_response_bytes_without_overflow() {
        let mut total = super::MAX_RESPONSE_BYTES - 1;
        assert!(super::add_response_bytes(&mut total, 1).is_ok());
        assert_eq!(total, super::MAX_RESPONSE_BYTES);
        assert!(super::add_response_bytes(&mut total, 1).is_err());

        let mut total = usize::MAX;
        assert!(super::add_response_bytes(&mut total, 1).is_err());
    }

    #[test]
    fn provider_config_uses_camel_case_for_tauri_invoke_payloads() {
        let config = super::ProviderConfig {
            provider: "OpenRouter".into(),
            model_id: "model-x".into(),
            base_url: None,
            custom_endpoint: false,
            api_key: "secret".into(),
            max_context_tokens: 128_000,
            max_output_tokens: 4_096,
            supports_vision: true,
        };
        let value = serde_json::to_value(config).unwrap();
        assert_eq!(value["modelId"], "model-x");
        assert_eq!(value["maxContextTokens"], 128_000);
        assert_eq!(value["supportsVision"], true);
        assert!(value.get("model_id").is_none());
    }

    #[test]
    fn normalizes_openai_compatible_usage() {
        let usage = serde_json::json!({
            "prompt_tokens": 1_200,
            "completion_tokens": 300,
            "total_tokens": 1_500,
            "prompt_tokens_details": {"cached_tokens": 800},
            "completion_tokens_details": {"reasoning_tokens": 25}
        });
        assert_eq!(
            super::provider_usage_envelope("OpenRouter", &usage),
            Some(super::ProviderUsage {
                input_tokens: Some(1_200),
                output_tokens: Some(300),
                total_tokens: Some(1_500),
                cached_input_tokens: Some(800),
                reasoning_tokens: Some(25),
                total_is_derived: false,
            })
        );
        assert_eq!(
            super::provider_usage_envelope("OpenAI Compatible", &usage)
                .unwrap()
                .total_tokens,
            Some(1_500)
        );
    }

    #[test]
    fn normalizes_ollama_final_chunk_counts() {
        let usage = serde_json::json!({"prompt_eval_count": 21, "eval_count": 9});
        assert_eq!(
            super::provider_usage_envelope("Ollama", &usage),
            Some(super::ProviderUsage {
                input_tokens: Some(21),
                output_tokens: Some(9),
                total_tokens: Some(30),
                cached_input_tokens: None,
                reasoning_tokens: None,
                total_is_derived: true,
            })
        );
    }

    #[test]
    fn usage_parser_preserves_missing_and_rejects_malformed_counts() {
        assert_eq!(
            super::provider_usage_envelope(
                "OpenRouter",
                &serde_json::json!({"prompt_tokens": "12", "completion_tokens": -1})
            ),
            None
        );
        assert_eq!(
            super::provider_usage_envelope("Ollama", &serde_json::json!({"prompt_eval_count": 4}))
                .unwrap()
                .total_tokens,
            None
        );
    }

    #[test]
    fn reads_cached_input_tokens_from_openai_usage() {
        let usage = serde_json::json!({
            "prompt_tokens": 120,
            "prompt_tokens_details": {"cached_tokens": 80}
        });
        assert_eq!(super::cached_input_tokens(&usage), Some(80));
    }

    #[test]
    fn reads_google_usage_metadata_from_stream_shape() {
        let usage = serde_json::json!({
            "promptTokenCount": 1_200,
            "candidatesTokenCount": 300,
            "totalTokenCount": 1_500,
            "cachedContentTokenCount": 800,
            "thoughtsTokenCount": 12
        });
        assert_eq!(
            super::provider_usage("Google AI Studio", &usage),
            Some(super::ProviderUsage {
                input_tokens: Some(1_200),
                output_tokens: Some(300),
                total_tokens: Some(1_500),
                cached_input_tokens: Some(800),
                reasoning_tokens: Some(12),
                total_is_derived: false,
            })
        );
    }

    #[test]
    fn reads_cached_input_tokens_from_google_usage() {
        let usage = serde_json::json!({
            "usageMetadata": {"cachedContentTokenCount": 80}
        });
        assert_eq!(super::cached_input_tokens(&usage), Some(80));
    }

    #[test]
    fn reads_cached_input_tokens_from_google_usage_nested_in_envelope() {
        let response = serde_json::json!({
            "usage": {"usageMetadata": {"cachedContentTokenCount": 7_000}}
        });
        assert_eq!(super::cached_input_tokens(&response["usage"]), Some(7_000));
    }

    fn data_url(mime: &str, bytes: &[u8]) -> String {
        use base64::Engine;
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    #[test]
    fn accepts_supported_image_signatures() {
        let cases: [(&str, &[u8]); 4] = [
            (
                "image/png",
                &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            ),
            ("image/jpeg", &[0xff, 0xd8, 0xff]),
            (
                "image/webp",
                &[b'R', b'I', b'F', b'F', 0, 0, 0, 0, b'W', b'E', b'B', b'P'],
            ),
            ("image/gif", b"GIF89a"),
        ];
        for (mime, bytes) in cases {
            let url = data_url(mime, bytes);
            assert_eq!(
                image_data_urls(&[format!("![sample]({url})")]).unwrap(),
                vec![url]
            );
        }
    }

    #[test]
    fn rejects_svg_invalid_mime_base64_signature_empty_and_oversized_images() {
        for attachment in [
            format!("![svg]({})", data_url("image/svg+xml", b"<svg/>")),
            format!("![wrong mime]({})", data_url("image/jpeg", b"GIF89a")),
            "![bad](data:image/png;base64,not_base64!!!)".to_string(),
            "![empty](data:image/png;base64,)".to_string(),
            format!("![huge](data:image/png;base64,{})", "A".repeat(5_592_408)),
        ] {
            assert!(image_data_urls(&[attachment]).is_err());
        }
        assert!(
            image_data_urls(&["notes.txt\njust text".into()])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn image_attachment_is_not_treated_as_text_file() {
        let attachment = "![sample](data:image/png;base64,iVBORw0KGgo=)";
        assert!(is_image_attachment(attachment));
        assert!(!is_image_attachment("[notes.txt]\nhello"));
    }

    #[test]
    fn image_is_a_vision_part_not_markdown_text() {
        let messages = vec![ModelMessage::new("user", "What is in this picture?")];
        let images =
            image_data_urls(&["![sample](data:image/png;base64,iVBORw0KGgo=)".into()]).unwrap();
        let mut payload = messages_json(&messages);
        attach_images_to_user(&mut payload, &images, false).unwrap();
        assert_eq!(payload[0]["content"][0]["type"], "text");
        assert_eq!(payload[0]["content"][0]["text"], "What is in this picture?");
        assert_eq!(payload[0]["content"][1]["type"], "image_url");
        assert_eq!(
            payload[0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,iVBORw0KGgo="
        );
    }

    #[test]
    fn historical_images_stay_with_their_messages_in_order() {
        let first = format!(
            "![first]({})",
            data_url(
                "image/png",
                &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
            )
        );
        let second = format!("![second]({})", data_url("image/jpeg", &[0xff, 0xd8, 0xff]));
        let messages = vec![
            ModelMessage::new("user", "first caption").with_attachments(&[first]),
            ModelMessage::new("assistant", "noted"),
            ModelMessage::new("user", "second caption").with_attachments(&[second]),
        ];
        let payload = messages_json_checked(&messages, false).unwrap();
        assert_eq!(payload.len(), 3);
        assert_eq!(payload[0]["content"][0]["text"], "first caption");
        assert_eq!(payload[0]["content"][1]["type"], "image_url");
        assert_eq!(payload[1]["content"], "noted");
        assert_eq!(payload[2]["content"][0]["text"], "second caption");
        assert_eq!(payload[2]["content"][1]["type"], "image_url");
    }

    #[test]
    fn rejects_invalid_historical_image_instead_of_silently_dropping_it() {
        let messages = vec![
            ModelMessage::new("user", "caption")
                .with_attachments(&["![bad](data:image/png;base64,not_base64!!!)".into()]),
        ];
        assert!(messages_json_checked(&messages, false).is_err());
    }

    #[test]
    fn image_with_file_context_keeps_text_in_separate_user_messages() {
        let messages = vec![ModelMessage::new("user", "Read this attached config")];
        let mut payload = messages_json(&messages);
        payload.insert(
            0,
            serde_json::json!({"role":"user", "content":"[config.json] {\"enabled\":true}"}),
        );
        let images =
            image_data_urls(&["![sample](data:image/png;base64,iVBORw0KGgo=)".into()]).unwrap();
        attach_images_to_user(&mut payload, &images, false).unwrap();
        assert_eq!(payload[0]["content"], "[config.json] {\"enabled\":true}");
        assert_eq!(payload[1]["content"][1]["type"], "image_url");
    }
}

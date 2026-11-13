use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
    pub attachments: Vec<String>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_call_ids: Vec<String>,
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
            tool_call_id: None,
            tool_name: None,
            tool_call_ids: Vec::new(),
        }
    }

    pub fn tool_result(id: &str, name: &str, content: &str) -> Self {
        let mut message = Self::new("user", content);
        message.role = "tool".to_string();
        message.tool_call_id = Some(id.to_string());
        message.tool_name = Some(name.to_string());
        message.tool_call_ids.push(id.to_string());
        message
    }

    pub fn with_attachments(mut self, attachments: &[String]) -> Self {
        self.attachments = attachments.to_vec();
        self
    }
}

const MAX_ERROR_BODY_CHARS: usize = 512;
const MAX_RESPONSE_BYTES: usize = 1_000_000;

pub fn is_image_attachment(attachment: &str) -> bool {
    attachment.starts_with("data:image/")
        || (attachment.starts_with("![") && attachment.contains("](data:image/"))
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
    let without_headers = body.split_once("\r\n\r\n").map_or(body, |(_, value)| value);
    let without_bearer = regex_lite_bearer(without_headers);
    without_bearer.chars().take(MAX_ERROR_BODY_CHARS).collect()
}

fn regex_lite_bearer(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find("Bearer ") {
        output.push_str(&rest[..index]);
        let token_start = index + "Bearer ".len();
        let token_end = rest[token_start..]
            .find(|character: char| {
                character.is_whitespace() || character == ',' || character == '"'
            })
            .map_or(rest.len(), |offset| token_start + offset);
        if token_end == token_start {
            output.push_str("Bearer ");
        } else {
            output.push_str("Bearer [redacted]");
        }
        rest = &rest[token_end..];
    }
    output.push_str(rest);
    output
}

fn add_response_bytes(total: &mut usize, chunk_len: usize) -> Result<(), String> {
    *total = total
        .checked_add(chunk_len)
        .filter(|size| *size <= MAX_RESPONSE_BYTES)
        .ok_or_else(|| "Provider response exceeded the maximum allowed size".to_string())?;
    Ok(())
}

fn validate_tool_definitions(tools: &[Value]) -> Result<(), String> {
    let mut names = std::collections::HashSet::with_capacity(tools.len());
    for (index, tool) in tools.iter().enumerate() {
        let function = tool.get("function").ok_or_else(|| {
            format!(
                "Tool definition {} is missing its function object",
                index + 1
            )
        })?;
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "Tool definition {} must use function type, a nonempty name, and object parameters",
                    index + 1
                )
            })?;
        if tool.get("type").and_then(Value::as_str) != Some("function")
            || !function.get("parameters").is_some_and(Value::is_object)
        {
            return Err(format!(
                "Tool definition {} must use function type, a nonempty name, and object parameters",
                index + 1
            ));
        }
        if !names.insert(name) {
            return Err(format!("Duplicate tool definition name: {name}"));
        }
    }
    Ok(())
}

fn parse_ollama_stream_event(line: &str) -> Result<Value, String> {
    serde_json::from_str(line).map_err(|error| format!("Malformed Ollama stream event: {error}"))
}

fn parse_ollama_completion(json: &Value) -> Result<String, String> {
    if let Some(error) = json.get("error").and_then(Value::as_str) {
        return Err(format_provider_error(
            "Ollama",
            reqwest::StatusCode::BAD_REQUEST,
            error,
        ));
    }
    json.pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "Ollama returned an unexpected completion shape (missing message.content)".into()
        })
}

fn format_transport_error(provider: &str, timed_out: bool) -> String {
    let source = if timed_out {
        "request timed out"
    } else {
        "network request failed"
    };
    format!("{provider}: {source}; check connectivity and endpoint availability")
}

fn format_provider_error(provider: &str, status: reqwest::StatusCode, body: &str) -> String {
    let safe_body = safe_error_body(body);
    let normalized = safe_body.to_ascii_lowercase();
    let category = if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        "authentication failed"
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        "rate limit exceeded"
    } else if normalized.contains("context length")
        || normalized.contains("context_length")
        || normalized.contains("prompt is too long")
        || normalized.contains("maximum context")
    {
        "context limit exceeded"
    } else {
        "request failed"
    };
    format!("{provider} {category} (HTTP {status}): {safe_body}")
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
mod utf8_stream_tests {
    use super::{process_sse_line, push_utf8_chunk};

    #[test]
    fn provider_errors_redact_bearer_secrets_and_bound_body_text() {
        let result = super::safe_error_body(r#"{"message":"bad", "auth":"Bearer topsecret"}"#);
        assert!(result.contains("Bearer [redacted]"));
        assert!(!result.contains("topsecret"));
        assert!(super::safe_error_body(&"x".repeat(600)).len() <= super::MAX_ERROR_BODY_CHARS);
    }

    #[test]
    fn parses_completion_text_and_multiple_native_tool_calls() {
        let result = super::parse_openai_completion(&serde_json::json!({
            "choices": [{"finish_reason":"tool_calls", "message": {
                "content": "working",
                "tool_calls": [
                    {"id":"call_a", "function":{"name":"read_file", "arguments":{"path":"a"}}},
                    {"id":"call_b", "function":{"name":"list_dir", "arguments":{"path":"src"}}}
                ]
            }}]
        }))
        .unwrap();
        let calls = crate::harness::ToolDispatcher::parse_tool_calls(&result);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "call_a");
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "a");
        assert_eq!(calls[1].id, "call_b");
        assert_eq!(calls[1].arguments["path"], "src");
        assert!(result.starts_with("working"));
    }

    #[test]
    fn rejects_malformed_or_truncated_completion_shapes_without_erasing_arguments() {
        assert!(super::parse_openai_completion(&serde_json::json!({"choices":[]})).is_err());
        assert!(
            super::parse_openai_completion(&serde_json::json!({
                "choices":[{"finish_reason":"length", "message":{"content":"partial"}}]
            }))
            .unwrap_err()
            .contains("truncated")
        );
        assert!(super::parse_openai_completion(&serde_json::json!({
            "choices":[{"message":{"tool_calls":[{"function":{"name":"read_file","arguments":[]}}]}}]
        })).is_err());
    }

    #[test]
    fn malformed_ollama_stream_json_is_an_error() {
        assert!(super::parse_ollama_stream_event(r#"{"message":{"content":"ok"}}"#).is_ok());
        assert!(
            super::parse_ollama_stream_event("{broken")
                .unwrap_err()
                .contains("Malformed Ollama stream event")
        );
    }

    #[test]
    fn ollama_completion_rejects_error_and_missing_content() {
        assert_eq!(
            super::parse_ollama_completion(&serde_json::json!({"message":{"content":"ok"}}))
                .unwrap(),
            "ok"
        );
        assert!(
            super::parse_ollama_completion(&serde_json::json!({"error":"model missing"})).is_err()
        );
        assert!(super::parse_ollama_completion(&serde_json::json!({})).is_err());
    }

    #[test]
    fn ollama_final_line_without_newline_reports_usage_errors_and_malformed_json() {
        let mut text = String::new();
        let mut tools = std::collections::BTreeMap::new();
        let mut usages = Vec::new();
        let mut process = |line: &str| {
            let json = super::parse_ollama_stream_event(line).unwrap();
            if let Some(usage) = super::provider_usage_envelope("Ollama", &json) {
                usages.push(usage);
            }
            text.push_str(json["message"]["content"].as_str().unwrap_or_default());
            if let Some(calls) = json["message"]["tool_calls"].as_array() {
                for call in calls {
                    tools.insert(
                        call["id"].as_str().unwrap_or_default().to_string(),
                        call.clone(),
                    );
                }
            }
        };
        process(r#"{"message":{"content":"done"},"prompt_eval_count":2,"eval_count":3}"#);
        assert!(super::parse_ollama_stream_event("{truncated").is_err());
        assert_eq!(text, "done");
        assert_eq!(usages.len(), 1);
        assert!(tools.is_empty());
    }

    #[test]
    fn builds_provider_client_with_bounded_connect_and_request_timeouts() {
        assert!(super::provider_http_client(std::time::Duration::from_secs(45)).is_ok());
    }

    #[test]
    fn formats_transport_failures_without_exposing_sensitive_details() {
        assert_eq!(
            super::format_transport_error("OpenRouter", true),
            "OpenRouter: request timed out; check connectivity and endpoint availability"
        );
        assert_eq!(
            super::format_transport_error("Ollama", false),
            "Ollama: network request failed; check connectivity and endpoint availability"
        );
    }

    #[test]
    fn reports_authentication_rate_limit_and_context_limit_statuses() {
        assert!(
            super::format_provider_error(
                "OpenAI Compatible",
                reqwest::StatusCode::UNAUTHORIZED,
                r#"{"error":"missing credential"}"#
            )
            .contains("authentication")
        );
        assert!(
            super::format_provider_error(
                "OpenRouter",
                reqwest::StatusCode::TOO_MANY_REQUESTS,
                r#"{"error":"busy"}"#
            )
            .contains("rate limit")
        );
        assert!(
            super::format_provider_error(
                "Google AI Studio",
                reqwest::StatusCode::BAD_REQUEST,
                r#"{"error":{"message":"prompt is too long: context length exceeded"}}"#
            )
            .contains("context limit")
        );
    }

    #[test]
    fn final_sse_line_processes_content_tools_usage_finish_and_errors() {
        let mut text = String::new();
        let mut tools = std::collections::BTreeMap::new();
        let mut tokens = Vec::new();
        let mut usages = Vec::new();
        let mut activities = Vec::new();
        let mut error = None;
        let mut token = |value| tokens.push(value);
        let mut usage = |value| usages.push(value);
        let mut activity = |value| activities.push(value);
        let mut process = |line: &str| {
            process_sse_line(
                line,
                "OpenRouter",
                &mut text,
                &mut tools,
                &mut token,
                &mut usage,
                &mut activity,
                &mut error,
            )
        };
        process(r#"data: {"choices":[{"delta":{"content":"final"},"finish_reason":"stop"}]}"#);
        process(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{}"}}]}}]}"#,
        );
        process(r#"data: {"usage":{"prompt_tokens":3,"completion_tokens":2}}"#);
        process("data: {malformed}");
        process("data: [DONE]");
        process(" ");
        drop(process);
        assert_eq!(text, "final");
        assert_eq!(tokens, ["final"]);
        assert_eq!(tools[&0].0, "call_1");
        assert_eq!(tools[&0].1, "read");
        assert_eq!(usages.len(), 1);
        assert!(error.is_some());
    }

    #[test]
    fn rejects_invalid_tool_definitions_before_provider_request() {
        let valid = serde_json::json!({"type":"function","function":{"name":"read_file","parameters":{"type":"object"}}});
        assert!(super::validate_tool_definitions(std::slice::from_ref(&valid)).is_ok());
        assert!(
            super::validate_tool_definitions(&[serde_json::json!({"function": valid["function"]})])
                .is_err()
        );
        assert!(
            super::validate_tool_definitions(&[
                serde_json::json!({"type":"function","function":{"name":" ","parameters":{}}})
            ])
            .is_err()
        );
        assert!(
            super::validate_tool_definitions(&[valid.clone(), valid])
                .unwrap_err()
                .contains("Duplicate tool definition")
        );
    }

    #[test]
    fn rejects_unexpected_openai_stream_events() {
        let mut text = String::new();
        let mut tools = std::collections::BTreeMap::new();
        let mut error = None;
        process_sse_line(
            r#"data: {"unexpected":"shape"}"#,
            "OpenRouter",
            &mut text,
            &mut tools,
            &mut |_| {},
            &mut |_| {},
            &mut |_| {},
            &mut error,
        );
        assert!(text.is_empty());
        assert!(
            error
                .unwrap()
                .contains("Unexpected OpenRouter stream event shape")
        );
    }

    #[test]
    fn malformed_stream_tool_arguments_are_preserved_raw() {
        let mut text = String::new();
        let mut tools = std::collections::BTreeMap::new();
        let mut error = None;
        process_sse_line(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_bad","function":{"name":"future_tool","arguments":"{"}}]}}]}"#,
            "OpenRouter",
            &mut text,
            &mut tools,
            &mut |_| {},
            &mut |_| {},
            &mut |_| {},
            &mut error,
        );
        assert_eq!(tools[&0].0, "call_bad");
        assert_eq!(tools[&0].1, "future_tool");
        assert_eq!(tools[&0].2, "{");
        assert!(error.is_none());
    }

    #[test]
    fn fragmented_utf8_is_preserved_and_invalid_sequences_replaced_at_eof() {
        let expected = "ASCII é Filipino ñ 中文 日本語 한국어 😀 mixed";
        let mut pending = Vec::new();
        let mut output = String::new();
        for byte in expected.as_bytes() {
            push_utf8_chunk(&mut pending, &mut output, &[*byte], false).unwrap();
        }
        push_utf8_chunk(&mut pending, &mut output, &[], true).unwrap();
        assert_eq!(output, expected);

        let mut pending = Vec::new();
        let mut output = String::new();
        push_utf8_chunk(&mut pending, &mut output, &[0xf0, 0x9f], false).unwrap();
        push_utf8_chunk(&mut pending, &mut output, &[0x41], false).unwrap();
        push_utf8_chunk(&mut pending, &mut output, &[], true).unwrap();
        assert_eq!(output, "�A");
    }
}

#[cfg(test)]
fn messages_json(messages: &[ModelMessage]) -> Vec<Value> {
    messages_json_checked(messages, false).expect("valid test fixture")
}

pub(crate) fn push_utf8_chunk(
    pending: &mut Vec<u8>,
    buffer: &mut String,
    chunk: &[u8],
    flush: bool,
) -> Result<(), String> {
    use std::str::Utf8Error;

    pending.extend_from_slice(chunk);
    loop {
        match std::str::from_utf8(pending) {
            Ok(text) => {
                buffer.push_str(text);
                pending.clear();
                return Ok(());
            }
            Err(error) => {
                let valid_len = error.valid_up_to();
                let valid = std::str::from_utf8(&pending[..valid_len])
                    .map_err(|error: Utf8Error| error.to_string())?;
                buffer.push_str(valid);
                pending.drain(..valid_len);
                if let Some(invalid_len) = error.error_len() {
                    buffer.push('\u{fffd}');
                    pending.drain(..invalid_len);
                } else if flush {
                    buffer.push('\u{fffd}');
                    pending.clear();
                    return Ok(());
                } else {
                    return Ok(());
                }
            }
        }
    }
}

fn messages_json_checked(messages: &[ModelMessage], ollama: bool) -> Result<Vec<Value>, String> {
    let mut normalized: Vec<Value> = Vec::new();
    let mut unresolved_tool_calls = std::collections::HashSet::new();
    for message in messages {
        if message.role == "assistant"
            && let Ok(mut val) = serde_json::from_str::<Value>(&message.content)
        {
            if let Some(calls) = val.get_mut("tool_calls").and_then(Value::as_array_mut) {
                for (call_index, call) in calls.iter_mut().enumerate() {
                    if call
                        .get("id")
                        .and_then(Value::as_str)
                        .is_none_or(|id| id.trim().is_empty())
                    {
                        call["id"] = serde_json::json!(format!("call_{}", call_index + 1));
                    }
                    if call["function"]["name"]
                        .as_str()
                        .is_none_or(|name| name.trim().is_empty())
                    {
                        call["function"]["name"] = serde_json::json!("unknown_tool");
                    }
                    if !call["function"]["arguments"].is_string()
                        && !call["function"]["arguments"].is_object()
                    {
                        call["function"]["arguments"] = serde_json::json!("{}");
                    }
                }
                for call in calls {
                    if let Some(id) = call.get("id").and_then(Value::as_str)
                        && !unresolved_tool_calls.insert(id.to_string())
                    {
                        return Err(format!("Duplicate tool call ID: {id}"));
                    }
                }
            }
            normalized.push(val);
            continue;
        }

        if message.role == "tool" {
            let id = message
                .tool_call_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| "Tool result is missing its call ID".to_string())?;
            let name = message
                .tool_name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .ok_or_else(|| "Tool result is missing its tool name".to_string())?;
            if !unresolved_tool_calls.remove(id) {
                return Err(format!(
                    "Tool result references unknown or already answered tool call: {id}"
                ));
            }
            normalized.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": id,
                "name": name,
                "content": message.content,
            }));
            continue;
        }

        if !unresolved_tool_calls.is_empty() {
            return Err(format!(
                "Provider request contains unresolved tool calls before message with role '{}': {}",
                message.role,
                unresolved_tool_calls
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        let role = if message.role == "system" {
            if normalized.iter().all(|item| item["role"] == "system") {
                "system"
            } else {
                "user"
            }
        } else if message.role == "assistant" {
            "assistant"
        } else if message.role == "tool" {
            "tool"
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
    if !unresolved_tool_calls.is_empty() {
        return Err(format!(
            "Provider request contains unresolved tool calls: {}",
            unresolved_tool_calls
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        ));
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
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid endpoint URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Endpoint URL must use HTTP or HTTPS".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Endpoint URL must not contain embedded credentials".into());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Endpoint URL must include a host".to_string())?;
    let host_address = host.trim_start_matches('[').trim_end_matches(']');
    let loopback = match host_address.parse::<std::net::IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => host == "localhost" || host.ends_with(".localhost"),
    };
    let private_or_local = match host_address.parse::<std::net::IpAddr>() {
        Ok(ip) => private_ip(ip),
        Err(_) => {
            loopback
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
        "http" if loopback && (provider == "Ollama" || provider == "OpenAI Compatible") => {
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

fn parse_openai_completion(json: &Value) -> Result<String, String> {
    let choice = json["choices"]
        .as_array()
        .and_then(|choices| choices.first())
        .ok_or_else(|| "Provider returned no completion choices".to_string())?;
    if choice["finish_reason"] == "length" {
        return Err("Model response was truncated by its output token limit".into());
    }
    if choice["finish_reason"] == "content_filter" {
        return Err("Model response was blocked by the provider's content filter".into());
    }
    let message = &choice["message"];
    let mut output = message["content"].as_str().unwrap_or_default().to_string();
    if let Some(calls) = message["tool_calls"].as_array() {
        for call in calls {
            let name = call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if name.trim().is_empty() {
                return Err("Provider returned a tool call without a function name".into());
            }
            let id = call["id"]
                .as_str()
                .filter(|id| !id.trim().is_empty())
                .unwrap_or("call_missing_id");
            let arguments = match call.pointer("/function/arguments") {
                Some(Value::String(raw)) => raw.clone(),
                Some(value) if value.is_object() => value.to_string(),
                _ => {
                    return Err(format!(
                        "Provider returned malformed arguments for tool '{name}'"
                    ));
                }
            };
            output.push_str(&format!(
                "<tool_call>\\n{}\\n</tool_call>\\n",
                serde_json::json!({"id": id, "name": name, "arguments": arguments})
            ));
        }
    }
    if output.is_empty() {
        return Err("Provider returned an empty completion without text or tool calls".into());
    }
    Ok(output)
}

fn provider_http_client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
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
    let client = provider_http_client(Duration::from_secs(45))?;

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
                .map_err(|e| format_transport_error("Ollama", e.is_timeout()))?;

            if !res.status().is_success() {
                let status = res.status();
                let error = read_capped_error(res).await;
                return Err(format_provider_error("Ollama", status, &error));
            }

            let body = read_capped_body(res).await?;
            let json: Value = serde_json::from_slice(&body)
                .map_err(|e| format!("Failed to parse Ollama JSON: {}", e))?;
            parse_ollama_completion(&json)
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
                .map_err(|e| format_transport_error(provider, e.is_timeout()))?;

            if !res.status().is_success() {
                let status = res.status();
                let error = read_capped_error(res).await;
                return Err(format_provider_error(provider, status, &error));
            }

            let body = read_capped_body(res).await?;
            let json: Value = serde_json::from_slice(&body)
                .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
            parse_openai_completion(&json)
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
fn cached_input_tokens(usage: &Value) -> Option<u64> {
    openai_usage(usage)
        .or_else(|| google_usage(usage))?
        .cached_input_tokens
}

#[allow(clippy::too_many_arguments)]
fn process_sse_line<OnToken, OnUsage, OnActivity>(
    line: &str,
    provider: &str,
    accumulated: &mut String,
    tool_accumulator: &mut std::collections::BTreeMap<usize, (String, String, String)>,
    on_token: &mut OnToken,
    on_usage: &mut OnUsage,
    on_activity: &mut OnActivity,
    stream_error: &mut Option<String>,
) where
    OnToken: FnMut(String),
    OnUsage: FnMut(ProviderUsage),
    OnActivity: FnMut(String),
{
    let Some(data) = line.strip_prefix("data:") else {
        return;
    };
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return;
    }
    process_sse_data(
        data,
        provider,
        accumulated,
        tool_accumulator,
        on_token,
        on_usage,
        on_activity,
        stream_error,
    );
}

#[allow(clippy::too_many_arguments)]
fn process_sse_data<OnToken, OnUsage, OnActivity>(
    data: &str,
    provider: &str,
    accumulated: &mut String,
    tool_accumulator: &mut std::collections::BTreeMap<usize, (String, String, String)>,
    on_token: &mut OnToken,
    on_usage: &mut OnUsage,
    on_activity: &mut OnActivity,
    stream_error: &mut Option<String>,
) where
    OnToken: FnMut(String),
    OnUsage: FnMut(ProviderUsage),
    OnActivity: FnMut(String),
{
    if data.trim().is_empty() || data.trim() == "[DONE]" {
        return;
    }
    let json = match serde_json::from_str::<Value>(data) {
        Ok(json) => json,
        Err(error) => {
            *stream_error = Some(format!("Malformed {} stream event: {}", provider, error));
            return;
        }
    };
    if let Some(raw_usage) = json.get("usage").or_else(|| json.get("usageMetadata"))
        && let Some(usage) = provider_usage_envelope(provider, raw_usage)
    {
        on_usage(usage);
    } else if matches!(provider, "Google AI Studio" | "Ollama")
        && let Some(usage) = provider_usage_envelope(provider, &json)
    {
        on_usage(usage);
    }
    if let Some(error) = json.get("error") {
        *stream_error = Some(format!(
            "{} returned error: {}",
            provider,
            safe_error_body(&error.to_string())
        ));
    }
    let has_choices = json.get("choices").is_some_and(Value::is_array);
    let has_usage = json.get("usage").is_some() || json.get("usageMetadata").is_some();
    if !has_choices && !has_usage && json.get("error").is_none() {
        *stream_error = Some(format!(
            "Unexpected {} stream event shape: expected choices, usage metadata, or an error",
            provider
        ));
        return;
    }
    if let Some(reasoning) = json["choices"][0]["delta"]["reasoning"]
        .as_str()
        .filter(|s| !s.is_empty())
    {
        on_activity(format!("[agent-step]{}", reasoning));
    }
    if let Some(content) = json["choices"][0]["delta"]["content"]
        .as_str()
        .or_else(|| json["choices"][0]["message"]["content"].as_str())
        .filter(|s| !s.is_empty())
    {
        accumulated.push_str(content);
        on_token(content.to_string());
    }
    if let Some(calls) = json["choices"][0]["delta"]["tool_calls"]
        .as_array()
        .or_else(|| json["choices"][0]["message"]["tool_calls"].as_array())
        .filter(|calls| !calls.is_empty())
    {
        for (fallback_idx, tc) in calls.iter().enumerate() {
            let index = tc
                .get("index")
                .and_then(Value::as_u64)
                .map(|v| v as usize)
                .unwrap_or(fallback_idx);
            let entry = tool_accumulator.entry(index).or_default();
            if let Some(id) = tc
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                entry.0 = id.to_string();
            }
            if let Some(name) = tc
                .pointer("/function/name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                entry.1 = name.to_string();
            }
            if let Some(args) = tc.pointer("/function/arguments") {
                if let Some(args) = args.as_str() {
                    entry.2.push_str(args);
                } else if args.is_object() {
                    entry.2 = args.to_string();
                }
            }
        }
    }
    if let Some(reason) = json["choices"][0]["finish_reason"].as_str() {
        match reason {
            "length" => *stream_error = Some(
                "Model response was truncated (finish_reason=length). Retry with a shorter prompt."
                    .to_string(),
            ),
            "content_filter" => {
                *stream_error =
                    Some("Model response was blocked by the provider's content filter.".to_string())
            }
            _ => {}
        }
    }
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
        mut on_token,
        mut on_usage,
        mut on_activity,
    } = callbacks;
    validate_tool_definitions(tools)?;
    let client = provider_http_client(Duration::from_secs(120))?;

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
                .map_err(|e| format_transport_error("Ollama", e.is_timeout()))?;
            if !res.status().is_success() {
                let status = res.status();
                let error = read_capped_error(res).await;
                return Err(format_provider_error("Ollama", status, &error));
            }
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            let mut buffer = String::new();
            let mut utf8_pending = Vec::new();
            let mut native_calls = Vec::new();
            let mut received_bytes = 0;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| format_transport_error("Ollama", e.is_timeout()))?;
                add_response_bytes(&mut received_bytes, chunk.len())?;
                push_utf8_chunk(&mut utf8_pending, &mut buffer, &chunk, false)?;
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer.drain(..=pos);
                    if line.is_empty() {
                        continue;
                    }
                    let json = parse_ollama_stream_event(&line)?;
                    if let Some(usage) = provider_usage_envelope("Ollama", &json) {
                        on_usage(usage);
                    }
                    if let Some(error) = json.get("error") {
                        return Err(format_provider_error(
                            "Ollama",
                            reqwest::StatusCode::BAD_REQUEST,
                            &error.to_string(),
                        ));
                    }
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
                }
            }
            push_utf8_chunk(&mut utf8_pending, &mut buffer, &[], true)?;
            if !buffer.trim().is_empty() {
                let json = parse_ollama_stream_event(buffer.trim())?;
                if let Some(usage) = provider_usage_envelope("Ollama", &json) {
                    on_usage(usage);
                }
                if let Some(error) = json.get("error") {
                    return Err(format_provider_error(
                        "Ollama",
                        reqwest::StatusCode::BAD_REQUEST,
                        &error.to_string(),
                    ));
                }
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
                .map_err(|e| format_transport_error(provider, e.is_timeout()))?;
            if !res.status().is_success() {
                let status = res.status();
                let error = read_capped_error(res).await;
                return Err(format_provider_error(provider, status, &error));
            }
            let mut stream = res.bytes_stream();
            let mut accumulated = String::new();
            let mut buffer = String::new();
            let mut utf8_pending = Vec::new();
            let mut stream_error = None;
            let mut tool_accumulator: std::collections::BTreeMap<usize, (String, String, String)> =
                std::collections::BTreeMap::new();
            let mut received_bytes = 0;
            loop {
                let at_eof = match stream.next().await {
                    Some(chunk) => {
                        let chunk =
                            chunk.map_err(|e| format_transport_error(provider, e.is_timeout()))?;
                        add_response_bytes(&mut received_bytes, chunk.len())?;
                        push_utf8_chunk(&mut utf8_pending, &mut buffer, &chunk, false)?;
                        false
                    }
                    None => {
                        push_utf8_chunk(&mut utf8_pending, &mut buffer, &[], true)?;
                        true
                    }
                };
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer.drain(..=pos);
                    if !line.is_empty() {
                        process_sse_line(
                            &line,
                            provider,
                            &mut accumulated,
                            &mut tool_accumulator,
                            &mut on_token,
                            &mut on_usage,
                            &mut on_activity,
                            &mut stream_error,
                        );
                    }
                }
                if at_eof {
                    if !buffer.trim().is_empty() {
                        process_sse_line(
                            buffer.trim(),
                            provider,
                            &mut accumulated,
                            &mut tool_accumulator,
                            &mut on_token,
                            &mut on_usage,
                            &mut on_activity,
                            &mut stream_error,
                        );
                    }
                    break;
                }
            }
            if !tool_accumulator.is_empty() {
                for (idx, (id, name, arguments)) in tool_accumulator {
                    let call_id = if id.is_empty() {
                        format!("call_{}", idx + 1)
                    } else {
                        id
                    };
                    let raw_arguments = if arguments.trim().is_empty() {
                        "{}".to_string()
                    } else {
                        arguments
                    };
                    accumulated.push_str(&format!(
                        "<tool_call>\n{}\n</tool_call>\n",
                        serde_json::json!({
                            "id": call_id,
                            "name": name,
                            "arguments": raw_arguments
                        })
                    ));
                }
            }
            if let Some(error) = stream_error {
                return Err(error);
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
    fn serializes_tool_results_with_original_call_ids_and_order() {
        let messages = vec![
            ModelMessage::new(
                "assistant",
                r#"{"role":"assistant","content":null,"tool_calls":[{"id":"call_a","type":"function","function":{"name":"read_file","arguments":"{}"}},{"id":"call_b","type":"function","function":{"name":"git_status","arguments":"{}"}}]}"#,
            ),
            ModelMessage::tool_result("call_a", "read_file", "{\"success\":true}"),
            ModelMessage::tool_result(
                "call_b",
                "git_status",
                "{\"success\":false,\"error\":\"failed\"}",
            ),
            ModelMessage::new("assistant", "Recovered."),
        ];
        let payload = messages_json_checked(&messages, false).unwrap();
        assert_eq!(payload[0]["tool_calls"][0]["id"], "call_a");
        assert_eq!(payload[0]["tool_calls"][1]["id"], "call_b");
        assert_eq!(payload[1]["role"], "tool");
        assert_eq!(payload[1]["tool_call_id"], "call_a");
        assert_eq!(payload[2]["role"], "tool");
        assert_eq!(payload[2]["tool_call_id"], "call_b");
        assert_eq!(payload[1]["content"], "{\"success\":true}");
        assert_eq!(
            payload[2]["content"],
            "{\"success\":false,\"error\":\"failed\"}"
        );
        assert_eq!(payload[3]["role"], "assistant");
        assert_eq!(payload[3]["content"], "Recovered.");
    }

    #[test]
    fn merges_leading_system_messages_into_one_system_block() {
        let messages = vec![
            ModelMessage::new("system", "Base instructions"),
            ModelMessage::new("system", "Project instructions"),
            ModelMessage::new("user", "Question"),
        ];

        let payload = messages_json_checked(&messages, false).unwrap();

        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0]["role"], "system");
        assert_eq!(
            payload[0]["content"],
            "Base instructions\n\nProject instructions"
        );
        assert_eq!(payload[1]["role"], "user");
        assert_eq!(payload[1]["content"], "Question");
    }

    #[test]
    fn demotes_system_messages_that_appear_after_conversation_start() {
        let messages = vec![
            ModelMessage::new("system", "Base instructions"),
            ModelMessage::new("user", "Question"),
            ModelMessage::new("system", "Mid-conversation note"),
        ];

        let payload = messages_json_checked(&messages, false).unwrap();

        assert_eq!(payload[0]["role"], "system");
        assert_eq!(payload.len(), 2);
        assert_eq!(payload[1]["role"], "user");
        assert_eq!(payload[1]["content"], "Question\n\nMid-conversation note");
    }

    #[test]
    fn rejects_unresolved_tool_calls_instead_of_sending_incomplete_history() {
        let assistant = ModelMessage::new(
            "assistant",
            r#"{"role":"assistant","content":null,"tool_calls":[{"id":"call_orphan","type":"function","function":{"name":"read_file","arguments":"{}"}}]}"#,
        );
        assert!(
            messages_json_checked(&[assistant], false)
                .unwrap_err()
                .contains("unresolved tool calls")
        );
    }

    #[test]
    fn accepts_complete_tool_call_group_and_rejects_duplicate_result() {
        let assistant = ModelMessage::new(
            "assistant",
            r#"{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{}"}}]}"#,
        );
        let result = ModelMessage::tool_result("call_1", "read_file", "ok");
        assert!(messages_json_checked(&[assistant.clone(), result.clone()], false).is_ok());
        assert!(
            messages_json_checked(&[assistant, result.clone(), result], false)
                .unwrap_err()
                .contains("unknown or already answered")
        );
    }

    #[test]
    fn rejects_tool_result_without_call_identity() {
        let mut result = ModelMessage::new("user", "{}");
        result.role = "tool".into();
        result.tool_call_id = Some("call_1".into());
        assert!(messages_json_checked(&[result], false).is_err());
    }

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
    fn provider_endpoint_requires_https_for_remote_hosts_and_rejects_credentials() {
        for endpoint in [
            "http://api.example.com/v1",
            "http://192.0.2.10/v1",
            "http://10.0.0.5/v1",
            "ftp://localhost/v1",
            "https://user:pass@example.com/v1",
            "https://example.com:99999/v1",
        ] {
            assert!(
                super::validated_endpoint_for_test("OpenAI Compatible", Some(endpoint)).is_err(),
                "accepted unsafe endpoint {endpoint}"
            );
        }
        for endpoint in [
            "http://localhost/v1",
            "http://127.0.0.1/v1",
            "http://[::1]/v1",
            "https://api.example.com/v1/",
            "https://api.example.com/custom/api",
        ] {
            assert!(
                super::validated_endpoint_for_test("OpenAI Compatible", Some(endpoint)).is_ok(),
                "rejected valid endpoint {endpoint}"
            );
        }
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
        assert!(is_image_attachment("data:image/png;base64,iVBORw0KGgo="));
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

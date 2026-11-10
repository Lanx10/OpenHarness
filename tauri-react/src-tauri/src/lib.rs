use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{Emitter, Manager, State};
use tokio::sync::oneshot;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
const CREATE_NO_WINDOW: u32 = 0x08000000;

mod client;
mod harness;
mod secrets;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub provider: String,
    pub model_id: String,
    pub base_url: Option<String>,
    pub custom_endpoint: bool,
    pub has_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub path: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub text: String,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatThread {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub messages: Vec<ChatMessage>,
}

pub struct AppState {
    pub providers: Mutex<HashMap<String, ProviderInfo>>,
    pub projects: Mutex<Vec<ProjectInfo>>,
    pub threads: Mutex<Vec<ChatThread>>,
    app_data_dir: Mutex<Option<PathBuf>>,
    pub streams: Mutex<HashMap<String, oneshot::Sender<()>>>,
    next_stream_id: AtomicU64,
    pub pending_approvals: Arc<Mutex<PendingApprovals>>,
    terminals: Arc<Mutex<HashMap<String, Arc<Mutex<TerminalSession>>>>>,
}

struct TerminalSession {
    child: Child,
    stdin: ChildStdin,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            providers: Mutex::new(HashMap::new()),
            projects: Mutex::new(Vec::new()),
            threads: Mutex::new(Vec::new()),
            app_data_dir: Mutex::new(None),
            streams: Mutex::new(HashMap::new()),
            next_stream_id: AtomicU64::new(1),
            pending_approvals: Arc::new(Mutex::new(PendingApprovals::default())),
            terminals: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl AppState {
    pub fn project_root(&self, project_id: &str) -> Result<PathBuf, String> {
        let projects = self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let project = projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or_else(|| format!("Project not found: {}", project_id))?;
        let path = project
            .path
            .as_deref()
            .ok_or_else(|| "Project has no folder selected".to_string())?;
        let root = PathBuf::from(path)
            .canonicalize()
            .map_err(|e| format!("Project folder is unavailable: {}", e))?;
        if !root.is_dir() {
            return Err("Project folder is not a directory".into());
        }
        Ok(root)
    }
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedState {
    providers: HashMap<String, ProviderInfo>,
    projects: Vec<ProjectInfo>,
    #[serde(default)]
    threads: Vec<ChatThread>,
}

impl AppState {
    fn persist(&self) -> Result<(), String> {
        let path = self
            .app_data_dir
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?
            .clone();
        let Some(dir) = path else {
            return Ok(());
        };
        let state = PersistedState {
            providers: self
                .providers
                .lock()
                .map_err(|e| format!("Lock failure: {}", e))?
                .clone(),
            projects: self
                .projects
                .lock()
                .map_err(|e| format!("Lock failure: {}", e))?
                .clone(),
            threads: self
                .threads
                .lock()
                .map_err(|e| format!("Lock failure: {}", e))?
                .clone(),
        };
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Could not create app data directory: {}", e))?;
        let path = dir.join("state.json");
        let temp = dir.join("state.json.tmp");
        let bytes = serde_json::to_vec(&state)
            .map_err(|e| format!("Could not serialize app state: {}", e))?;
        std::fs::write(&temp, bytes).map_err(|e| format!("Could not write app state: {}", e))?;
        std::fs::rename(&temp, &path).map_err(|e| format!("Could not replace app state: {}", e))?;
        Ok(())
    }

    fn load(&self, dir: PathBuf) -> Result<(), String> {
        *self
            .app_data_dir
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))? = Some(dir.clone());
        let path = dir.join("state.json");
        let state = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<PersistedState>(&bytes)
                .map_err(|e| format!("Could not parse app state: {}", e))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => PersistedState::default(),
            Err(error) => return Err(format!("Could not read app state: {}", error)),
        };
        *self
            .providers
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))? = state.providers;
        *self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))? = state.projects;
        *self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))? = state.threads;
        Ok(())
    }
}

impl AppState {
    pub fn save_provider(
        &self,
        provider: String,
        model_id: String,
        base_url: Option<String>,
        custom_endpoint: bool,
        api_key: String,
    ) -> Result<ProviderInfo, String> {
        let provider = provider.trim();
        let model_id = model_id.trim();
        let base_url = if custom_endpoint {
            base_url
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        } else {
            None
        };
        let validated_base_url = match base_url.as_deref() {
            Some(value) => Some(client::validated_endpoint_for_test(provider, Some(value))?),
            None => None,
        };
        let has_key = !api_key.trim().is_empty();
        if provider.is_empty() || model_id.is_empty() || (provider != "Ollama" && provider != "OpenAI Compatible" && !has_key) {
            return Err("Provider, model ID, and API key are required".into());
        }

        if has_key {
            secrets::save_secret(&format!("provider_{}", provider), &api_key)?;
        } else {
            secrets::delete_secret(&format!("provider_{}", provider))?;
        }

        let info = ProviderInfo {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            base_url: validated_base_url,
            custom_endpoint,
            has_key,
        };

        let mut guard = self
            .providers
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        guard.insert(provider.to_string(), info.clone());
        drop(guard);
        self.persist()?;

        Ok(info)
    }

    pub fn list_providers(&self) -> Result<Vec<ProviderInfo>, String> {
        let guard = self
            .providers
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        Ok(guard.values().cloned().collect())
    }

    pub fn delete_provider(&self, provider: &str) -> Result<(), String> {
        secrets::delete_secret(&format!("provider_{}", provider))?;
        let mut guard = self
            .providers
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        guard.remove(provider);
        drop(guard);
        self.persist()?;
        Ok(())
    }

    pub fn get_projects(&self) -> Result<Vec<ProjectInfo>, String> {
        let guard = self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        Ok(guard.clone())
    }

    pub fn select_project(&self, id: &str) -> Result<Vec<ProjectInfo>, String> {
        let mut guard = self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        for p in guard.iter_mut() {
            p.active = p.id == id;
        }
        let projects = guard.clone();
        drop(guard);
        self.persist()?;
        Ok(projects)
    }

    pub fn create_project(
        &self,
        name: String,
        path: Option<PathBuf>,
    ) -> Result<ProjectInfo, String> {
        if path.as_ref().is_some_and(|candidate| !candidate.is_dir()) {
            return Err("Project folder does not exist or is not a directory".into());
        }
        let name = name.trim();
        if name.is_empty() {
            return Err("Project name cannot be empty".into());
        }
        let path_value = path.as_ref().map(|p| p.to_string_lossy().to_string());
        let id = name
            .to_lowercase()
            .replace(
                |ch: char| ch.is_whitespace() || ch == '/' || ch == '\\',
                "-",
            )
            .trim_matches('-')
            .to_string();
        let new_project = ProjectInfo {
            id: id.clone(),
            name: name.to_string(),
            subtitle: "No chats yet".into(),
            path: path_value.clone(),
            active: true,
        };
        let mut guard = self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let next_id = if id.is_empty() {
            format!("project-{}", guard.len() + 1)
        } else {
            id.clone()
        };
        let id = if guard.iter().any(|project| project.id == next_id) {
            format!("{}-{}", next_id, guard.len() + 1)
        } else {
            next_id
        };
        let mut new_project = new_project;
        new_project.id = id;
        // ponytail: supports only path-based dedupe; full rename/delete requires API additions. project name changes and canonical path split should be editable.
        if guard
            .iter()
            .any(|project| project.path.as_deref() == path_value.as_deref() && path_value.is_some())
        {
            return Err("A project for this folder is already loaded.".into());
        }
        for project in guard.iter_mut() {
            project.active = false;
        }
        guard.insert(0, new_project.clone());
        drop(guard);
        self.persist()?;
        Ok(new_project)
    }

    pub fn rename_project(&self, id: &str, name: &str) -> Result<Vec<ProjectInfo>, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Project name cannot be empty".into());
        }
        let mut guard = self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let project = guard
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Project not found: {}", id))?;
        project.name = name.to_string();
        let projects = guard.clone();
        drop(guard);
        self.persist()?;
        Ok(projects)
    }

    pub fn delete_project(&self, id: &str) -> Result<Vec<ProjectInfo>, String> {
        let mut guard = self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        guard.retain(|p| p.id != id);
        let mut threads = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        threads.retain(|thread| thread.project_id != id);
        drop(threads);
        let projects = guard.clone();
        drop(guard);
        self.persist()?;
        Ok(projects)
    }

    pub fn relink_project(
        &self,
        id: &str,
        path: Option<PathBuf>,
    ) -> Result<Vec<ProjectInfo>, String> {
        if path.as_ref().is_some_and(|candidate| !candidate.is_dir()) {
            return Err("Selected folder does not exist or is not a directory".into());
        }
        let mut guard = self
            .projects
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let project = guard
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Project not found: {}", id))?;
        project.path = path
            .as_ref()
            .map(|value| value.to_string_lossy().to_string());
        project.subtitle = project
            .path
            .clone()
            .unwrap_or_else(|| "No folder selected".into());
        let projects = guard.clone();
        drop(guard);
        self.persist()?;
        Ok(projects)
    }

    pub fn list_threads(&self, project_id: Option<&str>) -> Result<Vec<ChatThread>, String> {
        let guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        Ok(guard
            .iter()
            .filter(|thread| project_id.map(|id| thread.project_id == id).unwrap_or(true))
            .cloned()
            .collect())
    }

    pub fn create_thread(&self, project_id: &str) -> Result<ChatThread, String> {
        let thread = ChatThread {
            id: format!(
                "thread-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos()
            ),
            project_id: project_id.to_string(),
            title: "New chat".into(),
            messages: Vec::new(),
        };
        let mut guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        guard.insert(0, thread.clone());
        drop(guard);
        self.persist()?;
        Ok(thread)
    }

    pub fn rename_thread(&self, id: &str, title: &str) -> Result<Vec<ChatThread>, String> {
        if title.trim().is_empty() {
            return Err("Thread title cannot be empty".into());
        }
        let mut guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let thread = guard
            .iter_mut()
            .find(|thread| thread.id == id)
            .ok_or_else(|| format!("Thread not found: {}", id))?;
        thread.title = title.trim().to_string();
        let threads = guard.clone();
        drop(guard);
        self.persist()?;
        Ok(threads)
    }

    pub fn delete_thread(&self, id: &str) -> Result<Vec<ChatThread>, String> {
        let mut guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        guard.retain(|thread| thread.id != id);
        let threads = guard.clone();
        drop(guard);
        self.persist()?;
        Ok(threads)
    }

    pub fn get_thread(&self, id: &str) -> Result<ChatThread, String> {
        let guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        guard
            .iter()
            .find(|thread| thread.id == id)
            .cloned()
            .ok_or_else(|| format!("Thread not found: {}", id))
    }

    pub fn append_thread_message(
        &self,
        thread_id: &str,
        role: &str,
        text: &str,
    ) -> Result<ChatMessage, String> {
        self.append_thread_message_with_attachments(thread_id, role, text, &[])
    }

    pub fn append_thread_message_with_attachments(
        &self,
        thread_id: &str,
        role: &str,
        text: &str,
        attachments: &[String],
    ) -> Result<ChatMessage, String> {
        let message = ChatMessage {
            id: format!(
                "msg-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos()
            ),
            role: role.into(),
            text: text.into(),
            attachments: attachments.to_vec(),
        };
        let mut guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let thread = guard
            .iter_mut()
            .find(|thread| thread.id == thread_id)
            .ok_or_else(|| format!("Thread not found: {}", thread_id))?;
        thread.messages.push(message.clone());
        if thread.title == "New chat" && role == "user" {
            thread.title = text.chars().take(40).collect();
        }
        drop(guard);
        self.persist()?;
        Ok(message)
    }

    pub fn edit_thread_message(
        &self,
        thread_id: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<ChatMessage, String> {
        let mut guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let thread = guard
            .iter_mut()
            .find(|thread| thread.id == thread_id)
            .ok_or_else(|| format!("Thread not found: {}", thread_id))?;
        let message = thread
            .messages
            .iter_mut()
            .find(|msg| msg.id == message_id)
            .ok_or_else(|| format!("Message not found: {}", message_id))?;
        message.text = new_text.trim().to_string();
        let updated = message.clone();
        drop(guard);
        self.persist()?;
        Ok(updated)
    }

    pub fn delete_thread_message(
        &self,
        thread_id: &str,
        message_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let mut guard = self
            .threads
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        let thread = guard
            .iter_mut()
            .find(|thread| thread.id == thread_id)
            .ok_or_else(|| format!("Thread not found: {}", thread_id))?;
        thread.messages.retain(|msg| msg.id != message_id);
        let remaining = thread.messages.clone();
        drop(guard);
        self.persist()?;
        Ok(remaining)
    }

    pub async fn chat(
        &self,
        prompt: &str,
        model_id: &str,
        provider: &str,
        project_id: Option<&str>,
    ) -> Result<ChatResponse, String> {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return Err("Prompt cannot be empty".into());
        }

        // ponytail: canonical provider IDs are OpenRouter / Google AI Studio / Ollama; Claude and OpenAI are removed. URL normalization happens in client.rs.
        let canonical_provider = provider;
        let key_opt = secrets::get_secret(&format!("provider_{}", canonical_provider))?;
        let saved_provider = {
            let guard = self
                .providers
                .lock()
                .map_err(|e| format!("Lock failure: {}", e))?;
            guard.get(canonical_provider).cloned()
        };

        let reply = if key_opt.is_some() || saved_provider.is_some() {
            let base_url = saved_provider.as_ref().and_then(|p| p.base_url.as_deref());
            let messages = vec![client::ModelMessage::new("user", trimmed)];
            client::query_llm(
                canonical_provider,
                model_id,
                &messages,
                key_opt.as_deref(),
                base_url,
            )
            .await?
        } else {
            format!(
                "Local OpenHarness Assistant (Model: {}):\n\nI understand you want to work on: \"{}\". Configure API access for {} in Settings to route requests to live inference.",
                model_id, trimmed, provider
            )
        };

        // ponytail: only the first active project gets the subtitle update; upgrade path is project_id-aware subtitle routing.
        if let Ok(mut guard) = self.projects.lock() {
            let target_id = project_id.map(str::to_owned).or_else(|| {
                guard
                    .iter()
                    .find(|project| project.active)
                    .map(|project| project.id.clone())
            });
            if let Some(project) = guard
                .iter_mut()
                .find(|project| Some(project.id.as_str()) == target_id.as_deref())
            {
                let mut chars = trimmed.chars();
                let snippet: String = chars.by_ref().take(24).collect();
                project.subtitle = if chars.next().is_some() {
                    format!("💬 {}...", snippet)
                } else {
                    format!("💬 {}", snippet)
                };
            }
        }

        Ok(ChatResponse {
            text: reply,
            model: model_id.to_string(),
            provider: provider.to_string(),
        })
    }
}

#[tauri::command]
fn provider_save(
    provider: String,
    model_id: String,
    base_url: Option<String>,
    custom_endpoint: bool,
    api_key: String,
    state: State<'_, AppState>,
) -> Result<ProviderInfo, String> {
    state.save_provider(provider, model_id, base_url, custom_endpoint, api_key)
}

#[tauri::command]
fn provider_list(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, String> {
    state.list_providers()
}

#[tauri::command]
fn provider_delete(provider: String, state: State<'_, AppState>) -> Result<(), String> {
    state.delete_provider(&provider)
}

#[tauri::command]
fn get_projects(state: State<'_, AppState>) -> Result<Vec<ProjectInfo>, String> {
    state.get_projects()
}

#[tauri::command]
fn select_project(id: String, state: State<'_, AppState>) -> Result<Vec<ProjectInfo>, String> {
    state.select_project(&id)
}

#[tauri::command]
fn create_project(
    name: String,
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProjectInfo, String> {
    state.create_project(name, path.map(PathBuf::from))
}

#[tauri::command]
fn rename_project(
    id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<Vec<ProjectInfo>, String> {
    state.rename_project(&id, &name)
}

#[tauri::command]
fn delete_project(id: String, state: State<'_, AppState>) -> Result<Vec<ProjectInfo>, String> {
    state.delete_project(&id)
}

#[tauri::command]
fn relink_project(
    id: String,
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ProjectInfo>, String> {
    state.relink_project(&id, path.map(PathBuf::from))
}

#[tauri::command]
fn list_threads(
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ChatThread>, String> {
    state.list_threads(project_id.as_deref())
}

#[tauri::command]
fn create_thread(project_id: String, state: State<'_, AppState>) -> Result<ChatThread, String> {
    state.create_thread(&project_id)
}

#[tauri::command]
fn delete_thread(id: String, state: State<'_, AppState>) -> Result<Vec<ChatThread>, String> {
    state.delete_thread(&id)
}

#[tauri::command]
fn rename_thread(
    id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatThread>, String> {
    state.rename_thread(&id, &title)
}

#[tauri::command]
fn get_thread(id: String, state: State<'_, AppState>) -> Result<ChatThread, String> {
    state.get_thread(&id)
}

#[tauri::command]
fn edit_thread_message(
    thread_id: String,
    message_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<ChatMessage, String> {
    state.edit_thread_message(&thread_id, &message_id, &text)
}

#[tauri::command]
fn delete_thread_message(
    thread_id: String,
    message_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    state.delete_thread_message(&thread_id, &message_id)
}

#[tauri::command]
fn append_thread_message(
    thread_id: String,
    role: String,
    text: String,
    attachments: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<ChatMessage, String> {
    state.append_thread_message_with_attachments(
        &thread_id,
        &role,
        &text,
        attachments.as_deref().unwrap_or(&[]),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepEvent {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Serialize, Clone)]
struct StreamChunk {
    text: String,
    done: bool,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval: Option<ApprovalRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<StepEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug)]
struct ApprovalDecision {
    allowed: bool,
    response: Option<String>,
}

#[derive(Default)]
pub struct PendingApprovals {
    next_id: u64,
    waiting: HashMap<String, tokio::sync::oneshot::Sender<ApprovalDecision>>,
}

const MAX_AGENT_ITERATIONS: usize = 12;
const MAX_AGENT_RESPONSE_BYTES: usize = 1_000_000;
const MAX_COMMAND_BYTES: usize = 16_384;
const MAX_TOOL_OUTPUT_BYTES: usize = 200_000;

fn clamp_string(value: String, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn requires_approval(tool: &str) -> bool {
    matches!(tool, "write_file" | "run_command")
}

fn tool_kind(tool: &str) -> &'static str {
    match tool {
        "read_file" => "read",
        "grep_search" | "list_dir" => "grep",
        "write_file" => "edit",
        "run_command" => "command",
        "git_status" | "git_diff" | "git_log" | "git_branch" => "git",
        _ => "tool",
    }
}

fn tool_label(tool: &str, arguments: &serde_json::Value) -> String {
    let target = match tool {
        "read_file" | "write_file" => arguments["path"].as_str(),
        "list_dir" => arguments["path"].as_str().filter(|path| !path.is_empty()),
        "grep_search" => arguments["query"].as_str(),
        "run_command" => arguments["command"].as_str(),
        _ => None,
    }.map(|value| clamp_string(value.to_string(), 96));
    match target {
        Some(target) => format!("{} {}", tool.replace('_', " "), target),
        None => tool.replace('_', " "),
    }
}

fn validate_tool_call(call: &harness::ToolCall) -> Result<(), String> {
    match call.name.as_str() {
        "read_file" => {
            let path = call.arguments["path"]
                .as_str()
                .filter(|path| !path.trim().is_empty())
                .ok_or("read_file requires a path")?;
            if path.len() > 4096 {
                return Err("File path exceeds 4096 bytes".to_string());
            }
        }
        "write_file" => {
            let path = call.arguments["path"]
                .as_str()
                .ok_or("write_file requires a path")?;
            let content = call.arguments["content"]
                .as_str()
                .ok_or("write_file requires content")?;
            if path.len() > 4096 || content.len() > 2_000_000 {
                return Err("Write path or content exceeds the allowed size".to_string());
            }
        }
        "list_dir" => {
            if call.arguments["path"].as_str().unwrap_or("").len() > 4096 {
                return Err("Directory path exceeds 4096 bytes".to_string());
            }
        }
        "grep_search" | "grep" => {
            let query = call.arguments["query"]
                .as_str()
                .or_else(|| call.arguments["pattern"].as_str())
                .ok_or("grep_search requires a query")?;
            if query.len() > 1024 {
                return Err("Search query exceeds 1024 bytes".to_string());
            }
        }
        "run_command" => {
            let command = call.arguments["command"]
                .as_str()
                .ok_or("run_command requires a command")?;
            if command.len() > MAX_COMMAND_BYTES {
                return Err(format!("Command exceeds {} bytes", MAX_COMMAND_BYTES));
            }
        }
        "git_status" | "git_diff" | "git_log" | "git_branch" => {}
        other => return Err(format!("Unknown tool: {}", other)),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_agent_loop(
    provider: String,
    model_id: String,
    key: Option<String>,
    base_url: Option<String>,
    project_root: PathBuf,
    initial_messages: Vec<client::ModelMessage>,
    emitter: Channel<StreamChunk>,
    approvals: Arc<Mutex<PendingApprovals>>,
    attachments: Vec<String>,
) -> Result<String, String> {
    let system_prompt = harness::ToolDispatcher::build_system_prompt();
    let mut messages = Vec::with_capacity(initial_messages.len() + attachments.len() + 1);
    messages.push(client::ModelMessage::new("system", &system_prompt));
    for m in initial_messages {
        if m.role != "system" {
            messages.push(m);
        }
    }
    for attachment in attachments {
        messages.push(client::ModelMessage::new("user", &attachment));
    }

    let final_text = String::new();
    for _iteration in 0..MAX_AGENT_ITERATIONS {
        let tools = harness::ToolDispatcher::list_tools()
            .into_iter()
            .map(|tool| serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters
                }
            }))
            .collect::<Vec<_>>();
        let result = client::query_llm_stream_with_tools(
            &provider,
            &model_id,
            &messages,
            key.as_deref(),
            base_url.as_deref(),
            &tools,
            |_| {},
        )
        .await?;

        if result.len() > MAX_AGENT_RESPONSE_BYTES {
            return Err("Model response exceeded the maximum allowed size".to_string());
        }

        let calls = harness::ToolDispatcher::parse_tool_calls(&result);
        if calls.is_empty() {
            let cleaned = harness::ToolDispatcher::strip_tool_calls(&result);
            let final_reply = if cleaned.is_empty() {
                if result.trim().is_empty() {
                    final_text
                } else {
                    result
                }
            } else {
                cleaned
            };
            let _ = emitter.send(StreamChunk {
                text: final_reply.clone(),
                done: false,
                error: None,
                approval: None,
                step: None,
            });
            return Ok(final_reply);
        }

        let native_tool_calls = result.trim_start().starts_with("<tool_call>")
            && result.contains("</tool_call>")
            && result
                .trim_start()
                .strip_prefix("<tool_call>")
                .and_then(|payload| payload.lines().next())
                .is_none_or(|first_line| !first_line.trim().is_empty());
        let tool_calls_repr = if native_tool_calls {
            serde_json::json!({"role": "assistant", "content": null, "tool_calls": calls.iter().map(|call| serde_json::json!({"id": call.id, "type": "function", "function": {"name": call.name, "arguments": serde_json::to_string(&call.arguments).unwrap_or_else(|_| "{}".into())}})).collect::<Vec<_>>() }).to_string()
        } else {
            calls
                .iter()
                .map(|call| format!("<tool_call>\n{}\n</tool_call>", serde_json::json!({"name": call.name, "arguments": call.arguments})))
                .collect::<Vec<_>>()
                .join("\n")
        };
        messages.push(client::ModelMessage::new("assistant", &tool_calls_repr));
        for call in calls {
            if let Err(error) = validate_tool_call(&call) {
                messages.push(client::ModelMessage::new(
                    "system",
                    &format!("Tool {} rejected: {}", call.name, error),
                ));
                continue;
            }

            if requires_approval(&call.name) {
                let approval_id = {
                    let mut pending = approvals.lock().map_err(|e| e.to_string())?;
                    pending.next_id += 1;
                    format!("approval-{}", pending.next_id)
                };
                let (sender, receiver) = tokio::sync::oneshot::channel::<ApprovalDecision>();
                approvals
                    .lock()
                    .map_err(|e| e.to_string())?
                    .waiting
                    .insert(approval_id.clone(), sender);
                let request = ApprovalRequest {
                    id: approval_id.clone(),
                    tool: call.name.clone(),
                    arguments: call.arguments.clone(),
                };
                let _ = emitter.send(StreamChunk {
                    text: String::new(),
                    done: false,
                    error: None,
                    approval: Some(request),
                    step: None,
                });
                let step_id = call.id.clone();
                let label = tool_label(&call.name, &call.arguments);
                let _ = emitter.send(StreamChunk {
                    text: String::new(),
                    done: false,
                    error: None,
                    approval: None,
                    step: Some(StepEvent { id: step_id.clone(), kind: tool_kind(&call.name).into(), label: label.clone(), status: "approval".into(), detail: None }),
                });
                let decision = receiver
                    .await
                    .map_err(|_| "Approval request was cancelled".to_string())?;
                let _ = emitter.send(StreamChunk {
                    text: String::new(),
                    done: false,
                    error: None,
                    approval: None,
                    step: Some(StepEvent { id: step_id, kind: tool_kind(&call.name).into(), label, status: if decision.allowed { "running" } else { "failed" }.into(), detail: None }),
                });
                if !decision.allowed {
                    let observation = decision
                        .response
                        .unwrap_or_else(|| "User declined this operation".to_string());
                    messages.push(client::ModelMessage::new(
                        "system",
                        &format!("Tool {} not run: {}", call.name, observation),
                    ));
                    continue;
                }
            } else {
                let _ = emitter.send(StreamChunk {
                    text: String::new(),
                    done: false,
                    error: None,
                    approval: None,
                    step: Some(StepEvent { id: call.id.clone(), kind: tool_kind(&call.name).into(), label: tool_label(&call.name, &call.arguments), status: "running".into(), detail: None }),
                });
            }

            let root = project_root.clone();
            let call_clone = call.clone();
            let tool_result = tokio::task::spawn_blocking(move || {
                harness::ToolDispatcher::execute_tool(&root, &call_clone, true)
            })
            .await
            .map_err(|e| format!("Tool task failed: {}", e))?;
            let status = if tool_result.success { "done" } else { "error" };
            let detail = if tool_result.success {
                let snippet = tool_result.output.trim().lines().next().unwrap_or("");
                if snippet.is_empty() { None } else { Some(clamp_string(snippet.to_string(), 80)) }
            } else {
                tool_result.error.as_ref().map(|e| clamp_string(e.clone(), 80))
            };
            let _ = emitter.send(StreamChunk {
                text: String::new(),
                done: false,
                error: None,
                approval: None,
                step: Some(StepEvent {
                    id: call.id.clone(),
                    kind: tool_kind(&call.name).into(),
                    label: tool_label(&call.name, &call.arguments),
                    status: status.into(),
                    detail,
                }),
            });
            let mut observation = harness::ToolDispatcher::format_observation(&tool_result);
            observation = clamp_string(observation, MAX_TOOL_OUTPUT_BYTES);
            messages.push(client::ModelMessage::new("system", &observation));
        }
    }

    Err("Agent stopped after reaching the maximum tool iteration count".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalReply {
    id: String,
    allowed: bool,
    response: Option<String>,
}

#[tauri::command]
fn respond_to_tool_approval(
    reply: ApprovalReply,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pending = state.pending_approvals.lock().map_err(|e| e.to_string())?;
    let sender = pending
        .waiting
        .remove(&reply.id)
        .ok_or_else(|| format!("Approval not found: {}", reply.id))?;
    sender
        .send(ApprovalDecision {
            allowed: reply.allowed,
            response: reply.response,
        })
        .map_err(|_| "Agent no longer waiting for approval".to_string())
}

#[tauri::command]
fn tool_list() -> Vec<harness::ToolDefinition> {
    harness::ToolDispatcher::list_tools()
}

#[tauri::command]
fn workspace_read_file(
    project_id: String,
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let root = state.project_root(&project_id)?;
    harness::Sandbox::new(root)?.read_file(&path, offset, limit)
}

#[tauri::command]
fn workspace_list_dir(
    project_id: String,
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<harness::FileEntry>, String> {
    let root = state.project_root(&project_id)?;
    harness::Sandbox::new(root)?.list_dir(path.as_deref())
}

#[tauri::command]
fn workspace_grep(
    project_id: String,
    query: String,
    pattern: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<harness::GrepMatch>, String> {
    let root = state.project_root(&project_id)?;
    harness::Sandbox::new(root)?.grep(&query, pattern.as_deref())
}

#[tauri::command]
#[allow(dead_code)]
fn workspace_run_command(
    project_id: String,
    command: String,
    timeout_secs: Option<u64>,
    state: State<'_, AppState>,
) -> Result<harness::CommandResult, String> {
    let root = state.project_root(&project_id)?;
    harness::ExecutionEngine::run_command(&root, &command, timeout_secs)
}

#[tauri::command]
fn workspace_git_status(project_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let root = state.project_root(&project_id)?;
    harness::GitContext::status(&root)
}

#[tauri::command]
fn workspace_git_diff(
    project_id: String,
    staged: Option<bool>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let root = state.project_root(&project_id)?;
    harness::GitContext::diff(&root, staged.unwrap_or(false))
}

#[tauri::command]
fn workspace_git_log(
    project_id: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let root = state.project_root(&project_id)?;
    harness::GitContext::log(&root, limit)
}

#[tauri::command]
fn workspace_git_branch(project_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let root = state.project_root(&project_id)?;
    harness::GitContext::branch(&root)
}

#[tauri::command]
async fn chat_send(
    prompt: String,
    model_id: String,
    provider: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ChatResponse, String> {
    state
        .chat(&prompt, &model_id, &provider, project_id.as_deref())
        .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn chat_send_stream(
    prompt: String,
    model_id: String,
    provider: String,
    project_id: Option<String>,
    thread_id: Option<String>,
    attachments: Option<Vec<String>>,
    channel: Channel<StreamChunk>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let stream_id = state
        .next_stream_id
        .fetch_add(1, Ordering::Relaxed)
        .to_string();
    let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Err("Prompt cannot be empty".into());
    }
    state
        .streams
        .lock()
        .map_err(|e| format!("Lock failure: {}", e))?
        .insert(stream_id.clone(), cancel_tx);
    let key = match secrets::get_secret(&format!("provider_{}", provider)) {
        Ok(key) => key,
        Err(error) => {
            if let Ok(mut streams) = state.streams.lock() {
                streams.remove(&stream_id);
            }
            return Err(error);
        }
    };
    let saved = state
        .providers
        .lock()
        .map_err(|e| format!("Lock failure: {}", e))?
        .get(&provider)
        .cloned();
    let config_url = saved.as_ref().and_then(|item| item.base_url.clone());
    let history = if let Some(thread_id) = thread_id.as_deref() {
        state.get_thread(thread_id)?.messages
    } else {
        Vec::new()
    };
    let attachments_from_request = attachments.clone().unwrap_or_default();
    if let Some(thread_id) = thread_id.as_deref() {
        state.append_thread_message_with_attachments(
            thread_id,
            "user",
            trimmed,
            &attachments_from_request,
        )?;
    }
    let mut messages = history
        .iter()
        .map(|message| client::ModelMessage::new(&message.role, &message.text))
        .collect::<Vec<_>>();
    messages.push(client::ModelMessage::new("user", trimmed));
    let emitter = channel.clone();
    let provider_owned = provider.clone();
    let model_owned = model_id.clone();
    let key_owned = key.clone();
    let url_owned = config_url.clone();
    let project_root_opt = project_id
        .as_deref()
        .and_then(|id| state.project_root(id).ok())
        .or_else(|| {
            let projects = state.projects.lock().ok()?;
            let active = projects.iter().find(|p| p.active).or_else(|| projects.first())?;
            state.project_root(&active.id).ok()
        })
        .or_else(|| std::env::current_dir().ok());
    let approvals = state.pending_approvals.clone();
    let task_state = state.inner();
    let task = tokio::spawn(async move {
        let attachments = attachments_from_request;
        if let Some(root) = project_root_opt {
            if key_owned.is_some() || saved.is_some() || provider_owned == "Ollama" {
                let result = run_agent_loop(
                    provider_owned.clone(),
                    model_owned.clone(),
                    key_owned.clone(),
                    url_owned.clone(),
                    root,
                    messages,
                    emitter.clone(),
                    approvals,
                    attachments,
                )
                .await?;
                Ok(result)
            } else {
                let text = format!(
                    "Local OpenHarness Assistant (Model: {}):\n\nConfigure API access for {} in Settings to route requests to live inference.",
                    model_owned, provider_owned
                );
                    let _ = emitter.send(StreamChunk {
                        text: text.clone(),
                        done: false,
                        error: None,
                        approval: None,
                        step: None,
                    });
                Ok(text)
            }
        } else if key_owned.is_some() || saved.is_some() {
            client::query_llm_stream(
                &provider_owned,
                &model_owned,
                &messages,
                key_owned.as_deref(),
                url_owned.as_deref(),
                move |text| {
                     let _ = emitter.send(StreamChunk {
                        text,
                        done: false,
                        error: None,
                        approval: None,
                        step: None,
                    });
                },
            )
            .await
        } else {
            let text = format!(
                "Local OpenHarness Assistant (Model: {}):\n\nConfigure API access for {} in Settings to route requests to live inference.",
                model_owned, provider_owned
            );
            let _ = emitter.send(StreamChunk {
                text: text.clone(),
                done: false,
                error: None,
                approval: None,
                step: None,
            });
            Ok(text)
        }
    });
    let abort_handle = task.abort_handle();
    let result = tokio::select! {
        _ = &mut cancel_rx => { abort_handle.abort(); Err("Generation stopped".to_string()) }
        joined = task => joined.map_err(|error| format!("Stream task failed: {}", error))?,
    };
    if let Ok(text) = &result
        && let Some(thread_id) = thread_id.as_deref()
    {
        task_state.append_thread_message(thread_id, "assistant", text)?;
    }
    if let Ok(mut streams) = state.streams.lock() {
        streams.remove(&stream_id);
    }
    match result {
        Ok(_) => {
            channel
                .send(StreamChunk {
                    text: String::new(),
                    done: true,
                    error: None,
                    approval: None,
                    step: None,
                })
                .map_err(|e| e.to_string())?;
            Ok(stream_id)
        }
        Err(error) => {
            let _ = channel.send(StreamChunk {
                text: String::new(),
                done: true,
                error: Some(error.clone()),
                approval: None,
                step: None,
            });
            Err(error)
        }
    }
}

#[tauri::command]
fn cancel_chat_stream(stream_id: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut streams = state
        .streams
        .lock()
        .map_err(|e| format!("Lock failure: {}", e))?;
    if let Some(id) = stream_id.filter(|s| !s.is_empty())
        && let Some(token) = streams.remove(&id)
    {
        token.send(()).ok();
        return Ok(());
    }
    if streams.is_empty() {
        return Err("Stream not found or already finished".to_string());
    }
    for (_, token) in streams.drain() {
        token.send(()).ok();
    }
    Ok(())
}

#[tauri::command]
async fn test_provider_connection(
    provider: String,
    model_id: String,
    base_url: Option<String>,
    api_key: String,
) -> Result<String, String> {
    let test_messages = vec![client::ModelMessage::new("user", "Hello")];
    client::query_llm(
        &provider,
        &model_id,
        &test_messages,
        Some(&api_key),
        base_url.as_deref(),
    )
    .await?;
    Ok("Connection successful".into())
}

fn read_terminal_output(stdout: std::process::ChildStdout, app: tauri::AppHandle, id: String) {
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut buffer = [0u8; 4096];
        let mut accum = String::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let raw = String::from_utf8_lossy(&buffer[..n]);
                    accum.push_str(&raw);
                    let screens = [
                        "Warning: PowerShell detected that you might be using a screen reader and has disabled PSReadLine for compatibility purposes. If you want to re-enable it, run 'Import-Module PSReadLine'.\r\n\r\n",
                        "Warning: PowerShell detected that you might be using a screen reader and has disabled PSReadLine for compatibility purposes. If you want to re-enable it, run 'Import-Module PSReadLine'.\n\n",
                        "Warning: PowerShell detected that you might be using a screen reader and has disabled PSReadLine for compatibility purposes. If you want to re-enable it, run 'Import-Module PSReadLine'.",
                    ];
                    let mut output = accum.clone();
                    for screen in screens {
                        output = output.replace(screen, "");
                    }
                    accum.clear();
                    if !output.is_empty() {
                        let _ = app.emit(
                            "terminal-output",
                            serde_json::json!({
                                "id": id,
                                "data": output,
                            }),
                        );
                    }
                }
                Err(_) => break,
            }
        }
    });
}

#[tauri::command]
fn terminal_create(
    project_id: Option<String>,
    shell: Option<String>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let working_dir = if let Some(project_id) = project_id {
        state.project_root(&project_id)?
    } else {
        std::env::current_dir().map_err(|e| e.to_string())?
    };
    let shell = shell.unwrap_or_else(|| "powershell".to_string());
    let mut command = if shell.eq_ignore_ascii_case("cmd") {
        let mut command = Command::new("cmd.exe");
        command.arg("/D");
        command
    } else {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NoExit",
            "-Command",
            "function global:prompt { '' }; $ProgressPreference = 'SilentlyContinue'; $WarningPreference = 'SilentlyContinue'; -",
        ]);
        command
    };
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .current_dir(&working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "terminal stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "terminal stdout unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "terminal stderr unavailable".to_string())?;
    let id = format!(
        "terminal-{}",
        state.next_stream_id.fetch_add(1, Ordering::Relaxed)
    );
    let app_stdout = app.clone();
    let app_stderr = app.clone();
    let id_stdout = id.clone();
    let id_stderr = id.clone();
    read_terminal_output(stdout, app_stdout, id_stdout);
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stderr);
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = app_stderr.emit(
                        "terminal-output",
                        serde_json::json!({
                            "id": id_stderr,
                            "data": String::from_utf8_lossy(&buffer[..n]),
                        }),
                    );
                }
                Err(_) => break,
            }
        }
    });
    state
        .terminals
        .lock()
        .map_err(|e| format!("Lock failure: {}", e))?
        .insert(
            id.clone(),
            Arc::new(Mutex::new(TerminalSession { child, stdin })),
        );
    spawn_terminal_monitor(app, id.clone(), state.terminals.clone());
    Ok(id)
}

fn spawn_terminal_monitor(
    app: tauri::AppHandle,
    id: String,
    terminals: Arc<Mutex<HashMap<String, Arc<Mutex<TerminalSession>>>>>,
) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let session = match terminals.lock() {
                Ok(map) => map.get(&id).cloned(),
                Err(_) => None,
            };
            let Some(session) = session else { return };
            let status = match session.lock() {
                Ok(mut session) => session.child.try_wait(),
                Err(_) => return,
            };
            match status {
                Ok(Some(status)) => {
                    let _ = app.emit(
                        "terminal-exit",
                        serde_json::json!({ "id": id.clone(), "code": status.code() }),
                    );
                    if let Ok(mut map) = terminals.lock() {
                        map.remove(&id);
                    }
                    return;
                }
                Ok(None) => {}
                Err(_) => return,
            }
        }
    });
}

#[tauri::command]
fn terminal_write(id: String, data: String, state: State<'_, AppState>) -> Result<(), String> {
    let session = {
        let map = state
            .terminals
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        map.get(&id)
            .cloned()
            .ok_or_else(|| "terminal not found".to_string())?
    };
    let mut session = session.lock().map_err(|e| format!("Lock failure: {}", e))?;
    session
        .stdin
        .write_all(data.as_bytes())
        .map_err(|e| e.to_string())?;
    session.stdin.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn terminal_kill(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let session = {
        let mut map = state
            .terminals
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        map.remove(&id)
            .ok_or_else(|| "terminal not found".to_string())?
    };
    let mut session = session.lock().map_err(|e| format!("Lock failure: {}", e))?;
    let _ = session.stdin.flush();
    let _ = session.stdin.write_all(b"\r\nexit\r\n");
    let _ = session.stdin.flush();
    let _ = session.child.kill();
    let _ = session.child.wait();
    Ok(())
}

#[tauri::command]
fn terminal_list(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state
        .terminals
        .lock()
        .map_err(|e| format!("Lock failure: {}", e))?
        .keys()
        .cloned()
        .collect())
}

#[tauri::command]
fn app_minimize(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn app_toggle_maximize(window: tauri::Window) -> Result<(), String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn app_close(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            provider_save,
            terminal_create,
            terminal_write,
            terminal_kill,
            terminal_list,
            provider_list,
            provider_delete,
            get_projects,
            select_project,
            create_project,
            rename_project,
            delete_project,
            relink_project,
            list_threads,
            create_thread,
            delete_thread,
            rename_thread,
            get_thread,
            append_thread_message,
            edit_thread_message,
            delete_thread_message,
            chat_send,
            chat_send_stream,
            cancel_chat_stream,
            test_provider_connection,
            tool_list,
            respond_to_tool_approval,
            workspace_read_file,
            workspace_list_dir,
            workspace_grep,
            workspace_git_status,
            workspace_git_diff,
            workspace_git_log,
            workspace_git_branch,
            app_minimize,
            app_toggle_maximize,
            app_close,
        ])
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            app.state::<AppState>().load(app_dir)?;
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_defaults() {
        let state = AppState::default();
        let projects = state.projects.lock().unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_provider_flow() {
        let state = AppState::default();
        let save_res = state.save_provider(
            "OpenRouter".into(),
            "test-model".into(),
            Some("https://api.test.com".into()),
            true,
            "test-secret-key-123".into(),
        );
        assert!(save_res.is_ok());

        let list_res = state.list_providers().unwrap();
        assert_eq!(list_res.len(), 1);
        assert_eq!(list_res[0].model_id, "test-model");
        assert!(list_res[0].has_key);

        let del_res = state.delete_provider("OpenRouter");
        assert!(del_res.is_ok());
        let list_after = state.list_providers().unwrap();
        assert_eq!(list_after.len(), 0);
    }

    #[test]
    fn test_project_management() {
        let state = AppState::default();
        let new_proj = state.create_project("Alpha Project".into(), None).unwrap();
        assert_eq!(new_proj.name, "Alpha Project");
        assert!(new_proj.active);

        let projects = state.get_projects().unwrap();
        assert_eq!(projects[0].name, "Alpha Project");
    }

    #[test]
    fn test_project_editing() {
        let state = AppState::default();
        let proj = state.create_project("Edit Me".into(), None).unwrap();
        let renamed = state.rename_project(&proj.id, "Renamed").unwrap();
        assert_eq!(renamed[0].name, "Renamed");
        let after_delete = state.delete_project(&proj.id).unwrap();
        assert!(after_delete.is_empty());
    }

    #[test]
    fn test_thread_management() {
        let state = AppState::default();
        let proj = state.create_project("Threads".into(), None).unwrap();
        let thread = state.create_thread(&proj.id).unwrap();
        let message = state
            .append_thread_message(&thread.id, "user", "hello")
            .unwrap();
        let fetched = state.get_thread(&thread.id).unwrap();
        assert_eq!(fetched.messages.len(), 1);
        assert_eq!(fetched.title, "hello");
        let renamed = state.rename_thread(&thread.id, "A better title").unwrap();
        assert_eq!(renamed[0].title, "A better title");
        assert!(state.rename_thread(&thread.id, "  ").is_err());
        let threads = state.list_threads(Some(&proj.id)).unwrap();
        assert_eq!(threads.len(), 1);

        let edited = state
            .edit_thread_message(&thread.id, &message.id, "edited text")
            .unwrap();
        assert_eq!(edited.text, "edited text");
        let after_edit = state.get_thread(&thread.id).unwrap();
        assert_eq!(after_edit.messages[0].text, "edited text");

        let remaining = state
            .delete_thread_message(&thread.id, &message.id)
            .unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_endpoint_validation_security() {
        assert!(client::validated_endpoint_for_test("Ollama", None).is_ok());
        assert!(client::validated_endpoint_for_test("OpenRouter", None).is_ok());
        assert!(
            client::validated_endpoint_for_test("OpenRouter", Some("http://192.168.1.5:8000"))
                .is_err()
        );
        assert!(
            client::validated_endpoint_for_test(
                "OpenRouter",
                Some("https://openrouter.ai/api/v1/chat/completions")
            )
            .is_ok()
        );
        assert!(
            client::validated_endpoint_for_test("Ollama", Some("http://localhost:11434/api/chat"))
                .is_ok()
        );
        assert!(
            client::validated_endpoint_for_test("Ollama", Some("http://127.0.0.1:11434/api/chat"))
                .is_ok()
        );
    }

    #[test]
    fn test_step_event_and_tool_helpers() {
        assert_eq!(tool_kind("read_file"), "read");
        assert_eq!(tool_kind("write_file"), "edit");
        assert_eq!(tool_kind("grep_search"), "grep");
        assert_eq!(tool_kind("run_command"), "command");
        assert_eq!(tool_kind("git_diff"), "git");

        let args = serde_json::json!({ "path": "src/App.tsx" });
        assert_eq!(tool_label("read_file", &args), "read file src/App.tsx");

        let step = StepEvent {
            id: "step_1".to_string(),
            kind: "read".to_string(),
            label: "read file src/App.tsx".to_string(),
            status: "done".to_string(),
            detail: Some("12 lines read".to_string()),
        };
        let serialized = serde_json::to_string(&step).unwrap();
        assert!(serialized.contains("\"kind\":\"read\""));
        assert!(serialized.contains("\"status\":\"done\""));
        assert!(serialized.contains("\"detail\":\"12 lines read\""));
    }
}



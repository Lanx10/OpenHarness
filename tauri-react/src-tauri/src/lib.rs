use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

mod client;
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

pub struct AppState {
    pub providers: Mutex<HashMap<String, ProviderInfo>>,
    pub projects: Mutex<Vec<ProjectInfo>>,
    app_data_dir: Mutex<Option<PathBuf>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            providers: Mutex::new(HashMap::new()),
            projects: Mutex::new(Vec::new()),
            app_data_dir: Mutex::new(None),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedState {
    providers: HashMap<String, ProviderInfo>,
    projects: Vec<ProjectInfo>,
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
        if provider.trim().is_empty() || model_id.trim().is_empty() || api_key.trim().is_empty() {
            return Err("Provider, model ID, and API key are required".into());
        }

        secrets::save_secret(&format!("provider_{}", provider), &api_key)?;

        let info = ProviderInfo {
            provider: provider.clone(),
            model_id,
            base_url,
            custom_endpoint,
            has_key: true,
        };

        let mut guard = self
            .providers
            .lock()
            .map_err(|e| format!("Lock failure: {}", e))?;
        guard.insert(provider, info.clone());
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

        // ponytail: canonical provider IDs are Anthropic-compatible / OpenAI-compatible / OpenRouter / Ollama / Google AI Studio; upgrade path is to normalize at API entry.
        let canonical_provider = match provider {
            "Anthropic" => "Anthropic-compatible",
            "OpenAI" => "OpenAI-compatible",
            other => other,
        };
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
            client::query_llm(
                canonical_provider,
                model_id,
                trimmed,
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
            provider_list,
            provider_delete,
            get_projects,
            select_project,
            create_project,
            chat_send,
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
            "TestProvider".into(),
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

        let del_res = state.delete_provider("TestProvider");
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
}

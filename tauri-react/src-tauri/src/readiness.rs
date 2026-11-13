use crate::harness::ProcessRegistry;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessStatus {
    Initializing,
    Ready,
    Degraded,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessComponent {
    pub id: String,
    pub display_name: String,
    pub status: ReadinessStatus,
    pub message: String,
    pub required: bool,
    pub recoverable: bool,
    pub validated_at_unix_ms: u64,
    pub duration_ms: u64,
}

impl ReadinessComponent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        status: ReadinessStatus,
        message: impl Into<String>,
        required: bool,
        recoverable: bool,
        validated_at_unix_ms: u64,
        duration_ms: u64,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            status,
            message: message.into(),
            required,
            recoverable,
            validated_at_unix_ms,
            duration_ms,
        }
    }

    pub fn record(
        &mut self,
        status: ReadinessStatus,
        message: impl Into<String>,
        started: Instant,
    ) {
        self.status = status;
        self.message = message.into();
        self.validated_at_unix_ms = unix_time_ms();
        self.duration_ms = started.elapsed().as_millis() as u64;
    }
}

#[derive(Debug, Clone)]
pub struct ReadinessReport {
    components: Vec<ReadinessComponent>,
}

impl ReadinessReport {
    #[cfg(test)]
    pub fn evaluate(root: &Path, shutdown_requested: bool) -> Self {
        Self::evaluate_with_process_registry(root, shutdown_requested, &ProcessRegistry::default())
    }

    pub fn evaluate_with_process_registry(
        root: &Path,
        shutdown_requested: bool,
        process_registry: &ProcessRegistry,
    ) -> Self {
        let now = unix_time_ms();
        let root = root.to_path_buf();
        let state_path = root.join("state.json");
        let mut components = Vec::new();

        let config_start = Instant::now();
        components.push(evaluate_configuration(
            &root,
            &state_path,
            now,
            config_start.elapsed(),
        ));
        let provider_start = Instant::now();
        components.push(evaluate_provider(
            &state_path,
            now,
            provider_start.elapsed(),
        ));
        components.push(check(
            "provider_connectivity",
            "Provider connectivity",
            false,
            true,
            now,
            Duration::from_millis(0),
        ));
        let tool_start = Instant::now();
        components.push(evaluate_tool_registry(now, tool_start.elapsed()));
        let skill_start = Instant::now();
        let global_skills_root = crate::user_home_dir().ok();
        components.push(evaluate_skills(
            global_skills_root.as_deref().unwrap_or(&root),
            now,
            skill_start.elapsed(),
        ));
        components.push(evaluate_skill_activation(
            None,
            global_skills_root.as_deref(),
            now,
        ));
        if let (true, Some(skill)) = (
            global_skills_root.is_none(),
            components
                .iter_mut()
                .find(|item| item.id == "skill_discovery"),
        ) {
            skill.status = ReadinessStatus::Degraded;
            skill.message = "user skill root unavailable".into();
        }
        components.push(evaluate_conversation_persistence(&root, now));
        components.push(evaluate_memory(&root, now));
        components.push(evaluate_filesystem_sandbox(
            &root,
            now,
            Duration::from_millis(0),
        ));
        components.push(evaluate_terminal_processes(process_registry, now));
        components.push(check(
            "structured_logging",
            "Structured logging",
            true,
            false,
            now,
            Duration::from_millis(0),
        ));
        components.push(check(
            "audit_persistence",
            "Audit persistence",
            true,
            true,
            now,
            Duration::from_millis(0),
        ));
        let directories_start = Instant::now();
        components.push(evaluate_data_directories(
            &root,
            now,
            directories_start.elapsed(),
        ));

        if shutdown_requested {
            for item in &mut components {
                if item.id != "provider_connectivity" {
                    item.status = ReadinessStatus::Unavailable;
                    item.message =
                        "application shutdown requested during readiness evaluation".into();
                }
            }
        }

        Self { components }
    }

    #[cfg(test)]
    pub fn is_fully_ready(&self) -> bool {
        self.components.iter().all(|item| {
            item.status == ReadinessStatus::Ready
                || (!item.required && item.status != ReadinessStatus::Failed)
        })
    }

    #[cfg(test)]
    pub fn require(&self, id: &str) -> Result<(), String> {
        match self.components.iter().find(|item| item.id == id) {
            Some(item)
                if item.required
                    && !matches!(
                        item.status,
                        ReadinessStatus::Ready | ReadinessStatus::Degraded
                    ) =>
            {
                Err(format!(
                    "{} is not ready: {}",
                    item.display_name, item.message
                ))
            }
            Some(_) => Ok(()),
            None => Err(format!("readiness component is missing: {id}")),
        }
    }

    #[cfg(test)]
    pub fn recover(
        &mut self,
        id: &str,
        status: ReadinessStatus,
        message: impl Into<String>,
        now_unix_ms: u64,
    ) {
        if let Some(item) = self.components.iter_mut().find(|item| item.id == id) {
            item.status = status;
            item.message = message.into();
            item.validated_at_unix_ms = now_unix_ms;
            item.recoverable = true;
        }
    }

    pub fn into_components(self) -> Vec<ReadinessComponent> {
        self.components
    }
}

fn evaluate_configuration(
    root: &Path,
    state_path: &Path,
    now: u64,
    elapsed: Duration,
) -> ReadinessComponent {
    match fs::read_to_string(state_path) {
        Ok(contents) => match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(value) if value.is_object() => component(
                "configuration",
                "Application configuration",
                ReadinessStatus::Ready,
                "application state parsed",
                true,
                true,
                now,
                elapsed,
            ),
            Ok(_) => component(
                "configuration",
                "Application configuration",
                ReadinessStatus::Failed,
                "application state must contain a JSON object",
                true,
                true,
                now,
                elapsed,
            ),
            Err(error) => component(
                "configuration",
                "Application configuration",
                ReadinessStatus::Failed,
                format!("application state contains invalid JSON: {error}"),
                true,
                true,
                now,
                elapsed,
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if root.is_dir() {
                component(
                    "configuration",
                    "Application configuration",
                    ReadinessStatus::Ready,
                    "application state will be created",
                    true,
                    true,
                    now,
                    elapsed,
                )
            } else {
                component(
                    "configuration",
                    "Application configuration",
                    ReadinessStatus::Failed,
                    format!("data directory missing: {error}"),
                    true,
                    true,
                    now,
                    elapsed,
                )
            }
        }
        Err(error) => component(
            "configuration",
            "Application configuration",
            ReadinessStatus::Failed,
            format!("application state unreadable: {error}"),
            true,
            true,
            now,
            elapsed,
        ),
    }
}

pub fn evaluate_terminal_processes(registry: &ProcessRegistry, now: u64) -> ReadinessComponent {
    let started = Instant::now();
    match registry.health() {
        Ok(()) => component(
            "terminal_processes",
            "Terminal/process management",
            ReadinessStatus::Ready,
            "process registry initialized and available",
            true,
            true,
            now,
            started.elapsed(),
        ),
        Err(error) => component(
            "terminal_processes",
            "Terminal/process management",
            ReadinessStatus::Failed,
            format!("process registry unavailable: {error}"),
            true,
            true,
            now,
            started.elapsed(),
        ),
    }
}

fn evaluate_memory(root: &Path, now: u64) -> ReadinessComponent {
    evaluate_memory_at(root.join("memory"), now)
}

pub(crate) fn evaluate_memory_at(memory_dir: PathBuf, now: u64) -> ReadinessComponent {
    let started = Instant::now();
    match fs::create_dir_all(&memory_dir)
        .map_err(|error| format!("memory directory create failed: {error}"))
        .and_then(|_| {
            let probe = memory_dir.join(format!(".readiness-{}", uuid::Uuid::new_v4()));
            fs::write(&probe, "memory readiness probe")
                .map_err(|error| format!("memory write failed: {error}"))
                .and_then(|_| {
                    fs::read_to_string(&probe)
                        .map_err(|error| format!("memory read failed: {error}"))
                        .map(|_| ())
                })
                .and_then(|_| {
                    fs::remove_file(&probe)
                        .map_err(|error| format!("memory cleanup failed: {error}"))
                })
        }) {
        Ok(()) => component(
            "memory",
            "Memory subsystem",
            ReadinessStatus::Ready,
            "memory directory is writable and readable",
            true,
            true,
            now,
            started.elapsed(),
        ),
        Err(error) => component(
            "memory",
            "Memory subsystem",
            ReadinessStatus::Failed,
            error,
            true,
            true,
            now,
            started.elapsed(),
        ),
    }
}

fn evaluate_conversation_persistence(root: &Path, now: u64) -> ReadinessComponent {
    let started = Instant::now();
    let probe = root.join(format!(".readiness-state-{}", uuid::Uuid::new_v4()));
    let bytes = br#"{"providers":{},"projects":[],"threads":[],"approval_mode":"manual"}"#;
    let result = fs::write(&probe, bytes)
        .map_err(|error| format!("conversation persistence write probe failed: {error}"))
        .and_then(|_| {
            fs::read(&probe)
                .map_err(|error| format!("conversation persistence read probe failed: {error}"))
                .and_then(|loaded| {
                    serde_json::from_slice::<serde_json::Value>(&loaded)
                        .map(|_| ())
                        .map_err(|error| {
                            format!("conversation persistence JSON probe failed: {error}")
                        })
                })
        })
        .and_then(|_| {
            fs::remove_file(&probe)
                .map_err(|error| format!("conversation persistence probe cleanup failed: {error}"))
        });
    let (status, message) = match result {
        Ok(()) => (
            ReadinessStatus::Ready,
            "conversation state load/save probe succeeded".to_string(),
        ),
        Err(error) => (ReadinessStatus::Failed, error),
    };
    component(
        "conversation_persistence",
        "Conversation persistence",
        status,
        message,
        true,
        true,
        now,
        started.elapsed(),
    )
}

fn evaluate_skill_activation(
    project_root: Option<&Path>,
    global_root: Option<&Path>,
    now: u64,
) -> ReadinessComponent {
    let started = Instant::now();
    let result = crate::harness::ToolDispatcher::list_skills_from_roots(project_root, global_root)
        .map_err(|error| format!("skill discovery unavailable: {error}"))
        .and_then(|skills| {
            skills
                .iter()
                .find(|skill| skill.available)
                .map(|skill| skill.name.clone())
                .ok_or_else(|| "no available skill can be activated".to_string())
        })
        .and_then(|name| {
            crate::harness::ToolDispatcher::activate_skill_from_roots(
                project_root,
                global_root,
                &name,
                &[],
            )
            .map_err(|error| format!("skill activation unavailable: {error}"))
            .map(|_| name)
        });
    let (status, message) = match result {
        Ok(name) => (
            ReadinessStatus::Ready,
            format!("activated probe skill: {name}"),
        ),
        Err(error) => (ReadinessStatus::Degraded, error),
    };
    component(
        "skill_activation",
        "Skill activation infrastructure",
        status,
        message,
        false,
        true,
        now,
        started.elapsed(),
    )
}

fn evaluate_tool_registry(now: u64, elapsed: Duration) -> ReadinessComponent {
    let tools = crate::harness::ToolDispatcher::list_tools();
    if tools.is_empty() {
        return component(
            "tool_registry",
            "Tool registry",
            ReadinessStatus::Failed,
            "tool registry is empty",
            true,
            true,
            now,
            elapsed,
        );
    }
    component(
        "tool_registry",
        "Tool registry",
        ReadinessStatus::Ready,
        format!("{} tools registered", tools.len()),
        true,
        true,
        now,
        elapsed,
    )
}

fn evaluate_skills(root: &Path, now: u64, elapsed: Duration) -> ReadinessComponent {
    match crate::harness::ToolDispatcher::list_skills_from_roots(None, Some(root)) {
        Ok(skills) => component(
            "skill_discovery",
            "Skill discovery",
            ReadinessStatus::Ready,
            format!("{} skills discovered", skills.len()),
            false,
            true,
            now,
            elapsed,
        ),
        Err(error) => component(
            "skill_discovery",
            "Skill discovery",
            ReadinessStatus::Degraded,
            format!("skill discovery failed: {error}"),
            false,
            true,
            now,
            elapsed,
        ),
    }
}

pub(crate) fn evaluate_filesystem_sandbox(
    root: &Path,
    now: u64,
    elapsed: Duration,
) -> ReadinessComponent {
    let probe_name = format!(".readiness-sandbox-{}", uuid::Uuid::new_v4());
    let result = crate::harness::Sandbox::new(root.to_path_buf()).and_then(|sandbox| {
        sandbox
            .write_file(&probe_name, "readiness probe")
            .map_err(|error| format!("sandbox write probe failed: {error}"))?;
        let read = sandbox
            .read_file(&probe_name, None, None)
            .map_err(|error| format!("sandbox read probe failed: {error}"));
        let cleanup = fs::remove_file(root.join(&probe_name));
        if let Err(error) = cleanup {
            return Err(format!("sandbox probe cleanup failed: {error}"));
        }
        read.and_then(|contents| {
            if contents == "readiness probe" {
                Ok(())
            } else {
                Err("sandbox read probe returned unexpected contents".into())
            }
        })
    });
    match result {
        Ok(()) => component(
            "filesystem_sandbox",
            "Filesystem sandbox",
            ReadinessStatus::Ready,
            "sandbox write/read probe succeeded",
            true,
            true,
            now,
            elapsed,
        ),
        Err(error) => component(
            "filesystem_sandbox",
            "Filesystem sandbox",
            ReadinessStatus::Failed,
            error,
            true,
            true,
            now,
            elapsed,
        ),
    }
}

fn evaluate_data_directories(root: &Path, now: u64, elapsed: Duration) -> ReadinessComponent {
    if !root.is_dir() {
        return component(
            "data_directories",
            "Required application data directories",
            ReadinessStatus::Failed,
            format!(
                "data directory is missing or not a directory: {}",
                root.display()
            ),
            true,
            true,
            now,
            elapsed,
        );
    }
    let probe = root.join(format!(".readiness-{}.tmp", uuid::Uuid::new_v4()));
    match fs::write(&probe, b"readiness probe") {
        Ok(()) => {
            let _ = fs::remove_file(probe);
            component(
                "data_directories",
                "Required application data directories",
                ReadinessStatus::Ready,
                "application data directory exists and is writable",
                true,
                true,
                now,
                elapsed,
            )
        }
        Err(error) => component(
            "data_directories",
            "Required application data directories",
            ReadinessStatus::Failed,
            format!("application data directory is not writable: {error}"),
            true,
            true,
            now,
            elapsed,
        ),
    }
}

fn evaluate_provider(state_path: &Path, now: u64, elapsed: Duration) -> ReadinessComponent {
    let configured = fs::read_to_string(state_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("providers")
                .filter(|providers| providers.is_object())
                .and_then(|p| p.get("selected"))
                .and_then(|s| s.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
        })
        .is_some();

    component(
        "provider_configuration",
        "Provider configuration",
        if configured {
            ReadinessStatus::Ready
        } else {
            ReadinessStatus::Unavailable
        },
        if configured {
            "provider selected"
        } else {
            "no provider configured"
        },
        true,
        true,
        now,
        elapsed,
    )
}

fn check(
    id: &'static str,
    display_name: &'static str,
    required: bool,
    recoverable: bool,
    now: u64,
    elapsed: Duration,
) -> ReadinessComponent {
    component(
        id,
        display_name,
        ReadinessStatus::Initializing,
        "health check has not run; readiness cannot be confirmed",
        required,
        recoverable,
        now,
        elapsed,
    )
}

#[allow(clippy::too_many_arguments)]
fn component(
    id: impl Into<String>,
    display_name: impl Into<String>,
    status: ReadinessStatus,
    message: impl Into<String>,
    required: bool,
    recoverable: bool,
    now: u64,
    elapsed: Duration,
) -> ReadinessComponent {
    ReadinessComponent::new(
        id,
        display_name,
        status,
        message,
        required,
        recoverable,
        now,
        elapsed.as_millis() as u64,
    )
}

pub(crate) fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    fn temp_dir() -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("openharness-readiness-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn terminal_process_probe_reports_live_registry_without_claiming_child_processes() {
        let registry = ProcessRegistry::default();
        let report = ReadinessReport::evaluate_with_process_registry(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .as_path(),
            false,
            &registry,
        );
        let component = report
            .components
            .iter()
            .find(|item| item.id == "terminal_processes")
            .unwrap();
        assert_eq!(component.status, ReadinessStatus::Ready);
        assert!(component.message.contains("registry initialized"));
        assert!(!report.is_fully_ready());
    }

    #[test]
    fn skill_activation_probe_reports_degraded_without_available_skills() {
        let global = temp_dir();
        let component = evaluate_skill_activation(None, Some(&global), unix_time_ms());
        assert_eq!(component.status, ReadinessStatus::Degraded);
        assert!(component.message.contains("no available skill"));
        fs::remove_dir_all(global).unwrap();
    }

    #[test]
    fn incomplete_startup_does_not_report_full_readiness() {
        let root = temp_dir();
        fs::write(root.join("state.json"), r#"{"providers":{}}"#).unwrap();
        let report = ReadinessReport::evaluate(&root, false);
        assert!(!report.is_fully_ready());
        assert_eq!(report.components.len(), 13);
        assert!(
            report.components.iter().any(|item| {
                item.id == "tool_registry" && item.status == ReadinessStatus::Ready
            })
        );
        assert!(
            report.components.iter().any(|item| {
                item.id == "skill_discovery" && item.status == ReadinessStatus::Ready
            })
        );
        assert!(report.components.iter().all(|item| {
            matches!(
                item.status,
                ReadinessStatus::Ready
                    | ReadinessStatus::Initializing
                    | ReadinessStatus::Unavailable
                    | ReadinessStatus::Degraded
            )
        }));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn conversation_persistence_probe_round_trips_in_app_data_directory() {
        let root = temp_dir();
        let component = evaluate_conversation_persistence(&root, unix_time_ms());
        assert_eq!(component.status, ReadinessStatus::Ready);
        assert!(component.message.contains("load/save"));
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn memory_probe_fails_when_directory_cannot_be_written() {
        let root = temp_dir();
        fs::write(root.join("memory"), "not a directory").unwrap();
        let component = evaluate_memory(&root, unix_time_ms());
        assert_eq!(component.status, ReadinessStatus::Failed);
        assert!(component.message.contains("memory"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn skill_activation_probe_activates_a_discovered_skill() {
        let root = temp_dir();
        let skill_dir = root.join(".agents/skills/probe-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: probe-skill\ndescription: Probe\n---\nActivation verified.",
        )
        .unwrap();
        let component = evaluate_skill_activation(None, Some(&root), unix_time_ms());
        assert_eq!(component.status, ReadinessStatus::Ready);
        assert!(component.message.contains("probe-skill"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_sandbox_probes_use_isolated_files_and_share_state_safely() {
        let root = temp_dir();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let root = root.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    evaluate_filesystem_sandbox(&root, unix_time_ms(), Duration::ZERO)
                })
            })
            .collect();

        for handle in handles {
            let component = handle.join().unwrap();
            assert_eq!(component.status, ReadinessStatus::Ready);
        }
        assert!(!std::fs::read_dir(&root).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".readiness-sandbox-")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn filesystem_sandbox_probe_recovers_after_root_becomes_writable() {
        let root = temp_dir();
        let blocker = root.join("not-a-directory");
        fs::write(&blocker, "blocker").unwrap();
        let failed = evaluate_filesystem_sandbox(&blocker, unix_time_ms(), Duration::ZERO);
        assert_eq!(failed.status, ReadinessStatus::Failed);

        let recovered = evaluate_filesystem_sandbox(&root, unix_time_ms(), Duration::ZERO);
        assert_eq!(recovered.status, ReadinessStatus::Ready);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn filesystem_sandbox_probe_exercises_a_real_write_and_read() {
        let root = temp_dir();
        let sandbox = ReadinessReport::evaluate(&root, false);
        let component = sandbox
            .components
            .iter()
            .find(|item| item.id == "filesystem_sandbox")
            .unwrap();
        assert_eq!(component.status, ReadinessStatus::Ready);
        assert!(component.message.contains("write/read"));
        assert!(fs::read_dir(root.join("memory")).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_persisted_configuration_is_failed_with_repair_guidance() {
        let root = temp_dir();
        fs::write(root.join("state.json"), "{broken").unwrap();
        let report = ReadinessReport::evaluate(&root, false);
        let config = report
            .components
            .iter()
            .find(|item| item.id == "configuration")
            .unwrap();
        assert_eq!(config.status, ReadinessStatus::Failed);
        assert!(config.message.contains("invalid JSON"));
        assert!(!report.is_fully_ready());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_data_directory_is_failed() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "openharness-readiness-missing-{}",
                uuid::Uuid::new_v4()
            ));
        let report = ReadinessReport::evaluate(&root, false);
        assert_eq!(
            report
                .components
                .iter()
                .find(|item| item.id == "configuration")
                .unwrap()
                .status,
            ReadinessStatus::Failed
        );
    }

    #[test]
    fn missing_provider_is_not_total_success_and_recovery_is_reported() {
        let root = temp_dir();
        fs::write(root.join("state.json"), r#"{"providers":{}}"#).unwrap();
        fs::write(root.join("audit.jsonl"), "").unwrap();
        fs::create_dir_all(root.join("memory")).unwrap();
        let mut report = ReadinessReport::evaluate(&root, false);
        assert!(!report.is_fully_ready());
        assert_eq!(
            report
                .components
                .iter()
                .find(|item| item.id == "provider_configuration")
                .unwrap()
                .status,
            ReadinessStatus::Unavailable
        );
        assert!(report.require("provider_configuration").is_err());
        report.recover(
            "provider_configuration",
            ReadinessStatus::Ready,
            "provider configured",
            42,
        );
        assert_eq!(
            report
                .components
                .iter()
                .find(|item| item.id == "provider_configuration")
                .unwrap()
                .message,
            "provider configured"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn shutdown_during_initialization_marks_pending_components_unavailable() {
        let root = temp_dir();
        let report = ReadinessReport::evaluate(&root, true);
        assert!(!report.is_fully_ready());
        assert!(
            report
                .components
                .iter()
                .any(|item| item.status == ReadinessStatus::Unavailable)
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn serializes_frontend_status_contract_and_timing() {
        let item = ReadinessComponent::new(
            "configuration",
            "Application configuration",
            ReadinessStatus::Failed,
            "state file is unreadable",
            true,
            false,
            123,
            8,
        );
        let value = serde_json::to_value(item).unwrap();
        assert_eq!(value["status"], "failed");
        assert_eq!(value["displayName"], "Application configuration");
        assert_eq!(value["validatedAtUnixMs"], 123);
        assert_eq!(value["durationMs"], 8);
    }
}

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const RETAINED_FILES: usize = 5;
static AUDIT: OnceLock<AuditLog> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
/// Full event vocabulary is a versioned persistence contract. Some variants are
/// reserved for future emitters and are intentionally retained.
#[allow(dead_code)]
pub enum EventKind {
    AppStarted,
    AppShutdown,
    AgentStarted,
    AgentFinished,
    ProviderRequestStarted,
    ProviderRequestFinished,
    ProviderRequestFailed,
    ProviderUsageReceived,
    ToolRequested,
    ToolStarted,
    ToolRejected,
    ToolApproval,
    ToolFinished,
    SkillDiscovered,
    SkillActivationRequested,
    SkillActivated,
    SkillActivationFailed,
    SandboxOperation,
    StreamCreated,
    StreamStarted,
    StreamFinished,
    StreamCancelled,
    StreamCleaned,
    AgentFailed,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub schema_version: u8,
    pub timestamp_unix_ms: u64,
    pub event_id: String,
    pub correlation_id: String,
    pub conversation_id: Option<String>,
    pub request_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub entity: Option<String>,
    pub kind: EventKind,
    pub outcome: Option<String>,
}

struct AuditLog {
    directory: PathBuf,
    lock: Mutex<()>,
}

pub fn initialize(directory: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&directory).map_err(|e| format!("audit directory unavailable: {e}"))?;
    let directory = directory
        .canonicalize()
        .map_err(|e| format!("audit directory unavailable: {e}"))?;
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|e| format!("audit directory unavailable: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Security error: audit location must be a real directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("audit permissions unavailable: {e}"))?;
    }
    let _ = AUDIT.set(AuditLog {
        directory,
        lock: Mutex::new(()),
    });
    emit(
        EventKind::AppStarted,
        "application",
        None,
        None,
        None,
        None,
        Some("started"),
        None,
    );
    Ok(())
}

pub fn check_persistence() -> Result<(), String> {
    AUDIT
        .get()
        .ok_or_else(|| "audit log is not initialized".to_string())?
        .check_persistence()
}

impl AuditLog {
    fn check_persistence(&self) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "audit lock poisoned".to_string())?;
        let path = self.directory.join("audit.jsonl");
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err("Security error: Audit log must be a regular non-symlink file".into());
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| format!("audit log is not writable: {error}"))?;
        read_last_hash(&path).map(|_| ())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn emit(
    kind: EventKind,
    correlation_id: &str,
    conversation_id: Option<&str>,
    request_id: Option<&str>,
    provider: Option<&str>,
    model: Option<&str>,
    outcome: Option<&str>,
    entity: Option<&str>,
) {
    let Some(log) = AUDIT.get() else {
        log::warn!("event=audit_write_failed reason=audit log is not initialized");
        return;
    };
    // Schema fields are identifiers and fixed event values only; never pass prompts, arguments,
    // file contents, credentials, or arbitrary error strings here.
    if let Err(_error) = log.emit(
        kind,
        correlation_id,
        conversation_id,
        request_id,
        provider,
        model,
        outcome,
        entity,
    ) {
        log::warn!("event=audit_write_failed reason=append_failed");
    }
}

fn safe_id(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
        .take(128)
        .collect()
}
fn safe_outcome(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .take(48)
        .collect()
}

#[derive(Serialize)]
struct ChainedEvent<'a> {
    previous_hash: &'a str,
    hash: String,
    event: &'a AuditEvent,
}
#[derive(Serialize)]
struct HashInput<'a> {
    previous_hash: &'a str,
    event: &'a AuditEvent,
}

impl AuditLog {
    #[allow(clippy::too_many_arguments)]
    fn emit(
        &self,
        kind: EventKind,
        correlation_id: &str,
        conversation_id: Option<&str>,
        request_id: Option<&str>,
        provider: Option<&str>,
        model: Option<&str>,
        outcome: Option<&str>,
        entity: Option<&str>,
    ) -> Result<(), String> {
        let event = AuditEvent {
            schema_version: 1,
            timestamp_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default(),
            event_id: uuid::Uuid::new_v4().to_string(),
            correlation_id: safe_id(correlation_id),
            conversation_id: conversation_id.map(safe_id),
            request_id: request_id.map(safe_id),
            provider: provider.map(safe_id),
            model: model.map(safe_id),
            entity: entity.map(safe_id),
            kind,
            outcome: outcome.map(safe_outcome),
        };
        self.append(&event)
    }

    fn append(&self, event: &AuditEvent) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "audit lock poisoned".to_string())?;
        let path = self.directory.join("audit.jsonl");
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err("Security error: Audit log must be a regular non-symlink file".into());
        }
        let previous = read_last_hash(&path)?;
        let input = serde_json::to_vec(&HashInput {
            previous_hash: &previous,
            event,
        })
        .map_err(|e| e.to_string())?;
        let hash = format!("{:x}", Sha256::digest(input));
        let line = serde_json::to_vec(&ChainedEvent {
            previous_hash: &previous,
            hash,
            event,
        })
        .map_err(|e| e.to_string())?;
        if path.exists()
            && fs::metadata(&path).map_err(|e| e.to_string())?.len() + line.len() as u64 + 1
                > MAX_FILE_BYTES
        {
            self.rotate()?;
        }
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(&path).map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Security error: Audit log must be a regular file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        file.write_all(&line)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_data())
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn resolve_audit_component(&self, name: &str) -> Result<PathBuf, String> {
        let path = self.directory.join(name);
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(
                "Security error: Audit component must be a regular non-symlink file".into(),
            );
        }
        Ok(path)
    }

    fn rotate(&self) -> Result<(), String> {
        let oldest = self.resolve_audit_component(&format!("audit.{RETAINED_FILES}.jsonl"))?;
        match fs::remove_file(&oldest) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        for index in (1..RETAINED_FILES).rev() {
            let from = self.resolve_audit_component(&if index == 1 {
                "audit.jsonl".into()
            } else {
                format!("audit.{index}.jsonl")
            })?;
            let to = self.directory.join(format!("audit.{}.jsonl", index + 1));
            if from.exists() {
                fs::rename(from, to).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

fn read_last_hash(path: &Path) -> Result<String, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.to_string()),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("Security error: Audit log must be a regular non-symlink file".into());
        }
        Ok(_) => {}
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Security error: Audit log must be a regular file".into());
    }
    let mut last = String::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if !line.trim().is_empty() {
            last = line;
        }
    }
    if last.is_empty() {
        return Ok(String::new());
    }
    serde_json::from_str::<serde_json::Value>(&last)
        .map_err(|e| format!("audit recovery failed: {e}"))?
        .get("hash")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "audit recovery failed: missing chain hash".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_emit_writes_sanitized_event_record() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("openharness-audit-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let log = AuditLog {
            directory: dir.clone(),
            lock: Mutex::new(()),
        };

        log.emit(
            EventKind::AgentFinished,
            "run secret!",
            Some("conversation/id"),
            Some("request-1"),
            Some("provider"),
            Some("model"),
            Some("success!"),
            Some("entity/value"),
        )
        .unwrap();

        let line = fs::read_to_string(dir.join("audit.jsonl")).unwrap();
        let record: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        let event = &record["event"];
        assert_eq!(event["schema_version"], 1);
        assert_eq!(event["kind"], "agent_finished");
        assert_eq!(event["correlation_id"], "runsecret");
        assert_eq!(event["conversation_id"], "conversationid");
        assert_eq!(event["request_id"], "request-1");
        assert_eq!(event["outcome"], "success");
        assert!(!line.contains("secret!"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn append_rejects_audit_log_symlink_without_touching_target() {
        use std::os::unix::fs::symlink;
        let base = std::env::current_dir().unwrap().join("target");
        let dir = base.join(format!("openharness-audit-link-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let outside = dir.with_extension("outside");
        fs::write(&outside, "protected").unwrap();
        symlink(&outside, dir.join("audit.jsonl")).unwrap();
        let log = AuditLog {
            directory: dir.clone(),
            lock: Mutex::new(()),
        };
        let result = log.emit(
            EventKind::AgentFailed,
            "run",
            None,
            None,
            None,
            None,
            Some("failed"),
            None,
        );
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "protected");
        fs::remove_dir_all(dir).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[test]
    fn persistence_probe_requires_a_readable_audit_chain_and_writable_file() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("openharness-audit-probe-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let log = AuditLog {
            directory: dir.clone(),
            lock: Mutex::new(()),
        };

        assert!(log.check_persistence().is_ok());
        fs::write(dir.join("audit.jsonl"), "not-json\\n").unwrap();
        assert!(log.check_persistence().is_err());
        fs::write(dir.join("audit.jsonl"), "").unwrap();
        assert!(log.check_persistence().is_ok());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn isolated_emit_returns_storage_failures() {
        let directory = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("openharness-audit-file-{}", uuid::Uuid::new_v4()));
        fs::write(&directory, "not a directory").unwrap();
        let log = AuditLog {
            directory: directory.clone(),
            lock: Mutex::new(()),
        };

        let error = log
            .emit(
                EventKind::AgentFailed,
                "run",
                None,
                None,
                None,
                None,
                Some("failed"),
                None,
            )
            .unwrap_err();
        assert!(!error.is_empty());
        fs::remove_file(directory).unwrap();
    }

    #[test]
    fn appends_structured_chained_events_and_recovers_after_restart() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("openharness-audit-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let log = AuditLog {
            directory: dir.clone(),
            lock: Mutex::new(()),
        };
        log.append(&AuditEvent {
            schema_version: 1,
            timestamp_unix_ms: 1,
            event_id: "e1".into(),
            correlation_id: "run".into(),
            conversation_id: Some("c".into()),
            request_id: None,
            provider: Some("p".into()),
            model: Some("m".into()),
            entity: None,
            kind: EventKind::AgentStarted,
            outcome: None,
        })
        .unwrap();
        let previous = read_last_hash(&dir.join("audit.jsonl")).unwrap();
        assert_eq!(previous.len(), 64);
        log.append(&AuditEvent {
            schema_version: 1,
            timestamp_unix_ms: 2,
            event_id: "e2".into(),
            correlation_id: "run".into(),
            conversation_id: Some("c".into()),
            request_id: Some("r".into()),
            provider: None,
            model: None,
            entity: None,
            kind: EventKind::AgentFinished,
            outcome: Some("ok".into()),
        })
        .unwrap();
        let second: serde_json::Value = serde_json::from_str(
            fs::read_to_string(dir.join("audit.jsonl"))
                .unwrap()
                .lines()
                .nth(1)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(second["previous_hash"], previous);
        let _ = fs::remove_dir_all(dir);
    }
}

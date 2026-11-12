use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const RETAINED_FILES: usize = 5;
static AUDIT: OnceLock<AuditLog> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
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
    let Some(log) = AUDIT.get() else { return };
    // Schema fields are identifiers and fixed event values only; never pass prompts, arguments,
    // file contents, credentials, or arbitrary error strings here.
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
    if let Err(error) = log.append(&event) {
        log::warn!("event=audit_write_failed reason={error}");
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
    fn append(&self, event: &AuditEvent) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "audit lock poisoned".to_string())?;
        let path = self.directory.join("audit.jsonl");
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
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
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

    fn rotate(&self) -> Result<(), String> {
        let oldest = self
            .directory
            .join(format!("audit.{}.jsonl", RETAINED_FILES));
        let _ = fs::remove_file(oldest);
        for index in (1..RETAINED_FILES).rev() {
            let from = self.directory.join(if index == 1 {
                "audit.jsonl".into()
            } else {
                format!("audit.{}.jsonl", index)
            });
            let to = self.directory.join(format!("audit.{}.jsonl", index + 1));
            if from.exists() {
                fs::rename(from, to).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

fn read_last_hash(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok(String::new());
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
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
            &fs::read_to_string(dir.join("audit.jsonl"))
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

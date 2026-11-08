use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepMatch {
    pub file: String,
    pub line_number: usize,
    pub line_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

const MAX_READ_BYTES: usize = 2_000_000;
const MAX_GREP_MATCHES: usize = 100;
const MAX_DIR_ENTRIES: usize = 250;
const MAX_OUTPUT_BYTES: usize = 500_000;

pub struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("Invalid project directory '{}': {}", root.display(), e))?;
        if !canonical_root.is_dir() {
            return Err(format!(
                "Project path '{}' is not a directory",
                canonical_root.display()
            ));
        }
        Ok(Self {
            root: canonical_root,
        })
    }

    pub fn resolve_path(&self, rel_path: &str) -> Result<PathBuf, String> {
        let trimmed = rel_path.trim().trim_start_matches(['/', '\\']);
        if trimmed.is_empty() {
            return Ok(self.root.clone());
        }

        for component in Path::new(trimmed).components() {
            if let std::path::Component::ParentDir = component {
                return Err("Security error: Parent traversal '..' is forbidden".to_string());
            }
        }

        let candidate = self.root.join(trimmed);
        if candidate.exists() {
            let canonical = candidate.canonicalize().map_err(|e| {
                format!(
                    "Failed to canonicalize candidate '{}': {}",
                    candidate.display(),
                    e
                )
            })?;
            if !canonical.starts_with(&self.root) {
                return Err(format!(
                    "Security error: Path '{}' is outside project root",
                    rel_path
                ));
            }
            Ok(canonical)
        } else {
            let mut ancestor = candidate.clone();
            while !ancestor.exists() {
                if let Some(parent) = ancestor.parent() {
                    ancestor = parent.to_path_buf();
                } else {
                    break;
                }
            }
            if ancestor.exists() {
                let canonical_ancestor = ancestor.canonicalize().map_err(|e| {
                    format!(
                        "Failed to canonicalize ancestor '{}': {}",
                        ancestor.display(),
                        e
                    )
                })?;
                if !canonical_ancestor.starts_with(&self.root) {
                    return Err(
                        "Security error: Destination parent is outside project root".to_string()
                    );
                }
            }
            Ok(candidate)
        }
    }

    pub fn read_file(
        &self,
        rel_path: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        let trimmed = rel_path.trim();
        if trimmed.is_empty() {
            return Err("File path cannot be empty".to_string());
        }
        let full_path = self.resolve_path(trimmed)?;
        if !full_path.is_file() {
            return Err(format!("'{}' is not a file", rel_path));
        }
        let sym_meta = fs::symlink_metadata(&full_path)
            .map_err(|e| format!("Failed to read metadata for '{}': {}", rel_path, e))?;
        if sym_meta.file_type().is_symlink() {
            return Err("Security error: Reading symlinks is forbidden".to_string());
        }

        let metadata = fs::metadata(&full_path)
            .map_err(|e| format!("Failed to read file metadata for '{}': {}", rel_path, e))?;
        if metadata.len() as usize > MAX_READ_BYTES {
            return Err(format!(
                "File '{}' exceeds maximum allowed size ({} bytes > {} max)",
                rel_path,
                metadata.len(),
                MAX_READ_BYTES
            ));
        }
        let content = fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read '{}': {}", rel_path, e))?;

        if offset.is_none() && limit.is_none() {
            return Ok(content);
        }

        let start_line = offset.unwrap_or(1).max(1);
        let max_lines = limit.unwrap_or(usize::MAX);
        let selected_lines: Vec<&str> = content
            .lines()
            .skip(start_line - 1)
            .take(max_lines)
            .collect();

        Ok(selected_lines.join("\n"))
    }

    pub fn write_file(&self, rel_path: &str, content: &str) -> Result<usize, String> {
        let trimmed = rel_path.trim();
        if trimmed.is_empty() {
            return Err("File path cannot be empty".to_string());
        }
        let full_path = self.resolve_path(trimmed)?;
        let parent = full_path
            .parent()
            .ok_or_else(|| "Target path must have a parent directory".to_string())?;

        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create directories for '{}': {}",
                    parent.display(),
                    e
                )
            })?;
        }

        let canonical_parent = parent.canonicalize().map_err(|e| {
            format!(
                "Failed to canonicalize parent '{}': {}",
                parent.display(),
                e
            )
        })?;
        if !canonical_parent.starts_with(&self.root) {
            return Err("Security error: Destination parent escapes project root".to_string());
        }

        if full_path.exists() {
            let meta = fs::symlink_metadata(&full_path).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err("Security error: Writing to a symlink is forbidden".to_string());
            }
        }

        fs::write(&full_path, content.as_bytes())
            .map_err(|e| format!("Failed to write to '{}': {}", rel_path, e))?;
        Ok(content.len())
    }

    pub fn list_dir(&self, rel_path: Option<&str>) -> Result<Vec<FileEntry>, String> {
        let target_dir = match rel_path {
            Some(path) => self.resolve_path(path)?,
            None => self.root.clone(),
        };

        if !target_dir.is_dir() {
            return Err(format!("'{}' is not a directory", target_dir.display()));
        }

        let canonical_dir = target_dir.canonicalize().map_err(|e| e.to_string())?;
        if !canonical_dir.starts_with(&self.root) {
            return Err("Security error: Directory escapes project root".to_string());
        }
        let entries = fs::read_dir(&canonical_dir)
            .map_err(|e| format!("Failed to list directory '{}': {}", target_dir.display(), e))?;

        let mut results = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" || name == "node_modules" || name == "target" {
                continue;
            }
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            let Ok(canonical_path) = path.canonicalize() else {
                continue;
            };
            if !canonical_path.starts_with(&self.root) {
                continue;
            }
            let is_dir = metadata.is_dir();
            let size = if is_dir {
                0
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };
            let rel_entry_path = path
                .strip_prefix(&self.root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());

            results.push(FileEntry {
                name,
                path: rel_entry_path,
                is_dir,
                size,
            });

            if results.len() >= MAX_DIR_ENTRIES {
                break;
            }
        }

        results.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(results)
    }

    pub fn grep(&self, query: &str, pattern: Option<&str>) -> Result<Vec<GrepMatch>, String> {
        if query.trim().is_empty() {
            return Err("Search query cannot be empty".to_string());
        }
        let mut matches = Vec::new();
        let query_lower = query.to_lowercase();
        let pattern_ext = pattern.map(|p| p.trim_start_matches('*').to_lowercase());

        fn visit_dir(
            dir: &Path,
            root: &Path,
            query_lower: &str,
            pattern_ext: Option<&str>,
            matches: &mut Vec<GrepMatch>,
        ) {
            if matches.len() >= MAX_GREP_MATCHES {
                return;
            }
            let entries = match fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    continue;
                }
                if path.is_dir() {
                    visit_dir(&path, root, query_lower, pattern_ext, matches);
                } else if path.is_file() {
                    if let Some(ext) = pattern_ext
                        && !name.to_lowercase().ends_with(ext)
                    {
                        continue;
                    }
                    if let Ok(metadata) = entry.metadata()
                        && metadata.len() > 1_000_000
                    {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(&path) {
                        let rel_file = path
                            .strip_prefix(root)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| name.clone());
                        for (idx, line) in content.lines().enumerate() {
                            if line.to_lowercase().contains(query_lower) {
                                matches.push(GrepMatch {
                                    file: rel_file.clone(),
                                    line_number: idx + 1,
                                    line_text: line.chars().take(200).collect(),
                                });
                                if matches.len() >= MAX_GREP_MATCHES {
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }

        visit_dir(
            &self.root,
            &self.root,
            &query_lower,
            pattern_ext.as_deref(),
            &mut matches,
        );
        Ok(matches)
    }
}

pub struct ExecutionEngine;

impl ExecutionEngine {
    pub fn run_command(
        root: &Path,
        command_str: &str,
        timeout_secs: Option<u64>,
    ) -> Result<CommandResult, String> {
        let trimmed = command_str.trim();
        if trimmed.is_empty() {
            return Err("Command cannot be empty".to_string());
        }

        let timeout = Duration::from_secs(timeout_secs.unwrap_or(30).clamp(1, 120));
        let start = Instant::now();

        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = Command::new("cmd.exe");
            c.arg("/C").arg(trimmed);
            c
        };

        #[cfg(not(target_os = "windows"))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.arg("-c").arg(trimmed);
            c
        };

        cmd.current_dir(root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to execute command '{}': {}", trimmed, e))?;

        let start_time = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let duration_ms = start.elapsed().as_millis();
                    let output = child
                        .wait_with_output()
                        .map_err(|e| format!("Failed to read output: {}", e))?;
                    let stdout_str = String::from_utf8_lossy(&output.stdout);
                    let stderr_str = String::from_utf8_lossy(&output.stderr);

                    let stdout = stdout_str.chars().take(MAX_OUTPUT_BYTES).collect();
                    let stderr = stderr_str.chars().take(MAX_OUTPUT_BYTES).collect();

                    return Ok(CommandResult {
                        exit_code: status.code(),
                        stdout,
                        stderr,
                        duration_ms,
                    });
                }
                Ok(None) => {
                    if start_time.elapsed() >= timeout {
                        let _ = child.kill();
                        return Err(format!(
                            "Command timed out after {} seconds",
                            timeout.as_secs()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(format!("Error waiting for process: {}", e)),
            }
        }
    }
}

pub struct GitContext;

impl GitContext {
    pub fn status(root: &Path) -> Result<String, String> {
        let output = Command::new("git")
            .arg("status")
            .arg("--short")
            .arg("--branch")
            .current_dir(root)
            .output()
            .map_err(|e| format!("Failed to run git status: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Git error: {}", err.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn diff(root: &Path, staged: bool) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.arg("diff");
        if staged {
            cmd.arg("--staged");
        }
        let output = cmd
            .current_dir(root)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Git error: {}", err.trim()));
        }
        let diff_text = String::from_utf8_lossy(&output.stdout);
        Ok(diff_text.chars().take(MAX_OUTPUT_BYTES).collect())
    }

    pub fn log(root: &Path, limit: Option<usize>) -> Result<String, String> {
        let count = limit.unwrap_or(10).clamp(1, 50);
        let output = Command::new("git")
            .arg("log")
            .arg(format!("-n{}", count))
            .arg("--oneline")
            .current_dir(root)
            .output()
            .map_err(|e| format!("Failed to run git log: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Git error: {}", err.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn branch(root: &Path) -> Result<String, String> {
        let output = Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .current_dir(root)
            .output()
            .map_err(|e| format!("Failed to run git branch: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Git error: {}", err.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

pub struct ToolDispatcher;

impl ToolDispatcher {
    pub fn list_tools() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read content from a file inside the project workspace".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path" },
                        "offset": { "type": "integer", "description": "Start line number (1-indexed)" },
                        "limit": { "type": "integer", "description": "Max line count to read" }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write or overwrite content into a file inside the project workspace. Requires submission of the full content string.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path" },
                        "content": { "type": "string", "description": "File content" }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "list_dir".to_string(),

                description: "List directory contents in the workspace".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative directory path (empty for root)" }
                    }
                }),
            },
            ToolDefinition {
                name: "grep_search".to_string(),
                description: "Search file contents across workspace files".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Text substring to search for" },
                        "pattern": { "type": "string", "description": "File extension or pattern, e.g. '*.rs'" }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "run_command".to_string(),
                description: "Execute a terminal shell command scoped to the project directory".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "Command line to run" },
                        "timeout_secs": { "type": "integer", "description": "Optional timeout in seconds" }
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "git_status".to_string(),
                description: "Get repository branch and working tree status".to_string(),
                parameters: serde_json::json!({ "type": "object", "properties": {} }),
            },
            ToolDefinition {
                name: "git_diff".to_string(),
                description: "Inspect unstaged or staged git changes".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "staged": { "type": "boolean", "description": "When true, show staged changes" }
                    }
                }),
            },
            ToolDefinition {
                name: "git_log".to_string(),
                description: "View recent commit history".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "Number of commits to show" }
                    }
                }),
            },
            ToolDefinition {
                name: "git_branch".to_string(),
                description: "Get the current git branch name".to_string(),
                parameters: serde_json::json!({ "type": "object", "properties": {} }),
            },
        ]
    }

    pub fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
        let mut calls = Vec::new();
        let mut cursor = 0;
        let tag_open = "<tool_call>";
        let tag_close = "</tool_call>";

        while let Some(start_idx) = text[cursor..].find(tag_open) {
            let actual_start = cursor + start_idx + tag_open.len();
            if let Some(end_idx) = text[actual_start..].find(tag_close) {
                let actual_end = actual_start + end_idx;
                let payload = text[actual_start..actual_end].trim();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(payload)
                    && let Some(name) = val["name"].as_str()
                {
                    let id = val["id"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("call_{}", calls.len() + 1));
                    let arguments = val
                        .get("arguments")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    calls.push(ToolCall {
                        id,
                        name: name.to_string(),
                        arguments,
                    });
                }
                cursor = actual_end + tag_close.len();
            } else {
                break;
            }
        }
        calls
    }

    pub fn execute_tool(root: &Path, call: &ToolCall, approved: bool) -> ToolResult {
        if (call.name == "write_file" || call.name == "run_command") && !approved {
            return ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                success: false,
                output: String::new(),
                error: Some("Operation requires explicit user approval".to_string()),
            };
        }

        let sandbox = match Sandbox::new(root.to_path_buf()) {
            Ok(s) => s,
            Err(e) => {
                return ToolResult {
                    tool_call_id: call.id.clone(),
                    name: call.name.clone(),
                    success: false,
                    output: String::new(),
                    error: Some(e),
                };
            }
        };

        match call.name.as_str() {
            "read_file" => {
                let path = call.arguments["path"].as_str().unwrap_or("");
                let offset = call.arguments["offset"].as_u64().map(|v| v as usize);
                let limit = call.arguments["limit"].as_u64().map(|v| v as usize);
                match sandbox.read_file(path, offset, limit) {
                    Ok(out) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: true,
                        output: out,
                        error: None,
                    },
                    Err(e) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    },
                }
            }
            "write_file" => {
                let path = call.arguments["path"].as_str().unwrap_or("");
                let content = call.arguments["content"].as_str().unwrap_or("");
                match sandbox.write_file(path, content) {
                    Ok(bytes) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: true,
                        output: format!("Successfully wrote {} bytes to '{}'", bytes, path),
                        error: None,
                    },
                    Err(e) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    },
                }
            }
            "list_dir" => {
                let path = call.arguments["path"].as_str();
                match sandbox.list_dir(path) {
                    Ok(entries) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: true,
                        output: serde_json::to_string_pretty(&entries).unwrap_or_default(),
                        error: None,
                    },
                    Err(e) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    },
                }
            }
            "grep_search" => {
                let query = call.arguments["query"].as_str().unwrap_or("");
                let pattern = call.arguments["pattern"].as_str();
                match sandbox.grep(query, pattern) {
                    Ok(matches) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: true,
                        output: serde_json::to_string_pretty(&matches).unwrap_or_default(),
                        error: None,
                    },
                    Err(e) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    },
                }
            }
            "run_command" => {
                let command = call.arguments["command"].as_str().unwrap_or("");
                let timeout = call.arguments["timeout_secs"].as_u64();
                match ExecutionEngine::run_command(root, command, timeout) {
                    Ok(res) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: res.exit_code == Some(0),
                        output: if !res.stdout.is_empty() {
                            res.stdout
                        } else {
                            res.stderr.clone()
                        },
                        error: if res.exit_code != Some(0) {
                            Some(format!(
                                "Exit code {:?}. Stderr: {}",
                                res.exit_code, res.stderr
                            ))
                        } else {
                            None
                        },
                    },
                    Err(e) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    },
                }
            }
            "git_status" => match GitContext::status(root) {
                Ok(status) => ToolResult {
                    tool_call_id: call.id.clone(),
                    name: call.name.clone(),
                    success: true,
                    output: status,
                    error: None,
                },
                Err(e) => ToolResult {
                    tool_call_id: call.id.clone(),
                    name: call.name.clone(),
                    success: false,
                    output: String::new(),
                    error: Some(e),
                },
            },
            "git_diff" => {
                let staged = call.arguments["staged"].as_bool().unwrap_or(false);
                match GitContext::diff(root, staged) {
                    Ok(diff) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: true,
                        output: if diff.is_empty() {
                            "No diff changes".to_string()
                        } else {
                            diff
                        },
                        error: None,
                    },
                    Err(e) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    },
                }
            }
            "git_log" => {
                let limit = call.arguments["limit"].as_u64().map(|v| v as usize);
                match GitContext::log(root, limit) {
                    Ok(log) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: true,
                        output: log,
                        error: None,
                    },
                    Err(e) => ToolResult {
                        tool_call_id: call.id.clone(),
                        name: call.name.clone(),
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    },
                }
            }
            "git_branch" => match GitContext::branch(root) {
                Ok(branch) => ToolResult {
                    tool_call_id: call.id.clone(),
                    name: call.name.clone(),
                    success: true,
                    output: branch,
                    error: None,
                },
                Err(e) => ToolResult {
                    tool_call_id: call.id.clone(),
                    name: call.name.clone(),
                    success: false,
                    output: String::new(),
                    error: Some(e),
                },
            },
            other => ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                success: false,
                output: String::new(),
                error: Some(format!("Unknown tool: {}", other)),
            },
        }
    }

    pub fn format_observation(result: &ToolResult) -> String {
        if result.success {
            format!("[Tool Observation - {}]:\n{}", result.name, result.output)
        } else {
            format!(
                "[Tool Error - {}]:\n{}",
                result.name,
                result.error.as_deref().unwrap_or("Failed")
            )
        }
    }

    pub fn build_system_prompt() -> String {
        let tools = Self::list_tools();
        let tools_json = serde_json::to_string_pretty(&tools).unwrap_or_default();
        format!(
            "You are OpenHarness Coding Assistant, an expert autonomous software engineer.\n\
            You have access to the user's project workspace through the following tools:\n\
            {}\n\n\
            To call a tool, output a block formatted exactly as:\n\
            <tool_call>\n\
            {{\"name\": \"tool_name\", \"arguments\": {{ ... }}}}\n\
            </tool_call>\n\n\
            You can read files, write edits, search the codebase, inspect git status/diff, and run terminal commands to test your work.\n\
            After receiving a tool observation, continue your analysis or execution until the task is complete.",
            tools_json
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_security_traversal_forbidden() {
        let unique = format!(
            "oh_sec_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let temp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_tmp")
            .join(unique);
        fs::create_dir_all(&temp_dir).unwrap();
        let sandbox = Sandbox::new(temp_dir.clone()).unwrap();

        assert!(sandbox.resolve_path("../some_file").is_err());
        assert!(sandbox.resolve_path("../../etc/passwd").is_err());
        assert!(sandbox.resolve_path("sub/../../escape").is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sandbox_file_lifecycle() {
        let unique = format!(
            "oh_life_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let temp_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test_tmp")
            .join(unique);
        fs::create_dir_all(&temp_dir).unwrap();
        let sandbox = Sandbox::new(temp_dir.clone()).unwrap();

        let bytes = sandbox
            .write_file("nested/test.txt", "hello world\nline two")
            .unwrap();
        assert_eq!(bytes, 20);

        let read = sandbox.read_file("nested/test.txt", None, None).unwrap();
        assert_eq!(read, "hello world\nline two");

        let slice = sandbox
            .read_file("nested/test.txt", Some(2), Some(1))
            .unwrap();
        assert_eq!(slice, "line two");

        let entries = sandbox.list_dir(Some("nested")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "test.txt");
        assert!(!entries[0].is_dir);

        let grep_res = sandbox.grep("world", None).unwrap();
        assert_eq!(grep_res.len(), 1);
        assert_eq!(grep_res[0].line_number, 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_tool_call_parsing() {
        let message = "Let me check the files first.\n\
            <tool_call>\n\
            {\"name\": \"list_dir\", \"arguments\": {\"path\": \"\"}}\n\
            </tool_call>\n\
            And read readme:\n\
            <tool_call>\n\
            {\"name\": \"read_file\", \"arguments\": {\"path\": \"README.md\"}}\n\
            </tool_call>";

        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "list_dir");
        assert_eq!(calls[1].name, "read_file");
        assert_eq!(calls[1].arguments["path"], "README.md");
    }

    #[test]
    fn test_execution_engine_echo() {
        let temp_dir = std::env::temp_dir();
        #[cfg(target_os = "windows")]
        let res = ExecutionEngine::run_command(&temp_dir, "echo harness_ok", Some(5)).unwrap();
        #[cfg(not(target_os = "windows"))]
        let res = ExecutionEngine::run_command(&temp_dir, "echo harness_ok", Some(5)).unwrap();

        assert_eq!(res.exit_code, Some(0));
        assert!(res.stdout.contains("harness_ok"));
    }
}

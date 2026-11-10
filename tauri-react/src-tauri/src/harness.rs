use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

        let path_obj = Path::new(trimmed);
        let normalized = if let Ok(stripped) = path_obj.strip_prefix(&self.root) {
            stripped
        } else if let Some(root_name) = self.root.file_name() {
            if let Ok(stripped) = path_obj.strip_prefix(root_name) {
                stripped
            } else {
                path_obj
            }
        } else {
            path_obj
        };

        let candidate = self.root.join(normalized);
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

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

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
        let mut output_cmd = Command::new("git");
        #[cfg(target_os = "windows")]
        output_cmd.creation_flags(CREATE_NO_WINDOW);
        let output = output_cmd
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
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
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
        let mut cmd = Command::new("git");
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd
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
        let mut cmd = Command::new("git");
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd
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
    pub fn native_tool_call(value: &serde_json::Value, next_id: usize) -> Option<ToolCall> {
        let function = value.get("function").unwrap_or(value);
        let raw_name = function
            .get("name")
            .or_else(|| function.get("tool"))
            .or_else(|| function.get("action"))
            .and_then(|v| v.as_str())?;

        let canonical_name = match raw_name.to_lowercase().replace(['-', ' '], "_").as_str() {
            "read_file" | "readfile" | "read" | "cat" | "view_file" => "read_file",
            "write_file" | "writefile" | "write" | "edit_file" | "create_file" => "write_file",
            "list_dir" | "listdir" | "ls" | "dir" | "list_directory" => "list_dir",
            "grep_search" | "grep" | "search" | "grepsearch" => "grep_search",
            "run_command" | "runcommand" | "bash" | "sh" | "terminal" | "exec" | "command" | "cmd" => {
                "run_command"
            }
            "git_status" | "gitstatus" => "git_status",
            "git_diff" | "gitdiff" => "git_diff",
            "git_log" | "gitlog" => "git_log",
            "git_branch" | "gitbranch" => "git_branch",
            _ => return None,
        };

        let mut arguments = if let Some(args_val) = function
            .get("arguments")
            .or_else(|| function.get("parameters"))
            .or_else(|| function.get("params"))
            .or_else(|| function.get("args"))
            .or_else(|| function.get("input"))
        {
            match args_val {
                serde_json::Value::String(raw) => {
                    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({}))
                }
                other => other.clone(),
            }
        } else {
            let mut top = serde_json::Map::new();
            if let Some(obj) = function.as_object() {
                for (k, v) in obj {
                    if !["name", "tool", "action", "id", "type", "function"].contains(&k.as_str()) {
                        top.insert(k.clone(), v.clone());
                    }
                }
            }
            serde_json::Value::Object(top)
        };

        if let Some(obj) = arguments.as_object_mut() {
            match canonical_name {
                "read_file" | "write_file" => {
                    if !obj.contains_key("path") {
                        for alias in ["file_path", "filePath", "file", "filename", "rel_path", "target", "uri"] {
                            if let Some(val) = obj.get(alias).cloned() {
                                obj.insert("path".to_string(), val);
                                break;
                            }
                        }
                    }
                    if let Some(p) = obj.get("path").and_then(|v| v.as_str()) {
                        let cleaned = p
                            .trim()
                            .trim_matches(['"', '\''])
                            .trim_start_matches("./")
                            .trim_start_matches(".\\");
                        obj.insert("path".to_string(), serde_json::json!(cleaned));
                    }
                    if canonical_name == "write_file" && !obj.contains_key("content") {
                        for alias in ["text", "data", "body", "code"] {
                            if let Some(val) = obj.get(alias).cloned() {
                                obj.insert("content".to_string(), val);
                                break;
                            }
                        }
                    }
                }
                "list_dir" => {
                    if !obj.contains_key("path") {
                        for alias in ["dir", "directory", "folder"] {
                            if let Some(val) = obj.get(alias).cloned() {
                                obj.insert("path".to_string(), val);
                                break;
                            }
                        }
                    }
                    if !obj.contains_key("path") {
                        obj.insert("path".to_string(), serde_json::json!(""));
                    }
                }
                "grep_search" => {
                    if !obj.contains_key("query") {
                        for alias in ["pattern", "search", "q", "regex", "text", "term"] {
                            if let Some(val) = obj.get(alias).cloned() {
                                obj.insert("query".to_string(), val);
                                break;
                            }
                        }
                    }
                }
                "run_command" => {
                    for alias in ["cmd", "script", "exec"] {
                        if !obj.contains_key("command")
                            && let Some(val) = obj.get(alias).cloned()
                        {
                            obj.insert("command".to_string(), val);
                            break;
                        }
                    }
                }
                _ => {}
            }
        }

        Some(ToolCall {
            id: value
                .get("id")
                .and_then(|id| id.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("call_{}", next_id)),
            name: canonical_name.to_string(),
            arguments,
        })
    }

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
        Self::parse_tool_calls_with_id(text, None)
    }

    pub fn parse_tool_calls_with_id(text: &str, native_id: Option<&str>) -> Vec<ToolCall> {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
            && let Some(items) = value["tool_calls"].as_array()
        {
            let mut calls = Vec::new();
            for (index, item) in items.iter().enumerate() {
                if let Some(call) = Self::native_tool_call(item, index + 1) {
                    calls.push(call);
                }
            }
            if !calls.is_empty() {
                return calls;
            }
        }
        if let Some(id) = native_id
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
            && let Some(mut call) = Self::native_tool_call(&value, 1)
        {
            call.id = id.to_string();
            return vec![call];
        }
        let mut calls = Vec::new();
        let tag_pairs = [
            ("<tool_call>", "</tool_call>"),
            ("<toolcall>", "</toolcall>"),
            ("<tool_calls>", "</tool_calls>"),
            ("<toolcalls>", "</toolcalls>"),
            ("<tool-call>", "</tool-call>"),
            ("<tool-calls>", "</tool-calls>"),
        ];

        for (open_tag, close_tag) in tag_pairs {
            let mut cur = 0;
            while let Some(start_idx) = text[cur..].to_lowercase().find(open_tag) {
                let actual_start = cur + start_idx + open_tag.len();
                if let Some(end_idx) = text[actual_start..].to_lowercase().find(close_tag) {
                    let actual_end = actual_start + end_idx;
                    let payload = text[actual_start..actual_end].trim();
                    let clean_payload = if let Some(stripped) = payload.strip_prefix("```") {
                        let c = stripped.find('\n').map(|nl| &stripped[nl + 1..]).unwrap_or(stripped);
                        c.rfind("```").map(|end| &c[..end]).unwrap_or(c).trim()
                    } else {
                        payload
                    };
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(clean_payload) {
                        if let Some(arr) = val.as_array() {
                            for item in arr {
                                if let Some(call) = Self::native_tool_call(item, calls.len() + 1) {
                                    calls.push(call);
                                }
                            }
                        } else if let Some(call) = Self::native_tool_call(&val, calls.len() + 1) {
                            calls.push(call);
                        }
                    }
                    cur = actual_end + close_tag.len();
                } else {
                    break;
                }
            }
        }

        if calls.is_empty() {
            let known_xml_tools = [
                ("read_file", "path"),
                ("write_file", "path"),
                ("list_dir", "path"),
                ("grep_search", "query"),
                ("grep", "query"),
                ("run_command", "command"),
                ("git_status", ""),
                ("git_diff", "staged"),
                ("git_log", "limit"),
                ("git_branch", ""),
            ];
            for (tool_name, default_arg) in known_xml_tools {
                let open_prefix = format!("<{}", tool_name);
                let close_tag = format!("</{}>", tool_name);
                let mut cur = 0;
                while let Some(idx) = text[cur..].to_lowercase().find(&open_prefix) {
                    let start = cur + idx;
                    let after_tag = start + open_prefix.len();
                    if after_tag < text.len() {
                        let ch = text.as_bytes()[after_tag];
                        if ch != b' ' && ch != b'>' && ch != b'/' && ch != b'\n' && ch != b'\r' {
                            cur = after_tag;
                            continue;
                        }
                    }

                    if let Some(close_bracket) = text[start..].find('>') {
                        let header = &text[start..start + close_bracket];
                        if header.ends_with('/') {
                            let mut args_map = serde_json::Map::new();
                            for part in header.split_whitespace() {
                                if let Some((k, v)) = part.split_once('=') {
                                    let k = k.trim().to_lowercase();
                                    let v = v.trim().trim_matches(['"', '\'']);
                                    args_map.insert(k, serde_json::json!(v));
                                }
                            }
                            if let Some(call) = Self::native_tool_call(
                                &serde_json::json!({
                                    "name": tool_name,
                                    "arguments": serde_json::Value::Object(args_map)
                                }),
                                calls.len() + 1,
                            ) {
                                calls.push(call);
                            }
                            cur = start + close_bracket + 1;
                            continue;
                        }
                    }

                    if let Some(open_bracket) = text[start..].find('>') {
                        let content_start = start + open_bracket + 1;
                        if let Some(end_idx) = text[content_start..].to_lowercase().find(&close_tag) {
                            let content_end = content_start + end_idx;
                            let inner = text[content_start..content_end].trim();
                            let clean_inner = if let Some(stripped) = inner.strip_prefix("```") {
                                let c = stripped.find('\n').map(|nl| &stripped[nl + 1..]).unwrap_or(stripped);
                                c.rfind("```").map(|end| &c[..end]).unwrap_or(c).trim()
                            } else {
                                inner
                            };
                            let call = if let Ok(val) = serde_json::from_str::<serde_json::Value>(clean_inner) {
                                Self::native_tool_call(&serde_json::json!({
                                    "name": tool_name,
                                    "arguments": val
                                }), calls.len() + 1)
                            } else if !default_arg.is_empty() && !clean_inner.is_empty() {
                                let mut map = serde_json::Map::new();
                                map.insert(default_arg.to_string(), serde_json::json!(clean_inner));
                                Self::native_tool_call(&serde_json::json!({
                                    "name": tool_name,
                                    "arguments": serde_json::Value::Object(map)
                                }), calls.len() + 1)
                            } else {
                                Self::native_tool_call(&serde_json::json!({
                                    "name": tool_name,
                                    "arguments": {}
                                }), calls.len() + 1)
                            };
                            if let Some(c) = call {
                                calls.push(c);
                            }
                            cur = content_end + close_tag.len();
                            continue;
                        }
                    }

                    cur = start + open_prefix.len();
                }
            }
        }

        if !calls.is_empty() {
            return calls;
        }

        fn extract_call(val: &serde_json::Value, next_id: usize) -> Option<ToolCall> {
            if let Some(items) = val["tool_calls"].as_array() {
                return items
                    .iter()
                    .enumerate()
                    .filter_map(|(i, item)| ToolDispatcher::native_tool_call(item, next_id + i))
                    .next();
            }
            if val["function"].is_object() {
                return ToolDispatcher::native_tool_call(val, next_id);
            }
            let name = val["name"].as_str().or_else(|| val["tool"].as_str())?;
            let arguments = val.get("arguments").cloned().unwrap_or_else(|| val.clone());
            let call = ToolDispatcher::native_tool_call(
                &serde_json::json!({"id": val["id"], "name": name, "arguments": arguments}),
                next_id,
            )?;
            Some(call)
        }

        // Check for markdown code blocks containing tool call JSON
        let mut block_cur = 0;
        while let Some(start) = text[block_cur..].find("```") {
            let block_start = block_cur + start + 3;
            let content_start = text[block_start..]
                .find('\n')
                .map(|nl| block_start + nl + 1)
                .unwrap_or(block_start);
            if let Some(end) = text[content_start..].find("```") {
                let block_end = content_start + end;
                let snippet = text[content_start..block_end].trim();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(snippet) {
                    if let Some(call) = extract_call(&val, calls.len() + 1) {
                        calls.push(call);
                    } else if let Some(arr) = val.as_array() {
                        for item in arr {
                            if let Some(call) = extract_call(item, calls.len() + 1) {
                                calls.push(call);
                            }
                        }
                    }
                }
                block_cur = block_end + 3;
            } else {
                break;
            }
        }

        if !calls.is_empty() {
            return calls;
        }

        // Fallback: look for balanced JSON objects in text
        let mut start_pos = 0;
        while let Some(open_idx) = text[start_pos..].find('{') {
            let abs_open = start_pos + open_idx;
            let mut depth = 0;
            let mut in_str = false;
            let mut escaped = false;
            let mut found_close = None;
            for (i, c) in text[abs_open..].char_indices() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if c == '\\' {
                    escaped = true;
                    continue;
                }
                if c == '"' {
                    in_str = !in_str;
                    continue;
                }
                if in_str {
                    continue;
                }
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        found_close = Some(abs_open + i);
                        break;
                    }
                }
            }
            if let Some(close_idx) = found_close {
                let candidate = &text[abs_open..=close_idx];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate)
                    && let Some(call) = extract_call(&val, calls.len() + 1)
                {
                    calls.push(call);
                    start_pos = close_idx + 1;
                    continue;
                }
                start_pos = abs_open + 1;
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
            "grep_search" | "grep" => {
                let query = call.arguments["query"]
                    .as_str()
                    .or_else(|| call.arguments["pattern"].as_str())
                    .unwrap_or("");
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

    pub fn strip_tool_calls(text: &str) -> String {
        let normalized: String = text
            .chars()
            .filter(|c| !['\u{200b}', '\u{200c}', '\u{200d}', '\u{feff}'].contains(c))
            .collect();
        let text = &normalized;
        let mut clean = String::new();
        let lower = text.to_lowercase();
        let tags = [
            ("<tool_call>", "</tool_call>"),
            ("<toolcall>", "</toolcall>"),
            ("<tool_calls>", "</tool_calls>"),
            ("<toolcalls>", "</toolcalls>"),
        ];

        let mut pos = 0;
        while pos < text.len() {
            let mut earliest_match = None;
            for (open_tag, close_tag) in &tags {
                if let Some(open_idx) = lower[pos..].find(open_tag) {
                    let actual_open = pos + open_idx;
                    match earliest_match {
                        None => earliest_match = Some((actual_open, open_tag.len(), close_tag)),
                        Some((earliest_pos, _, _)) if actual_open < earliest_pos => {
                            earliest_match = Some((actual_open, open_tag.len(), close_tag));
                        }
                        _ => {}
                    }
                }
            }

            if let Some((open_pos, open_len, close_tag)) = earliest_match {
                clean.push_str(&text[pos..open_pos]);
                let after_open = open_pos + open_len;
                if let Some(close_idx) = lower[after_open..].find(close_tag) {
                    pos = after_open + close_idx + close_tag.len();
                } else {
                    pos = text.len();
                }
            } else {
                clean.push_str(&text[pos..]);
                break;
            }
        }

        // Also strip any residual code blocks that were parsed as tool calls
        let mut final_clean = String::new();
        let mut b_pos = 0;
        while let Some(start) = clean[b_pos..].find("```") {
            let block_start = b_pos + start;
            let after_start = block_start + 3;
            if let Some(end) = clean[after_start..].find("```") {
                let block_end = after_start + end + 3;
                let block_content = &clean[block_start..block_end];
                if !Self::parse_tool_calls(block_content).is_empty() {
                    final_clean.push_str(&clean[b_pos..block_start]);
                    b_pos = block_end;
                    continue;
                }
            }
            final_clean.push_str(&clean[b_pos..after_start]);
            b_pos = after_start;
        }
        final_clean.push_str(&clean[b_pos..]);

        let trimmed = final_clean.trim();
        if trimmed.starts_with('{')
            && trimmed.ends_with('}')
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed)
            && (val.get("name").is_some() || val.get("tool").is_some())
        {
            return String::new();
        }

        final_clean.trim().to_string()
    }

    pub fn build_system_prompt() -> String {
        let tools = Self::list_tools();
        let tools_json = serde_json::to_string_pretty(&tools).unwrap_or_default();
        format!(
            "You are OpenHarness Coding Assistant, an expert autonomous software engineer with full tool access to the user's project workspace.\n\
            You have access to the following tools:\n\
            {}\n\n\
            IMPORTANT INSTRUCTIONS:\n\
            1. You ALWAYS have tool access active and available on every turn. Never claim that tool access is unavailable.\n\
            2. When the user asks you to understand, read, explore, inspect, or modify the project or any files/directories, proactively call the appropriate tools (such as list_dir, read_file, grep_search, git_status) to inspect the actual files first.\n\
            3. To call a tool, output a block formatted as:\n\
            <tool_call>\n\
            {{\"name\": \"tool_name\", \"arguments\": {{ ... }}}}\n\
            </tool_call>\n\
            For example, to read a file:\n\
            <tool_call>\n\
            {{\"name\": \"read_file\", \"arguments\": {{\"path\": \"README.md\"}}}}\n\
            </tool_call>\n\
            4. When calling a tool, do NOT add speculative explanations or final answers in the same message. Output the tool call block and wait for the tool observation before explaining.\n\
            5. After receiving a tool observation, analyze the returned data and continue with additional tool calls if needed, or provide your final response.",
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
    fn test_tool_call_parsing_raw_json() {
        let message = r#"{"name":"read_file","arguments":{"path":"APP-COMPLETION-AUDIT.md"}}"#;
        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "APP-COMPLETION-AUDIT.md");
    }

    #[test]
    fn test_tool_call_parsing_markdown_code_block() {
        let message = "```json\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"test.rs\"}}\n```";
        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "test.rs");
    }

    #[test]
    fn test_tool_call_parsing_raw_json_with_prose() {
        let message = "Sure, reading the file:\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"src/main.rs\"}}";
        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "src/main.rs");
    }

    #[test]
    fn test_tool_call_parsing_parameter_aliases() {
        let message = "<tool_call>{\"name\": \"read_file\", \"parameters\": {\"file_path\": \"./src/lib.rs\"}}</tool_call>";
        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "src/lib.rs");
    }

    #[test]
    fn test_tool_call_parsing_top_level_arguments() {
        let message = "{\"tool\": \"read_file\", \"path\": \"Cargo.toml\"}";
        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "Cargo.toml");
    }

    #[test]
    fn test_tool_call_parsing_direct_xml() {
        let message = "Let me read that:\n<read_file path=\"Cargo.toml\" />";
        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "Cargo.toml");

        let message_block = "<read_file>src/main.rs</read_file>";
        let calls_block = ToolDispatcher::parse_tool_calls(message_block);
        assert_eq!(calls_block.len(), 1);
        assert_eq!(calls_block[0].name, "read_file");
        assert_eq!(calls_block[0].arguments["path"], "src/main.rs");
    }

    #[test]
    fn test_tool_call_parsing_grep_alias() {
        let message = "<tool_call>{\"name\": \"grep\", \"arguments\": {\"pattern\": \"AppState\"}}</tool_call>";
        let calls = ToolDispatcher::parse_tool_calls(message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "grep_search");
        assert_eq!(calls[0].arguments["query"], "AppState");
    }

    #[test]
    fn test_strip_tool_calls_and_preserve_prose() {
        let message = "Before <tool_call>{\"name\":\"read_file\"}</tool_call> After";
        assert_eq!(ToolDispatcher::strip_tool_calls(message), "Before  After");
    }

    #[test]
    fn test_strip_zero_width_tool_tags() {
        let message = "<\u{200b}tool_call>{}</\u{200b}tool_call>";
        assert!(ToolDispatcher::strip_tool_calls(message).is_empty());
    }

    #[test]
    fn test_strip_keeps_regular_json_code() {
        let message = "```json\\n{\\\"ok\\\": true}\\n```";
        assert_eq!(ToolDispatcher::strip_tool_calls(message), message);
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

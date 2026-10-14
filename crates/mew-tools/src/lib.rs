use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use mew_guard::WorkspaceGuard;
use mew_provider::ToolSpec;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, OnceLock,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

pub mod hooks;
pub use hooks::{run_hook_event, HookEvent};

#[derive(Debug, Clone, Copy, Default)]
pub struct ToolPermissions {
    pub allow_write: bool,
    pub allow_exec: bool,
    pub allow_network: bool,
}

pub fn tool_definitions(
    include_edits: bool,
    include_exec: bool,
    include_network: bool,
) -> Vec<ToolSpec> {
    tool_definitions_with_shell(include_edits, include_exec, include_network, false)
}

pub fn tool_definitions_with_shell(
    include_edits: bool,
    include_exec: bool,
    include_network: bool,
    include_shell: bool,
) -> Vec<ToolSpec> {
    let mut tools = vec![
        spec(
            "list_files",
            "List files under a workspace-relative directory (bounded output).",
            json!({"type":"object","properties":{"path":{"type":"string","description":"Relative directory; defaults to ."}},"additionalProperties":false}),
        ),
        spec(
            "read_file",
            "Read a UTF-8 text file within the workspace (max 1 MB).",
            json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}),
        ),
        spec(
            "search_text",
            "Search text in workspace files, skipping hidden/build/dependency directories.",
            json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string","description":"Optional relative directory; defaults to ."}},"required":["query"],"additionalProperties":false}),
        ),
        spec(
            "git_status",
            "Show the current git branch and working-tree status.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        spec(
            "git_diff",
            "Show a bounded git diff, optionally restricted to one workspace-relative file.",
            json!({"type":"object","properties":{"path":{"type":"string"}},"additionalProperties":false}),
        ),
    ];
    if include_edits {
        tools.push(spec("replace_file", "Propose and apply one exact, unique text replacement after the user approves the displayed diff.", json!({"type":"object","properties":{"path":{"type":"string"},"old_text":{"type":"string"},"new_text":{"type":"string"}},"required":["path","old_text","new_text"],"additionalProperties":false})));
    }
    if include_exec {
        tools.push(spec("run_check", "Run only a supported project check (fmt, check, or test); never accepts arbitrary shell commands.", json!({"type":"object","properties":{"kind":{"type":"string","enum":["fmt","check","test"]}},"required":["kind"],"additionalProperties":false})));
    }
    if include_network {
        tools.push(spec("http_get", "Fetch a public HTTPS URL after explicit approval; private and local addresses are blocked.", json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"],"additionalProperties":false})));
    }
    if include_shell {
        tools.extend([
            spec("run_command", "Run a local shell command from the workspace root. Auto mode only; commands have a 60-second foreground timeout. Set background=true to keep a process running and receive its id.", json!({"type":"object","properties":{"command":{"type":"string","minLength":1,"maxLength":4000},"background":{"type":"boolean","default":false}},"required":["command"],"additionalProperties":false})),
            spec("background_list", "List Mew-managed background commands and their current states.", json!({"type":"object","properties":{},"additionalProperties":false})),
            spec("background_status", "Check the state of one Mew-managed background command.", json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false})),
            spec("background_output", "Read bounded combined stdout/stderr for a Mew-managed background command.", json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false})),
            spec("background_cancel", "Stop a Mew-managed background command that is still running.", json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false})),
        ]);
    }
    tools
}

fn spec(name: &str, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

pub fn is_write_tool(name: &str) -> bool {
    name == "replace_file"
}
pub fn is_exec_tool(name: &str) -> bool {
    matches!(name, "run_check" | "run_command" | "background_cancel")
}
pub fn is_network_tool(name: &str) -> bool {
    name == "http_get"
}

pub fn check_command(root: &Path, kind: &str) -> Result<String> {
    if root.join("Cargo.toml").exists() {
        return match kind {
            "fmt" => Ok("cargo fmt --all -- --check".into()),
            "check" => Ok("cargo check --workspace".into()),
            "test" => Ok("cargo test --workspace".into()),
            _ => Err(anyhow!("unsupported check kind")),
        };
    }
    if root.join("package.json").exists() {
        return match kind {
            "check" => Ok("npm run --if-present typecheck".into()),
            "test" => Ok("npm test -- --runInBand".into()),
            _ => Err(anyhow!("no safe default formatter check for this project")),
        };
    }
    if root.join("pyproject.toml").exists() {
        return match kind {
            "check" => Ok("python -m compileall -q .".into()),
            "test" => Ok("python -m pytest -q".into()),
            _ => Err(anyhow!("no safe default formatter check for this project")),
        };
    }
    if root.join("go.mod").exists() {
        return match kind {
            "fmt" => Ok("gofmt -d .".into()),
            "check" => Ok("go vet ./...".into()),
            "test" => Ok("go test ./...".into()),
            _ => Err(anyhow!("unsupported check kind")),
        };
    }
    Err(anyhow!("no supported project manifest found"))
}

pub fn edit_preview(root: &Path, args: &Value) -> Result<(String, String, String, String)> {
    let path = required_string(args, "path")?;
    let old = required_string(args, "old_text")?;
    let new = required_string(args, "new_text")?;
    if old.is_empty() {
        return Err(anyhow!("old_text must not be empty"));
    }
    let guard = WorkspaceGuard::new(root)?;
    let current = guard.read_text(path)?;
    let matches = current.match_indices(old).count();
    if matches != 1 {
        return Err(anyhow!(
            "expected old_text to occur exactly once, found {matches}"
        ));
    }
    let updated = current.replacen(old, new, 1);
    let diff = simple_diff(path, old, new);
    Ok((path.to_string(), current, updated, diff))
}

pub async fn execute_tool(
    root: &Path,
    name: &str,
    args: &Value,
    permissions: ToolPermissions,
) -> Result<String> {
    let guard = WorkspaceGuard::new(root)?;
    match name {
        "list_files" => {
            let rel = optional_string(args, "path").unwrap_or(".");
            let dir = if rel == "." {
                guard.root().to_path_buf()
            } else {
                guard.existing(rel)?
            };
            if !dir.is_dir() {
                return Err(anyhow!("path is not a directory"));
            }
            let mut out = Vec::new();
            list_files(&guard, &dir, 0, &mut out, 250)?;
            Ok(out.join("\n"))
        }
        "read_file" => guard.read_text(required_string(args, "path")?),
        "search_text" => {
            let query = required_string(args, "query")?;
            if query.is_empty() {
                return Err(anyhow!("query must not be empty"));
            }
            let rel = optional_string(args, "path").unwrap_or(".");
            let base = if rel == "." {
                guard.root().to_path_buf()
            } else {
                guard.existing(rel)?
            };
            let mut files = Vec::new();
            collect_files(&guard, &base, 0, &mut files, 2_000)?;
            let mut results = Vec::new();
            for file in files {
                if results.len() >= 100 {
                    break;
                }
                let Ok(meta) = fs::metadata(&file) else {
                    continue;
                };
                if meta.len() > 1_000_000 {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&file) else {
                    continue;
                };
                let rel = file.strip_prefix(guard.root()).unwrap_or(&file);
                for (i, line) in text.lines().enumerate() {
                    if line.contains(query) {
                        results.push(format!("{}:{}:{}", rel.display(), i + 1, line.trim()));
                        if results.len() >= 100 {
                            break;
                        }
                    }
                }
            }
            if results.is_empty() {
                Ok("No matches.".to_string())
            } else {
                Ok(results.join("\n"))
            }
        }
        "git_status" => git_read(&guard, &["status", "--short", "--branch"]).await,
        "git_diff" => {
            let args_vec = if let Some(path) = optional_string(args, "path") {
                guard.existing(path)?;
                vec!["diff", "--no-ext-diff", "--", path]
            } else {
                vec!["diff", "--no-ext-diff"]
            };
            git_read(&guard, &args_vec).await
        }
        "replace_file" => {
            if !permissions.allow_write {
                return Err(anyhow!("write was not approved"));
            }
            let (rel, _old, updated, _) = edit_preview(root, args)?;
            let target = guard.destination(&rel)?;
            let parent = target
                .parent()
                .ok_or_else(|| anyhow!("missing parent directory"))?;
            let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let tmp = parent.join(format!(".mew-edit-{}-{nonce}.tmp", std::process::id()));
            let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            file.write_all(updated.as_bytes())?;
            if let Ok(metadata) = fs::metadata(&target) {
                fs::set_permissions(&tmp, metadata.permissions())?;
            }
            file.sync_all()?;
            drop(file);
            #[cfg(windows)]
            if target.exists() {
                fs::remove_file(&target)?;
            }
            if let Err(error) = fs::rename(&tmp, &target) {
                let _ = fs::remove_file(&tmp);
                return Err(error).with_context(|| format!("replace {}", target.display()));
            }
            Ok(format!("Updated {rel} after exact-match patch approval."))
        }
        "run_check" => {
            if !permissions.allow_exec {
                return Err(anyhow!("check execution was not approved"));
            }
            let kind = required_string(args, "kind")?;
            run_check(guard.root(), kind).await
        }
        "run_command" => {
            if !permissions.allow_exec {
                return Err(anyhow!("command execution was not approved"));
            }
            let command = required_string(args, "command")?;
            if command.trim().is_empty() || command.len() > 4_000 {
                return Err(anyhow!("command must contain 1-4000 bytes"));
            }
            if args
                .get("background")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                start_background_command(guard.root(), command).await
            } else {
                run_shell_command(guard.root(), command).await
            }
        }
        "background_list" => background_command_list().await,
        "background_status" => background_command_status(required_string(args, "id")?).await,
        "background_output" => background_command_output(required_string(args, "id")?).await,
        "background_cancel" => {
            if !permissions.allow_exec {
                return Err(anyhow!("background cancellation was not approved"));
            }
            cancel_background_command(required_string(args, "id")?).await
        }
        "http_get" => {
            if !permissions.allow_network {
                return Err(anyhow!("network access was not approved"));
            }
            http_get(required_string(args, "url")?).await
        }
        other => Err(anyhow!("unknown tool: {other}")),
    }
}

const MAX_BG_OUTPUT: usize = 64 * 1024;
static NEXT_BG_ID: AtomicU64 = AtomicU64::new(1);
static BACKGROUND_COMMANDS: OnceLock<Mutex<HashMap<String, BackgroundEntry>>> = OnceLock::new();

struct BackgroundEntry {
    child: Option<Child>,
    process_group_id: Option<u32>,
    output: Arc<Mutex<String>>,
    status: String,
    command: String,
    directory: PathBuf,
}

impl Drop for BackgroundEntry {
    fn drop(&mut self) {
        terminate_process_group(self.process_group_id, false);
    }
}

fn background_store() -> &'static Mutex<HashMap<String, BackgroundEntry>> {
    BACKGROUND_COMMANDS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(unix)]
fn terminate_process_group(pid: Option<u32>, force: bool) {
    if let Some(pid) = pid {
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        unsafe {
            libc::kill(-(pid as i32), signal);
        }
    }
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: Option<u32>, _force: bool) {}

fn shell_command(root: &Path, command: &str) -> Command {
    #[cfg(windows)]
    let mut process = {
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/C", command]);
        cmd
    };
    #[cfg(not(windows))]
    let mut process = {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", command]);
        cmd
    };
    process
        .current_dir(root)
        .stdin(Stdio::null())
        .env_clear()
        .kill_on_drop(true);
    for key in [
        "PATH",
        "HOME",
        "TMPDIR",
        "TEMP",
        "TMP",
        "LANG",
        "LC_ALL",
        "SYSTEMROOT",
        "WINDIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            process.env(key, value);
        }
    }
    process
}

async fn run_shell_command(root: &Path, command: &str) -> Result<String> {
    let mut process = shell_command(root, command);
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = process.spawn().context("start local shell command")?;
    let process_group_id = child.id();
    let output = Arc::new(Mutex::new(String::new()));
    let stdout_reader = child
        .stdout
        .take()
        .map(|stream| spawn_output_reader(stream, output.clone()));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stream| spawn_output_reader(stream, output.clone()));
    let status = match timeout(Duration::from_secs(60), child.wait()).await {
        Ok(result) => result.context("wait for local shell command")?,
        Err(_) => {
            terminate_process_group(process_group_id, true);
            let _ = child.kill().await;
            bail!("command timed out after 60 seconds");
        }
    };
    terminate_process_group(process_group_id, false);
    if let Some(reader) = stdout_reader {
        let _ = reader.await;
    }
    if let Some(reader) = stderr_reader {
        let _ = reader.await;
    }
    let captured = output.lock().await.clone();
    Ok(format!("exit={status}\noutput:\n{captured}"))
}

async fn start_background_command(root: &Path, command: &str) -> Result<String> {
    let mut process = shell_command(root, command);
    process
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = process.spawn().context("start background command")?;
    let process_group_id = child.id();
    let output = Arc::new(Mutex::new(String::new()));
    if let Some(stream) = child.stdout.take() {
        spawn_output_reader(stream, output.clone());
    }
    if let Some(stream) = child.stderr.take() {
        spawn_output_reader(stream, output.clone());
    }
    let counter = NEXT_BG_ID.fetch_add(1, Ordering::Relaxed);
    let id = format!("bg-{}-{counter}", std::process::id());
    let entry = BackgroundEntry {
        child: Some(child),
        process_group_id,
        output,
        status: "running".into(),
        command: command.to_string(),
        directory: root.to_path_buf(),
    };
    let mut store = background_store().lock().await;
    if store.len() >= 100 {
        let completed = store
            .iter()
            .find(|(_, entry)| entry.child.is_none())
            .map(|(id, _)| id.clone());
        if let Some(id) = completed {
            store.remove(&id);
        } else {
            return Err(anyhow!(
                "background process limit reached (100 running processes)"
            ));
        }
    }
    store.insert(id.clone(), entry);
    Ok(format!("started background command {id} in {}\ncommand: {command}\nUse background_status/background_output/background_cancel with this id.", root.display()))
}

fn spawn_output_reader<R>(mut stream: R, output: Arc<Mutex<String>>) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buffer = [0u8; 4096];
        loop {
            let Ok(count) = stream.read(&mut buffer).await else {
                break;
            };
            if count == 0 {
                break;
            }
            let mut text = output.lock().await;
            const TRUNCATION_MARKER: &str = "\n[output truncated at 64 KiB]";
            let content_limit = MAX_BG_OUTPUT.saturating_sub(TRUNCATION_MARKER.len());
            if text.len() < content_limit {
                let chunk = String::from_utf8_lossy(&buffer[..count]);
                let mut end = (content_limit - text.len()).min(chunk.len());
                while !chunk.is_char_boundary(end) {
                    end -= 1;
                }
                text.push_str(&chunk[..end]);
                if end < chunk.len() && !text.ends_with(TRUNCATION_MARKER) {
                    text.push_str(TRUNCATION_MARKER);
                }
            } else if !text.ends_with(TRUNCATION_MARKER) {
                text.push_str(TRUNCATION_MARKER);
            }
        }
    })
}

fn refresh_background_status(entry: &mut BackgroundEntry) -> Result<()> {
    let status = entry
        .child
        .as_mut()
        .map(Child::try_wait)
        .transpose()?
        .flatten();
    if let Some(status) = status {
        terminate_process_group(entry.process_group_id, false);
        entry.status = if status.success() {
            format!("completed ({status})")
        } else {
            format!("failed ({status})")
        };
        entry.child.take();
    }
    Ok(())
}

pub async fn background_command_list() -> Result<String> {
    let mut store = background_store().lock().await;
    if store.is_empty() {
        return Ok("No Mew-managed background commands.".into());
    }
    let mut rows = Vec::new();
    for (id, entry) in store.iter_mut() {
        refresh_background_status(entry)?;
        rows.push(format!(
            "{id} · {} · {} · {}",
            entry.status,
            entry.directory.display(),
            entry.command
        ));
    }
    rows.sort();
    Ok(rows.join("\n"))
}

pub async fn background_command_status(id: &str) -> Result<String> {
    let mut store = background_store().lock().await;
    let entry = store
        .get_mut(id)
        .ok_or_else(|| anyhow!("unknown background command id {id:?}"))?;
    refresh_background_status(entry)?;
    Ok(format!(
        "{id} · {} · {}\ncommand: {}",
        entry.status,
        entry.directory.display(),
        entry.command
    ))
}

pub async fn background_command_output(id: &str) -> Result<String> {
    let store = background_store().lock().await;
    let entry = store
        .get(id)
        .ok_or_else(|| anyhow!("unknown background command id {id:?}"))?;
    let text = entry.output.lock().await.clone();
    if text.is_empty() {
        Ok("No output captured yet.".into())
    } else {
        Ok(text)
    }
}

pub async fn cancel_background_command(id: &str) -> Result<String> {
    let mut store = background_store().lock().await;
    let entry = store
        .get_mut(id)
        .ok_or_else(|| anyhow!("unknown background command id {id:?}"))?;
    if let Some(mut child) = entry.child.take() {
        terminate_process_group(entry.process_group_id, false);
        if timeout(Duration::from_secs(2), child.wait()).await.is_err() {
            terminate_process_group(entry.process_group_id, true);
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        entry.status = "cancelled".into();
        Ok(format!("stopped background command {id}"))
    } else {
        Ok(format!(
            "background command {id} is already {}",
            entry.status
        ))
    }
}

fn required_string<'a>(v: &'a Value, key: &'a str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing string argument {key}"))
}
fn optional_string<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn list_files(
    guard: &WorkspaceGuard,
    dir: &Path,
    depth: usize,
    out: &mut Vec<String>,
    limit: usize,
) -> Result<()> {
    if depth > 5 || out.len() >= limit {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= limit {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if skip_name(&name) {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(guard.root())
            .unwrap_or(&path)
            .display()
            .to_string();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            // Canonicalize before displaying/following; never recurse outside the root.
            if let Ok(canon) = fs::canonicalize(&path) {
                if canon.starts_with(guard.root()) {
                    out.push(format!("{rel}@"));
                }
            }
        } else if meta.is_dir() {
            out.push(format!("{rel}/"));
            list_files(guard, &path, depth + 1, out, limit)?;
        } else if meta.is_file() {
            out.push(rel);
        }
    }
    Ok(())
}

fn collect_files(
    guard: &WorkspaceGuard,
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    limit: usize,
) -> Result<()> {
    if depth > 5 || out.len() >= limit {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= limit {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if skip_name(&name) {
            continue;
        }
        let p = entry.path();
        let meta = fs::symlink_metadata(&p)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_files(guard, &p, depth + 1, out, limit)?;
        } else if meta.is_file() && p.starts_with(guard.root()) {
            out.push(p);
        }
    }
    Ok(())
}

fn skip_name(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "node_modules" | "target" | "dist" | "build" | "vendor" | "coverage"
        )
}

async fn git_read(guard: &WorkspaceGuard, args: &[&str]) -> Result<String> {
    let output = timeout(
        Duration::from_secs(10),
        Command::new("git")
            .arg("-C")
            .arg(guard.root())
            .args(args)
            .output(),
    )
    .await??;
    if !output.status.success() {
        return Err(anyhow!(
            "git returned {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    bounded(String::from_utf8_lossy(&output.stdout).to_string(), 40_000)
}

async fn run_check(root: &Path, kind: &str) -> Result<String> {
    let (program, args): (&str, Vec<&str>) = if root.join("Cargo.toml").exists() {
        match kind {
            "fmt" => ("cargo", vec!["fmt", "--all", "--", "--check"]),
            "check" => ("cargo", vec!["check", "--workspace"]),
            "test" => ("cargo", vec!["test", "--workspace"]),
            _ => return Err(anyhow!("unsupported check kind")),
        }
    } else if root.join("package.json").exists() {
        match kind {
            "fmt" => return Err(anyhow!("no safe default formatter check for this project")),
            "check" => ("npm", vec!["run", "--if-present", "typecheck"]),
            "test" => ("npm", vec!["test", "--", "--runInBand"]),
            _ => return Err(anyhow!("unsupported check kind")),
        }
    } else if root.join("pyproject.toml").exists() {
        match kind {
            "fmt" => return Err(anyhow!("no safe default formatter check for this project")),
            "check" => ("python", vec!["-m", "compileall", "-q", "."]),
            "test" => ("python", vec!["-m", "pytest", "-q"]),
            _ => return Err(anyhow!("unsupported check kind")),
        }
    } else if root.join("go.mod").exists() {
        match kind {
            "fmt" => ("gofmt", vec!["-d", "."]),
            "check" => ("go", vec!["vet", "./..."]),
            "test" => ("go", vec!["test", "./..."]),
            _ => return Err(anyhow!("unsupported check kind")),
        }
    } else {
        return Err(anyhow!("no supported project manifest found"));
    };
    let mut command = Command::new(program);
    command
        .args(&args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    let mut child = command.spawn().context("start project check")?;
    let process_group_id = child.id();
    let output = Arc::new(Mutex::new(String::new()));
    let stdout_reader = child
        .stdout
        .take()
        .map(|stream| spawn_output_reader(stream, output.clone()));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stream| spawn_output_reader(stream, output.clone()));
    let status = match timeout(Duration::from_secs(180), child.wait()).await {
        Ok(result) => result.context("wait for project check")?,
        Err(_) => {
            terminate_process_group(process_group_id, true);
            let _ = child.kill().await;
            bail!("check timed out after 180 seconds");
        }
    };
    terminate_process_group(process_group_id, false);
    if let Some(reader) = stdout_reader {
        let _ = reader.await;
    }
    if let Some(reader) = stderr_reader {
        let _ = reader.await;
    }
    let captured = output.lock().await.clone();
    bounded(
        format!(
            "$ {program} {}\nexit: {}\n{captured}",
            args.join(" "),
            status
        ),
        24_000,
    )
}

async fn http_get(raw: &str) -> Result<String> {
    let url = reqwest::Url::parse(raw)?;
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return Err(anyhow!(
            "only public HTTPS URLs without embedded credentials are allowed"
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("URL has no host"))?
        .to_ascii_lowercase();
    let lookup_host = host.trim_start_matches('[').trim_end_matches(']');
    if lookup_host == "localhost"
        || lookup_host.ends_with(".localhost")
        || lookup_host.ends_with(".local")
        || lookup_host.ends_with(".internal")
        || lookup_host.parse::<std::net::IpAddr>().is_ok()
    {
        return Err(anyhow!("local and literal-IP hosts are blocked"));
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::net::lookup_host((lookup_host, port))
        .await?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|addr| is_non_public_ip(addr.ip())) {
        return Err(anyhow!(
            "host resolved to no public addresses or to a private/reserved address"
        ));
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .resolve_to_addrs(&host, &addresses)
        .build()?;
    let response = client
        .get(url)
        .header(
            reqwest::header::ACCEPT,
            "text/plain, text/markdown, application/json, text/html;q=0.7",
        )
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow!("HTTP {}", response.status()));
    }
    const LIMIT: usize = 100_000;
    if response.content_length().is_some_and(|n| n > LIMIT as u64) {
        return Err(anyhow!("response exceeds 100 KB limit"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > LIMIT {
            return Err(anyhow!("response exceeds 100 KB limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn is_non_public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || a == 0
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0 && (c == 0 || c == 2))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113)
        }
        std::net::IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || first & 0xfe00 == 0xfc00
                || first & 0xffc0 == 0xfe80
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_non_public_ip(v4.into()))
        }
    }
}

fn bounded(s: String, limit: usize) -> Result<String> {
    if s.len() <= limit {
        Ok(s)
    } else {
        Ok(format!(
            "{}\n[truncated at {limit} bytes]",
            &s[..s.floor_char_boundary(limit)]
        ))
    }
}

fn simple_diff(path: &str, old: &str, new: &str) -> String {
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    for line in old.lines() {
        out.push('-');
        out.push_str(line);
        out.push('\n');
    }
    for line in new.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_shell_tools_are_registered_only_for_auto_mode() {
        let normal = tool_definitions(true, true, true);
        assert!(!normal.iter().any(|tool| tool.name == "run_command"));
        let auto = tool_definitions_with_shell(true, true, false, true);
        assert!(auto.iter().any(|tool| tool.name == "run_command"));
        assert!(auto.iter().any(|tool| tool.name == "background_cancel"));
        assert!(!auto.iter().any(|tool| tool.name == "http_get"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn background_command_can_be_observed_and_cancelled() {
        let root = std::env::temp_dir().join(format!(
            "mew-bg-test-{}-{}",
            std::process::id(),
            NEXT_BG_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let permissions = ToolPermissions {
            allow_exec: true,
            ..ToolPermissions::default()
        };
        let start = execute_tool(
            &root,
            "run_command",
            &json!({"command":"printf lifecycle-ready; sleep 15", "background":true}),
            permissions,
        )
        .await
        .unwrap();
        let id = start.split_whitespace().nth(3).unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(background_command_status(id)
            .await
            .unwrap()
            .contains("running"));
        assert!(background_command_output(id)
            .await
            .unwrap()
            .contains("lifecycle-ready"));
        execute_tool(&root, "background_cancel", &json!({"id":id}), permissions)
            .await
            .unwrap();
        assert!(background_command_status(id)
            .await
            .unwrap()
            .contains("cancelled"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn edit_preview_requires_unique_match() {
        let dir = std::env::temp_dir().join(format!("mew-tools-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "one\none\n").unwrap();
        let args = json!({"path":"a.txt","old_text":"one","new_text":"two"});
        assert!(edit_preview(&dir, &args).is_err());
        fs::write(dir.join("a.txt"), "one\n").unwrap();
        assert!(edit_preview(&dir, &args).unwrap().3.contains("+two"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn private_and_reserved_http_addresses_are_blocked() {
        for address in [
            "127.0.0.1",
            "10.2.3.4",
            "169.254.1.2",
            "100.64.0.1",
            "192.0.2.1",
            "::1",
            "fc00::1",
            "fe80::1",
        ] {
            let ip = address.parse().unwrap();
            assert!(is_non_public_ip(ip), "{address} should be rejected");
        }
        for address in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            let ip = address.parse().unwrap();
            assert!(!is_non_public_ip(ip), "{address} should be permitted");
        }
    }
}

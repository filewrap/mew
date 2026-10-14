use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::{timeout_at, Instant};

const HOOK_FILE: &str = ".mew/hooks.toml";
const MAX_HOOK_OUTPUT: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    SessionEnd,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "session_start",
            Self::UserPromptSubmit => "user_prompt_submit",
            Self::PreToolUse => "pre_tool_use",
            Self::PostToolUse => "post_tool_use",
            Self::SessionEnd => "session_end",
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct HookFile {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
    #[serde(default)]
    hooks: Vec<HookSpec>,
}

#[derive(Debug, Deserialize)]
struct HookSpec {
    event: String,
    /// Executable and arguments, passed directly without invoking a shell.
    command: Vec<String>,
    #[serde(default = "default_true")]
    blocking: bool,
}

fn default_timeout() -> u64 {
    10
}
fn default_true() -> bool {
    true
}

pub fn hooks_path(root: &Path) -> PathBuf {
    root.join(HOOK_FILE)
}

pub fn read_hook_file(root: &Path) -> Result<Option<String>> {
    let directory = root.join(".mew");
    if fs::symlink_metadata(&directory).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!("refusing to read hooks through a .mew symlink");
    }
    let path = hooks_path(root);
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!("refusing to read a symlinked hooks.toml");
    }
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn validate_hook_file(root: &Path) -> Result<(bool, usize)> {
    let Some(text) = read_hook_file(root)? else {
        return Ok((false, 0));
    };
    let config: HookFile = toml::from_str(&text).context("parse .mew/hooks.toml")?;
    validate_config(&config)?;
    Ok((config.enabled, config.hooks.len()))
}

fn validate_config(config: &HookFile) -> Result<()> {
    if !(1..=30).contains(&config.timeout_seconds) {
        bail!("hooks timeout_seconds must be between 1 and 30");
    }
    for hook in &config.hooks {
        if !matches!(
            hook.event.as_str(),
            "session_start"
                | "user_prompt_submit"
                | "pre_tool_use"
                | "post_tool_use"
                | "session_end"
        ) {
            bail!("unknown hook event {:?}", hook.event);
        }
        if hook.command.is_empty() || hook.command[0].trim().is_empty() {
            bail!("hook for {} needs a non-empty command array", hook.event);
        }
        if hook.command.len() > 32 || hook.command.iter().any(|part| part.len() > 4096) {
            bail!("hook command has too many or oversized arguments");
        }
    }
    Ok(())
}

/// Run configured hooks for one lifecycle event. Hooks are completely inactive
/// unless `.mew/hooks.toml` explicitly sets `enabled = true`.
pub async fn run_hook_event(root: &Path, event: HookEvent, payload: Value) -> Result<Vec<String>> {
    let Some(text) = read_hook_file(root)? else {
        return Ok(Vec::new());
    };
    let config: HookFile = toml::from_str(&text).context("parse .mew/hooks.toml")?;
    validate_config(&config)?;
    if !config.enabled {
        return Ok(Vec::new());
    }
    let input = serde_json::to_vec(&payload)?;
    let timeout_duration = Duration::from_secs(config.timeout_seconds);
    let mut results = Vec::new();
    for hook in config
        .hooks
        .iter()
        .filter(|hook| hook.event == event.as_str())
    {
        let outcome = execute_hook(root, event, hook, &input, timeout_duration).await;
        let (status, stdout, stderr) = match outcome {
            Ok(output) => output,
            Err(error) if hook.blocking => return Err(error),
            Err(error) => {
                eprintln!(
                    "Warning: non-blocking {} hook failed: {error:#}",
                    event.as_str()
                );
                continue;
            }
        };
        let decision = serde_json::from_str::<Value>(&stdout).ok();
        let denied = decision
            .as_ref()
            .and_then(|value| value.get("decision"))
            .and_then(Value::as_str)
            == Some("deny");
        let reason = decision
            .as_ref()
            .and_then(|value| value.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let failure = if denied {
            Some(if reason.is_empty() {
                format!("{} hook denied the operation", event.as_str())
            } else {
                format!("{} hook denied the operation: {reason}", event.as_str())
            })
        } else if !status.success() {
            Some(format!(
                "{} hook exited with {}{}",
                event.as_str(),
                status,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            ))
        } else {
            None
        };
        if let Some(failure) = failure {
            if hook.blocking {
                return Err(anyhow!(failure));
            }
            eprintln!("Warning: {failure}");
        } else if !stdout.trim().is_empty() {
            results.push(stdout);
        }
    }
    Ok(results)
}

async fn execute_hook(
    root: &Path,
    event: HookEvent,
    hook: &HookSpec,
    input: &[u8],
    timeout_duration: Duration,
) -> Result<(ExitStatus, String, String)> {
    let mut command = Command::new(&hook.command[0]);
    command
        .args(&hook.command[1..])
        .current_dir(root)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
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
            command.env(key, value);
        }
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("start {} hook {:?}", event.as_str(), hook.command[0]))?;
    let process_group_id = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("hook stdout pipe missing"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("hook stderr pipe missing"))?;
    let stdout_reader = tokio::spawn(read_bounded(stdout));
    let stderr_reader = tokio::spawn(read_bounded(stderr));
    let deadline = Instant::now() + timeout_duration;
    if let Some(mut stdin) = child.stdin.take() {
        let write_result = timeout_at(deadline, async {
            stdin.write_all(input).await?;
            stdin.shutdown().await
        })
        .await;
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                abort_hook(&mut child, process_group_id, stdout_reader, stderr_reader).await;
                return Err(error).context("write hook event payload");
            }
            Err(_) => {
                abort_hook(&mut child, process_group_id, stdout_reader, stderr_reader).await;
                return Err(anyhow!(
                    "{} hook timed out writing its event payload",
                    event.as_str()
                ));
            }
        }
    }
    let status = match timeout_at(deadline, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            abort_hook(&mut child, process_group_id, stdout_reader, stderr_reader).await;
            return Err(error).context("wait for lifecycle hook");
        }
        Err(_) => {
            abort_hook(&mut child, process_group_id, stdout_reader, stderr_reader).await;
            return Err(anyhow!("{} hook timed out", event.as_str()));
        }
    };
    terminate_hook_group(process_group_id, false);
    let stdout = stdout_reader.await.context("join hook stdout reader")??;
    let stderr = stderr_reader.await.context("join hook stderr reader")??;
    Ok((status, stdout, stderr))
}

async fn abort_hook(
    child: &mut tokio::process::Child,
    process_group_id: Option<u32>,
    stdout_reader: tokio::task::JoinHandle<Result<String>>,
    stderr_reader: tokio::task::JoinHandle<Result<String>>,
) {
    terminate_hook_group(process_group_id, true);
    let _ = child.kill().await;
    let _ = child.wait().await;
    let _ = stdout_reader.await;
    let _ = stderr_reader.await;
}

#[cfg(unix)]
fn terminate_hook_group(pid: Option<u32>, force: bool) {
    if let Some(pid) = pid {
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        unsafe {
            libc::kill(-(pid as i32), signal);
        }
    }
}

#[cfg(not(unix))]
fn terminate_hook_group(_pid: Option<u32>, _force: bool) {}

async fn read_bounded<R: AsyncRead + Unpin>(mut stream: R) -> Result<String> {
    let mut captured = Vec::with_capacity(MAX_HOOK_OUTPUT);
    let mut buffer = [0u8; 4096];
    let mut truncated = false;
    loop {
        let count = stream.read(&mut buffer).await.context("read hook output")?;
        if count == 0 {
            break;
        }
        let remaining = MAX_HOOK_OUTPUT.saturating_sub(captured.len());
        let take = remaining.min(count);
        captured.extend_from_slice(&buffer[..take]);
        truncated |= take < count;
    }
    let mut text = String::from_utf8_lossy(&captured).to_string();
    if truncated {
        text.push_str("\n[hook output truncated]");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

    fn test_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mew-hook-test-{}-{}",
            std::process::id(),
            NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[tokio::test]
    async fn hooks_are_inactive_without_explicit_enablement() {
        let root = test_root();
        fs::create_dir_all(root.join(".mew")).unwrap();
        fs::write(
            root.join(HOOK_FILE),
            "enabled = false\n[[hooks]]\nevent = 'pre_tool_use'\ncommand = ['/bin/false']\n",
        )
        .unwrap();
        let results = run_hook_event(&root, HookEvent::PreToolUse, json!({"tool":"read_file"}))
            .await
            .unwrap();
        assert!(results.is_empty());
        assert_eq!(validate_hook_file(&root).unwrap(), (false, 1));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn event_payload_write_obeys_timeout_for_non_reading_hooks() {
        let root = test_root();
        let hook = HookSpec {
            event: "pre_tool_use".into(),
            command: vec!["sleep".into(), "5".into()],
            blocking: true,
        };
        let input = vec![b'x'; 1024 * 1024];
        let started = std::time::Instant::now();
        let error = execute_hook(
            &root,
            HookEvent::PreToolUse,
            &hook,
            &input,
            Duration::from_millis(250),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(3));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn blocking_pre_tool_hook_can_deny_an_operation() {
        let root = test_root();
        fs::create_dir_all(root.join(".mew")).unwrap();
        fs::write(
            root.join(HOOK_FILE),
            r#"enabled = true
 timeout_seconds = 3
 [[hooks]]
 event = "pre_tool_use"
 command = ["printf", '{"decision":"deny","reason":"test policy"}']
 blocking = true
 "#,
        )
        .unwrap();
        let result =
            run_hook_event(&root, HookEvent::PreToolUse, json!({"tool":"replace_file"})).await;
        assert!(result.unwrap_err().to_string().contains("test policy"));
        fs::remove_dir_all(root).unwrap();
    }
}

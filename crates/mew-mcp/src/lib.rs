use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct McpClient {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpClient {
    pub async fn start(program: &str, args: &[String], cwd: Option<&Path>) -> Result<Self> {
        Self::start_with_env(program, args, cwd, &BTreeMap::new()).await
    }

    pub async fn start_with_env(
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
        environment: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(environment)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("start MCP server {program:?}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("MCP server stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP server stdout unavailable"))?;
        let mut client = Self {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        let initialized = client
            .request(
                "initialize",
                json!({
                    "protocolVersion":"2024-11-05",
                    "capabilities":{},
                    "clientInfo":{"name":"mew","version":env!("CARGO_PKG_VERSION")}
                }),
            )
            .await?;
        let version = initialized
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or("");
        if version.is_empty() {
            return Err(anyhow!(
                "MCP server returned an invalid initialize response"
            ));
        }
        client
            .notify("notifications/initialized", json!({}))
            .await?;
        Ok(client)
    }

    pub async fn list_tools(&mut self) -> Result<Vec<Value>> {
        let response = self.request("tools/list", json!({})).await?;
        Ok(response
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        self.request("tools/call", json!({"name":name,"arguments":arguments}))
            .await
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.write_message(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).await?;
            if read == 0 {
                return Err(anyhow!("MCP server closed stdout before replying"));
            }
            if line.len() > 2_000_000 {
                return Err(anyhow!("MCP message exceeds 2 MB"));
            }
            let value: Value =
                serde_json::from_str(line.trim()).context("invalid MCP JSON-RPC response")?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(anyhow!("MCP error: {}", error));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.write_message(json!({"jsonrpc":"2.0","method":method,"params":params}))
            .await
    }

    async fn write_message(&mut self, value: Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(&value)?;
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await?;
        self.stdin.flush().await?;
        Ok(())
    }
}

/// Serve a read-only MCP bridge over stdio. Stdout is protocol-only; logs/errors use stderr.
pub async fn serve_stdio(root: impl AsRef<Path>) -> Result<()> {
    let root = PathBuf::from(root.as_ref());
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        if line.len() > 2_000_000 {
            continue;
        }
        let request: Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            continue;
        };
        let id = request.get("id").cloned();
        if method.starts_with("notifications/") {
            continue;
        }
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion":"2024-11-05",
                "capabilities":{"tools":{"listChanged":false}},
                "serverInfo":{"name":"mew-native-tools","version":env!("CARGO_PKG_VERSION")}
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(
                json!({"tools": mew_tools::tool_definitions(false, false, false).into_iter().map(|t| json!({"name":t.name,"description":t.description,"inputSchema":t.parameters})).collect::<Vec<_>>()}),
            ),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match mew_tools::execute_tool(
                    &root,
                    name,
                    &args,
                    mew_tools::ToolPermissions::default(),
                )
                .await
                {
                    Ok(text) => {
                        Ok(json!({"content":[{"type":"text","text":text}],"isError":false}))
                    }
                    Err(err) => Ok(
                        json!({"content":[{"type":"text","text":format!("{err:#}")}],"isError":true}),
                    ),
                }
            }
            _ => Err(json!({"code":-32601,"message":format!("method not found: {method}")})),
        };
        if let Some(id) = id {
            let response = match result {
                Ok(value) => json!({"jsonrpc":"2.0","id":id,"result":value}),
                Err(error) => json!({"jsonrpc":"2.0","id":id,"error":error}),
            };
            let mut bytes = serde_json::to_vec(&response)?;
            bytes.push(b'\n');
            stdout.write_all(&bytes).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn built_in_mcp_tools_are_read_only() {
        let names = mew_tools::tool_definitions(false, false, false)
            .into_iter()
            .map(|t| t.name)
            .collect::<Vec<_>>();
        assert!(!names
            .iter()
            .any(|n| n == "replace_file" || n == "run_check" || n == "http_get"));
    }
}

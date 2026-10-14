use crate::args::{McpCommand, McpSubcommand};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;

pub async fn run(root: &Path, cmd: McpCommand) -> Result<()> {
    match cmd.command {
        McpSubcommand::Serve => mew_mcp::serve_stdio(root).await,
        McpSubcommand::Tools { server } => {
            let (program, args) = split_server(server)?;
            let mut client = timeout(
                Duration::from_secs(30),
                mew_mcp::McpClient::start(&program, &args, Some(root)),
            )
            .await??;
            let tools = timeout(Duration::from_secs(30), client.list_tools()).await??;
            println!("{}", serde_json::to_string_pretty(&tools)?);
            Ok(())
        }
        McpSubcommand::Call {
            tool,
            arguments_json,
            server,
        } => {
            let args: Value = serde_json::from_str(&arguments_json)?;
            let (program, server_args) = split_server(server)?;
            let mut client = timeout(
                Duration::from_secs(30),
                mew_mcp::McpClient::start(&program, &server_args, Some(root)),
            )
            .await??;
            let tools = timeout(Duration::from_secs(30), client.list_tools()).await??;
            if !tools
                .iter()
                .any(|t| t.get("name").and_then(Value::as_str) == Some(&tool))
            {
                return Err(anyhow!("MCP server does not advertise tool {tool:?}"));
            }
            let result = timeout(Duration::from_secs(60), client.call_tool(&tool, args)).await??;
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
    }
}

fn split_server(mut values: Vec<String>) -> Result<(String, Vec<String>)> {
    if values.is_empty() {
        return Err(anyhow!("supply an MCP server command"));
    }
    let program = values.remove(0);
    Ok((program, values))
}

use crate::args::AgentModeArg;
use anyhow::Result;
use mew_common::{MewConfig, MewPaths};

pub async fn run(
    paths: &MewPaths,
    cfg: &MewConfig,
    prompt: String,
    model: Option<String>,
) -> Result<()> {
    super::agent::run(
        paths,
        cfg,
        super::agent::AgentRunOptions {
            prompt,
            model,
            mode: AgentModeArg::Ask,
            allow_checks: false,
            allow_network: false,
            yes: false,
            requested_skills: Vec::new(),
            subagent_id: None,
        },
    )
    .await
}

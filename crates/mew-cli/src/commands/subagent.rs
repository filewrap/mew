use crate::args::{SubagentCommand, SubagentSubcommand};
use crate::commands::definitions::{
    create_definition_file, load_agent_definitions, validate_id, workspace_definition_dir,
    AgentDefinition,
};
use anyhow::{anyhow, bail, Result};
use mew_common::{MewConfig, MewPaths};
use std::path::Path;

pub async fn run(
    paths: &MewPaths,
    root: &Path,
    cfg: &MewConfig,
    command: SubagentCommand,
) -> Result<()> {
    match command.command {
        SubagentSubcommand::List => {
            let definitions = load_agent_definitions(root)?;
            if definitions.is_empty() {
                println!(
                    "No workspace subagents found. Create one with `mew subagent init <id>`. "
                );
            }
            for definition in definitions {
                println!(
                    "{:<24} model={} skills={}",
                    definition.id,
                    definition
                        .model
                        .as_deref()
                        .unwrap_or(&cfg.providers.active_model),
                    if definition.skills.is_empty() {
                        "none".into()
                    } else {
                        definition.skills.join(",")
                    },
                );
            }
            Ok(())
        }
        SubagentSubcommand::Init { id } => {
            let directory =
                workspace_definition_dir(root, "agents", true)?.expect("created directory");
            let path = create_definition_file(&directory, &id, &AgentDefinition::scaffold(&id))?;
            println!(
                "created {}",
                path.strip_prefix(root).unwrap_or(&path).display()
            );
            Ok(())
        }
        SubagentSubcommand::Show { id } => {
            validate_id(&id, "agent")?;
            let definition = load_agent_definitions(root)?
                .into_iter()
                .find(|definition| definition.id == id)
                .ok_or_else(|| anyhow!("unknown workspace subagent {id:?}"))?;
            println!("{}", toml::to_string_pretty(&definition)?);
            Ok(())
        }
        SubagentSubcommand::Run { id, prompt, model } => {
            let definitions = load_agent_definitions(root)?;
            if !definitions.iter().any(|definition| definition.id == id) {
                bail!("unknown workspace subagent {id:?}; create it with `mew subagent init {id}`");
            }
            super::agent::run(
                paths,
                cfg,
                super::agent::AgentRunOptions {
                    prompt,
                    model,
                    mode: crate::args::AgentModeArg::Ask,
                    allow_checks: false,
                    allow_network: false,
                    yes: false,
                    requested_skills: Vec::new(),
                    subagent_id: Some(id),
                },
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_uses_reviewable_safe_defaults() {
        let definition = AgentDefinition::scaffold("test-agent");
        assert_eq!(definition.id, "test-agent");
        assert_eq!(definition.max_steps, Some(8));
        assert!(definition.model.is_none());
    }
}

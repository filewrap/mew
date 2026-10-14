use crate::args::AgentModeArg;
use crate::commands::definitions::{build_agent_profile, load_agent_definitions};
use anyhow::{anyhow, Result};
use mew_common::{MewConfig, MewPaths};
use mew_core::{run_agent_with_profiles, AgentMode, AgentOptions};
use mew_provider::ProviderRegistry;
use mew_session::{save_session, MewSession};
use mew_tokens::usage_report;
use mew_ui::{assistant_bubble_document, meta_line, status_line, theme_by_name};
use std::io::{self, IsTerminal, Write};
use std::time::Instant;

pub struct AgentRunOptions {
    pub prompt: String,
    pub model: Option<String>,
    pub mode: AgentModeArg,
    pub allow_checks: bool,
    pub allow_network: bool,
    pub yes: bool,
    pub requested_skills: Vec<String>,
    pub subagent_id: Option<String>,
}

pub async fn run(paths: &MewPaths, cfg: &MewConfig, request: AgentRunOptions) -> Result<()> {
    let AgentRunOptions {
        prompt,
        model,
        mode,
        allow_checks,
        allow_network,
        yes,
        requested_skills,
        subagent_id,
    } = request;
    let root = std::env::current_dir()?.canonicalize()?;
    let definitions = load_agent_definitions(&root)?;
    let selected_subagent = match subagent_id {
        Some(id) => Some(
            definitions
                .iter()
                .find(|definition| definition.id == id)
                .ok_or_else(|| anyhow!("unknown workspace subagent {id:?}"))?
                .clone(),
        ),
        None => None,
    };
    let model_ref = model
        .or_else(|| {
            selected_subagent
                .as_ref()
                .and_then(|definition| definition.model.clone())
        })
        .unwrap_or_else(|| cfg.providers.active_model.clone());
    let (provider_id, model_id) = ProviderRegistry::parse_model_ref(&model_ref)?;
    let registry = ProviderRegistry::from_config(cfg);
    let provider = registry.get(&provider_id)?;
    if !provider.is_authorized() {
        return Err(anyhow!(
            "provider {provider_id} is not authorized; configure its API key first"
        ));
    }
    let mut subagent_profiles = Vec::new();
    for definition in &definitions {
        if selected_subagent
            .as_ref()
            .is_some_and(|selected| selected.id == definition.id)
        {
            continue;
        }
        subagent_profiles.push(build_agent_profile(
            definition, cfg, &registry, &root, None,
        )?);
    }
    let model_context = provider
        .models()
        .into_iter()
        .find(|m| m.id == model_id)
        .map(|m| m.context)
        .filter(|n| *n > 0)
        .unwrap_or(32_000);
    let theme = theme_by_name(&cfg.style.theme);
    let mode_core = match mode {
        AgentModeArg::Ask => AgentMode::Ask,
        AgentModeArg::Edit => AgentMode::Edit,
        AgentModeArg::Auto => AgentMode::Auto,
    };
    if mode_core == AgentMode::Auto && allow_network {
        return Err(anyhow!("auto mode disables network access; use edit mode with --allow-network for individually approved requests"));
    }
    if mode_core == AgentMode::Ask && allow_checks {
        return Err(anyhow!("--allow-checks is available only with --mode edit"));
    }
    let mut skill_names = requested_skills;
    if let Some(definition) = &selected_subagent {
        skill_names.extend(definition.skills.iter().cloned());
    }
    if !skill_names.iter().any(|s| s == "caveman") {
        skill_names.push("caveman".to_string());
    }
    skill_names.sort();
    skill_names.dedup();
    if skill_names.len() > 5 {
        return Err(anyhow!("at most five skills may be loaded at once"));
    }
    let mut skills = Vec::new();
    for name in &skill_names {
        skills.push(mew_skills::load(&root, name)?);
    }

    println!(
        "{}",
        status_line(
            &theme,
            &format!(
                "{} mode · model {provider_id}/{model_id} · workspace {}",
                match mode_core {
                    AgentMode::Ask => "read-only ask",
                    AgentMode::Edit => "edit",
                    AgentMode::Auto => "auto · local edits/commands enabled; network/MCP disabled",
                },
                root.display()
            )
        )
    );
    if mode_core == AgentMode::Ask {
        println!(
            "{}",
            status_line(
                &theme,
                "filesystem and git tools only; no edits, shell commands, or network requests"
            )
        );
    }
    let start = Instant::now();
    let selected_limits = selected_subagent.as_ref();
    let options = AgentOptions {
        mode: mode_core,
        allow_exec: allow_checks || mode_core == AgentMode::Auto,
        allow_network: allow_network && mode_core != AgentMode::Auto,
        allow_external: false,
        auto_approve: yes || mode_core == AgentMode::Auto,
        max_steps: selected_limits
            .and_then(|definition| definition.max_steps)
            .unwrap_or(12),
        context_tokens: selected_limits
            .and_then(|definition| definition.context_tokens)
            .unwrap_or(model_context),
        response_reserve: selected_limits
            .and_then(|definition| definition.response_reserve)
            .unwrap_or((model_context / 10).clamp(1_000, 4_000)),
        system_prompt: selected_subagent.map(|definition| definition.system_prompt),
    };
    let result = run_agent_with_profiles(provider, &model_id, &prompt, &root, &skills, options, subagent_profiles, move |preview| {
        println!("\nProposed action:\n{preview}");
        if yes { return true; }
        if !io::stdin().is_terminal() {
            eprintln!("Not applied: interactive approval is unavailable. Use --yes only if you intend to approve these actions.");
            return false;
        }
        print!("Proceed? [y/N] ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).is_ok() && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
    }).await?;

    println!();
    println!(
        "{}",
        assistant_bubble_document(&theme, &cfg.identity.display_name, &result.presentation,)
    );
    let mut session = MewSession::new(prompt, provider_id.clone(), model_id.clone());
    session.messages = result.transcript;
    save_session(paths, &session).await?;
    let elapsed = start.elapsed();
    println!(
        "{}",
        meta_line(
            &theme,
            &format!(
                "time={}ms model={}/{} {} calls={} session={}",
                elapsed.as_millis(),
                provider_id,
                model_id,
                usage_report(
                    result.input_tokens,
                    result.output_tokens,
                    result.estimated_input_tokens
                ),
                result.iterations,
                session.id
            )
        )
    );
    Ok(())
}

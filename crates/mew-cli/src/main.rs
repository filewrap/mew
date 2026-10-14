mod args;
mod commands;

use anyhow::Result;
use args::{AgentModeArg, Cli, Commands};
use clap::Parser;
use mew_common::{hydrate_environment, MewConfig, MewPaths};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_env_filter("warn")
        .init();

    let cli = Cli::parse();
    let fullscreen = !cli.inline;
    let paths = MewPaths::discover()?;
    let mut cfg = MewConfig::load_or_create(&paths)?;
    hydrate_environment(&paths)?;
    let root = std::env::current_dir()?.canonicalize()?;
    let first_run = cli.command.is_none() && !cfg.setup_complete;

    match cli.command {
        Some(Commands::Setup) => commands::setup::run(&paths, &mut cfg).await?,
        None if first_run => commands::setup::run(&paths, &mut cfg).await?,
        Some(Commands::Doctor) => commands::doctor::run(&paths, &cfg)?,
        Some(Commands::Init { dry_run }) => commands::init::run(&paths, &cfg, dry_run)?,
        Some(Commands::Name(cmd)) => commands::name::run(&paths, &mut cfg, cmd)?,
        Some(Commands::Style(cmd)) => commands::style::run(&paths, &mut cfg, cmd)?,
        Some(Commands::Config(cmd)) => commands::config::run(&paths, &cfg, cmd)?,
        Some(Commands::Provider(cmd)) => commands::provider::run(&paths, &mut cfg, cmd).await?,
        Some(Commands::Model(cmd)) => commands::model::run(&paths, &mut cfg, cmd).await?,
        Some(Commands::Ask { prompt, model }) => {
            commands::ask::run(&paths, &cfg, prompt, model).await?
        }
        Some(Commands::Agent(args)) => {
            commands::agent::run(
                &paths,
                &cfg,
                commands::agent::AgentRunOptions {
                    prompt: args.prompt,
                    model: args.model,
                    mode: args.mode,
                    allow_checks: args.allow_checks,
                    allow_network: args.allow_network,
                    yes: args.yes,
                    requested_skills: args.skills,
                    subagent_id: None,
                },
            )
            .await?
        }
        Some(Commands::Fix {
            prompt,
            model,
            allow_checks,
            allow_network,
            yes,
            skills,
        }) => {
            commands::agent::run(
                &paths,
                &cfg,
                commands::agent::AgentRunOptions {
                    prompt,
                    model,
                    mode: AgentModeArg::Edit,
                    allow_checks,
                    allow_network,
                    yes,
                    requested_skills: skills,
                    subagent_id: None,
                },
            )
            .await?
        }
        Some(Commands::Review { prompt, model }) => {
            commands::agent::run(
                &paths,
                &cfg,
                commands::agent::AgentRunOptions {
                    prompt,
                    model,
                    mode: AgentModeArg::Ask,
                    allow_checks: false,
                    allow_network: false,
                    yes: false,
                    requested_skills: vec!["review".to_string()],
                    subagent_id: None,
                },
            )
            .await?
        }
        Some(Commands::Council { prompt, model }) => {
            commands::council::run(&root, &cfg, prompt, model).await?
        }
        Some(Commands::Skills(cmd)) => commands::skills::run(&root, &cfg, cmd)?,
        Some(Commands::Mcp(cmd)) => commands::mcp::run(&root, cmd).await?,
        Some(Commands::Workflow(cmd)) => commands::workflow::run(&root, &cfg, cmd, false).await?,
        Some(Commands::Pipeline(cmd)) => commands::workflow::run(&root, &cfg, cmd, true).await?,
        Some(Commands::Subagent(cmd)) => commands::subagent::run(&paths, &root, &cfg, cmd).await?,
        Some(Commands::Worktree(cmd)) => commands::worktree::run(&root, cmd.command)?,
        Some(Commands::Hooks(cmd)) => commands::hooks::run(&root, cmd.command)?,
        Some(Commands::Chat { model }) => {
            commands::chat::run(&paths, &mut cfg, model, fullscreen).await?
        }
        Some(Commands::Session(cmd)) => {
            commands::session::run(&paths, &mut cfg, cmd, fullscreen).await?
        }
        None => commands::chat::run(&paths, &mut cfg, None, fullscreen).await?,
    }

    Ok(())
}

#[cfg(test)]
mod cli_startup_tests {
    use super::*;

    #[test]
    fn bare_chat_and_setup_have_distinct_cli_routes() {
        let default_cli = Cli::try_parse_from(["mew"]).unwrap();
        assert!(default_cli.command.is_none());
        assert!(!default_cli.inline, "full-screen chat is the default");
        assert!(Cli::try_parse_from(["mew", "--inline"]).unwrap().inline);
        assert!(
            Cli::try_parse_from(["mew", "chat", "--inline"])
                .unwrap()
                .inline
        );
        assert!(matches!(
            Cli::try_parse_from(["mew", "chat"]).unwrap().command,
            Some(Commands::Chat { model: None })
        ));
        assert!(matches!(
            Cli::try_parse_from(["mew", "setup"]).unwrap().command,
            Some(Commands::Setup)
        ));
    }

    #[test]
    fn gemini_provider_login_has_a_cli_route() {
        let parsed = Cli::try_parse_from(["mew", "provider", "login", "gemini"]).unwrap();
        assert!(matches!(
            parsed.command,
            Some(Commands::Provider(crate::args::ProviderCommand {
                command: crate::args::ProviderSubcommand::Login { provider }
            })) if provider == "gemini"
        ));
    }

    #[test]
    fn flag_and_subcommand_help_include_the_quick_start() {
        for args in [vec!["mew", "--help"], vec!["mew", "help"]] {
            let error = Cli::try_parse_from(args).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
            assert!(error.to_string().contains("Quick start:"));
        }
    }

    #[test]
    fn style_command_accepts_reduced_motion_preference() {
        let parsed = Cli::try_parse_from(["mew", "style", "motion", "reduced"]).unwrap();
        assert!(matches!(
            parsed.command,
            Some(Commands::Style(crate::args::StyleCommand {
                command: crate::args::StyleSubcommand::Motion {
                    mode: crate::args::MotionMode::Reduced
                }
            }))
        ));
    }
}

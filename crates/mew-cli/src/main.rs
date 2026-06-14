mod args;
mod commands;

use anyhow::Result;
use args::{Cli, Commands};
use clap::Parser;
use mew_common::{MewConfig, MewPaths};
use mew_ui::{clear_screen, hint_card, startup_banner};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_env_filter("warn")
        .init();

    let cli = Cli::parse();
    let paths = MewPaths::discover()?;
    let mut cfg = MewConfig::load_or_create(&paths)?;

    match cli.command {
        Some(Commands::Doctor) => commands::doctor::run(&paths, &cfg)?,
        Some(Commands::Init { dry_run }) => commands::init::run(&paths, &cfg, dry_run)?,
        Some(Commands::Name(cmd)) => commands::name::run(&paths, &mut cfg, cmd)?,
        Some(Commands::Style(cmd)) => commands::style::run(&paths, &mut cfg, cmd)?,
        Some(Commands::Config(cmd)) => commands::config::run(&paths, &cfg, cmd)?,
        Some(Commands::Provider(cmd)) => commands::provider::run(&paths, &mut cfg, cmd).await?,
        Some(Commands::Model(cmd)) => commands::model::run(&paths, &mut cfg, cmd).await?,
        Some(Commands::Ask { prompt, model }) => commands::ask::run(&paths, &mut cfg, prompt, model).await?,
        Some(Commands::Chat { model }) => commands::chat::run(&paths, &mut cfg, model).await?,
        Some(Commands::Session(cmd)) => commands::session::run(&paths, cmd).await?,
        None => {
            clear_screen();
            println!("{}", startup_banner(&cfg, "not initialized"));
            println!();
            println!(
                "{}",
                hint_card(&[
                    "ask: mew ask \"what does this repo do?\"",
                    "provider: mew provider list",
                    "model: mew model list",
                ])
            );
        }
    }

    Ok(())
}

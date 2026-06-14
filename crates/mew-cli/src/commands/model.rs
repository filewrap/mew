use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::{ProviderModel, ProviderRegistry};
use mew_ui::kv_table;

use crate::args::{ModelCommand, ModelSubcommand};

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, cmd: ModelCommand) -> Result<()> {
    match cmd.command {
        ModelSubcommand::List {
            provider,
            remote,
            all,
        } => {
            let reg = ProviderRegistry::from_config(cfg);

            let models = if let Some(provider) = provider {
                if remote {
                    reg.list_remote_models_for(&provider).await?
                } else {
                    reg.list_models_for(&provider)?
                }
            } else if remote {
                let mut out = Vec::new();

                for p in reg.list_authorized_info() {
                    match reg.list_remote_models_for(&p.id).await {
                        Ok(mut models) => out.append(&mut models),
                        Err(err) => eprintln!("skip {}: {}", p.id, err),
                    }
                }

                out
            } else if all {
                reg.list_models()
            } else {
                let authorized = reg.list_authorized_info();
                let mut out = Vec::new();

                for p in authorized {
                    if let Ok(mut models) = reg.list_models_for(&p.id) {
                        out.append(&mut models);
                    }
                }

                out
            };

            if models.is_empty() {
                println!("no authorized provider models found");
                println!("try:");
                println!("  export OPENAI_API_KEY='...'");
                println!("  export OPENROUTER_API_KEY='...'");
                println!("  export GEMINI_API_KEY='...'");
                println!("  mew model list --all");
                println!("  mew model list openrouter --remote");
                return Ok(());
            }

            print_models(models);
        }
        ModelSubcommand::Use { model } => {
            let _ = ProviderRegistry::parse_model_ref(&model)?;
            cfg.providers.active_model = model.clone();
            cfg.save(paths)?;
            println!("{}", model);
        }
        ModelSubcommand::Show => {
            println!("{}", cfg.providers.active_model);
        }
    }

    Ok(())
}

fn print_models(models: Vec<ProviderModel>) {
    for m in models {
        println!(
            "{}",
            kv_table(
                &format!("{}/{}", m.provider, m.id),
                &[
                    (
                        "context",
                        if m.context == 0 {
                            "unknown".to_string()
                        } else {
                            m.context.to_string()
                        },
                    ),
                    ("tools", m.supports_tools.to_string()),
                    ("vision", m.supports_vision.to_string()),
                    ("notes", m.notes),
                ],
            )
        );
        println!();
    }
}

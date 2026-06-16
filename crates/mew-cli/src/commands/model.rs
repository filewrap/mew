use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Cell, Table};
use mew_common::{MewConfig, MewPaths};
use mew_provider::{ProviderModel, ProviderRegistry};

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
                    if let Ok(mut models) = reg.list_remote_models_for(&p.id).await {
                        out.append(&mut models);
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
    if models.is_empty() {
        println!("no models");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("#"),
        Cell::new("name"),
        Cell::new("id"),
    ]);

    for (idx, m) in models.iter().enumerate() {
        table.add_row(vec![
            Cell::new((idx + 1).to_string()),
            Cell::new(short_name(&m.id)),
            Cell::new(format!("{}/{}", m.provider, m.id)),
        ]);
    }

    println!("{}", table);
}

fn short_name(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).to_string()
}

use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::ProviderRegistry;
use mew_ui::kv_table;

use crate::args::{ModelCommand, ModelSubcommand};

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, cmd: ModelCommand) -> Result<()> {
    match cmd.command {
        ModelSubcommand::List => {
            let reg = ProviderRegistry::from_config(cfg);
            for m in reg.list_models() {
                println!(
                    "{}",
                    kv_table(
                        &format!("{}/{}", m.provider, m.id),
                        &[
                            ("context", m.context.to_string()),
                            ("tools", m.supports_tools.to_string()),
                            ("vision", m.supports_vision.to_string()),
                            ("notes", m.notes),
                        ],
                    )
                );
                println!();
            }
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

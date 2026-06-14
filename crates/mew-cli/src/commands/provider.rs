use anyhow::Result;
use mew_common::{CustomProviderConfig, MewConfig, MewPaths, ModelConfig};
use mew_provider::ProviderRegistry;
use mew_ui::kv_table;

use crate::args::{ProviderCommand, ProviderSubcommand};

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, cmd: ProviderCommand) -> Result<()> {
    match cmd.command {
        ProviderSubcommand::List => {
            let reg = ProviderRegistry::from_config(cfg);
            for p in reg.list_info() {
                println!(
                    "{}",
                    kv_table(
                        &p.id,
                        &[
                            ("name", p.name),
                            ("enabled", p.enabled.to_string()),
                            ("authorized", p.authorized.to_string()),
                            ("auth", p.auth),
                            ("base", p.base_url),
                            ("default", p.default_model),
                        ],
                    )
                );
                println!();
            }
        }
        ProviderSubcommand::Test { provider } => {
            let reg = ProviderRegistry::from_config(cfg);
            let p = reg.get(&provider)?;
            p.test().await?;
            println!("ok");
        }
        ProviderSubcommand::AddOpenai {
            id,
            base_url,
            api_key_env,
            model,
        } => {
            cfg.providers.custom.insert(
                id,
                CustomProviderConfig {
                    enabled: true,
                    kind: "openai-compatible".to_string(),
                    api_key_env,
                    base_url,
                    default_model: model.clone(),
                    models: vec![ModelConfig {
                        id: model,
                        context: 128_000,
                        supports_tools: true,
                        supports_vision: false,
                        notes: "custom OpenAI-compatible model".to_string(),
                    }],
                },
            );

            cfg.save(paths)?;
            println!("saved");
        }
    }

    Ok(())
}

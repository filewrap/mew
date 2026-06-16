use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_session::{list_sessions, load_session};
use mew_ui::kv_table;

use crate::args::{SessionCommand, SessionSubcommand};

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, cmd: SessionCommand) -> Result<()> {
    match cmd.command {
        SessionSubcommand::List => {
            let sessions = list_sessions(paths).await?;

            if sessions.is_empty() {
                println!("no sessions");
                return Ok(());
            }

            for s in sessions {
                println!(
                    "{}",
                    kv_table(
                        &s.id,
                        &[
                            ("title", s.title),
                            ("provider", s.provider),
                            ("model", s.model),
                            ("messages", s.messages.len().to_string()),
                            ("updated", s.updated_at.to_rfc3339()),
                        ],
                    )
                );
                println!();
            }
        }
        SessionSubcommand::Show { id } => {
            let s = load_session(paths, &id).await?;

            println!(
                "{}",
                kv_table(
                    &s.id,
                    &[
                        ("title", s.title.clone()),
                        ("provider", s.provider.clone()),
                        ("model", s.model.clone()),
                        ("messages", s.messages.len().to_string()),
                        ("created", s.created_at.to_rfc3339()),
                        ("updated", s.updated_at.to_rfc3339()),
                    ],
                )
            );

            println!();

            for msg in s.messages {
                println!("--- {}", msg.role);
                println!("{}", msg.content);
                println!();
            }
        }
        SessionSubcommand::Resume { id } => {
            crate::commands::chat::resume(paths, cfg, id).await?;
        }
    }

    Ok(())
}

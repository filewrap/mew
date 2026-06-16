use anyhow::{anyhow, Result};
use mew_common::{MewConfig, MewPaths};
use mew_ui::{
    code_block, diff_sample, hint_card, kv_table, phrase, spinner_frame, theme_exists, tool_card,
    THEMES,
};

use crate::args::{StyleCommand, StyleSubcommand};

pub fn run(paths: &MewPaths, cfg: &mut MewConfig, cmd: StyleCommand) -> Result<()> {
    match cmd.command {
        StyleSubcommand::List => {
            for theme in THEMES {
                println!("{:<22} {}", theme.name, theme.description);
            }
        }
        StyleSubcommand::Set { theme } => {
            if !theme_exists(&theme) {
                return Err(anyhow!("unknown theme: {}", theme));
            }

            cfg.style.theme = theme;
            cfg.save(paths)?;
            println!("saved");
        }
        StyleSubcommand::Preview => {
            println!(
                "{}",
                kv_table(
                    "style",
                    &[
                        ("theme", cfg.style.theme.clone()),
                        ("density", cfg.style.density.clone()),
                        ("emoji", cfg.style.emoji.to_string()),
                        ("kaomoji", cfg.style.kaomoji.to_string()),
                    ],
                )
            );

            println!();
            println!(
                "{}",
                hint_card(&["mew style set claude-minimal", "mew name set paww"])
            );
            println!();
            println!("{}", tool_card("fs.read", "src/main.rs", "safe"));
            println!();

            let spin = spinner_frame(3, "thinking");
            println!("{} {}", spin.frame, spin.text);
            println!("{}", phrase("token"));
            println!("{}", phrase("council"));

            println!();
            println!(
                "{}",
                code_block("rust", "fn main() {\n    println!(\"mew~\");\n}")
            );
            println!();
            println!("{}", diff_sample());
        }
    }

    Ok(())
}

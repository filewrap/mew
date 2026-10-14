use crate::args::{SkillsCommand, SkillsSubcommand};
use anyhow::Result;
use mew_common::MewConfig;
use mew_ui::{status_line, theme_by_name};
use std::path::Path;

pub fn run(root: &Path, cfg: &MewConfig, cmd: SkillsCommand) -> Result<()> {
    let theme = theme_by_name(&cfg.style.theme);
    match cmd.command {
        SkillsSubcommand::List => {
            for skill in mew_skills::list(root)? {
                println!(
                    "{} {:<18} {} [{}]",
                    status_line(&theme, "•"),
                    skill.name,
                    skill.description,
                    skill.source
                );
            }
            println!("\nWorkspace skills live in .mew/skills/<name>.md and can be loaded with --skill <name>.");
        }
        SkillsSubcommand::Show { name } => {
            let skill = mew_skills::load(root, &name)?;
            println!(
                "# {}\n\n{}\n\n{}",
                skill.name, skill.description, skill.body
            );
        }
    }
    Ok(())
}

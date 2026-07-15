use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_index::{build_index, render_repo_map, save_index, sniff};
use mew_ui::{kv_table, phrase};
use std::fs;

pub fn run(_paths: &MewPaths, _cfg: &MewConfig, dry_run: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let info = sniff(&cwd)?;

    println!("{}", phrase("scanning"));
    println!();

    println!(
        "{}",
        kv_table(
            "mew init",
            &[
                ("path", cwd.display().to_string()),
                ("git", info.git.to_string()),
                ("file count", info.file_count.to_string()),
                ("language", info.languages.join(", ")),
                ("package", info.package.clone()),
                ("instructions", info.instructions.join(", ")),
                ("dry run", dry_run.to_string()),
            ],
        )
    );

    if !dry_run {
        let mew_dir = cwd.join(".mew");
        fs::create_dir_all(&mew_dir)?;

        fs::write(
            mew_dir.join("project.toml"),
            format!(
                "path = {:?}\nlanguage = {:?}\npackage = {:?}\n",
                cwd.display().to_string(),
                info.languages.join(", "),
                info.package
            ),
        )?;

        let memory_path = mew_dir.join("memory.md");
        if !memory_path.exists() {
            fs::write(memory_path, "# mew memory\n\n")?;
        }

        fs::write(mew_dir.join("repo-map.md"), render_repo_map(&info))?;

        save_index(&cwd, &build_index(&cwd)?)?;

        println!();
        println!("saved .mew/");
    }

    Ok(())
}

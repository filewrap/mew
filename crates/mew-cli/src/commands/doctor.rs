use anyhow::Result;
use mew_common::{arch_name, is_termux, os_name, MewConfig, MewPaths};
use mew_ui::kv_table;
use std::process::Command;

pub fn run(paths: &MewPaths, cfg: &MewConfig) -> Result<()> {
    let rows = vec![
        ("os", os_name().to_string()),
        ("arch", arch_name().to_string()),
        ("termux", is_termux().to_string()),
        ("config", paths.config_file.display().to_string()),
        ("data", paths.data_dir.display().to_string()),
        ("cache", paths.cache_dir.display().to_string()),
        ("display name", cfg.identity.display_name.clone()),
        ("theme", cfg.style.theme.clone()),
        ("default provider", cfg.providers.default.clone()),
        ("active model", cfg.providers.active_model.clone()),
        ("git", found("git")),
        ("rg", found("rg")),
        ("curl", found("curl")),
    ];

    println!("{}", kv_table("doctor", &rows));
    Ok(())
}

fn found(cmd: &str) -> String {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| {
            if o.status.success() {
                "found"
            } else {
                "missing"
            }
            .to_string()
        })
        .unwrap_or_else(|_| "missing".to_string())
}

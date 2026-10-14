use crate::args::HooksSubcommand;
use anyhow::{bail, Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const TEMPLATE: &str = "# Project lifecycle hooks for Mew. Hooks do not run until enabled is set true.\n# Commands are executable/argument arrays (no shell parsing) and receive one JSON payload on stdin.\n# Events: session_start, user_prompt_submit, pre_tool_use, post_tool_use, session_end\nenabled = false\ntimeout_seconds = 10\n\n# Example (uncomment and customize):\n# [[hooks]]\n# event = \"pre_tool_use\"\n# command = [\"python3\", \".mew/check_tool.py\"]\n# blocking = true\n";

pub fn run(root: &Path, command: HooksSubcommand) -> Result<()> {
    match command {
        HooksSubcommand::Init => println!("Created {}", init(root)?.display()),
        HooksSubcommand::Status => {
            let (enabled, count) = mew_tools::hooks::validate_hook_file(root)?;
            println!(
                "Hooks are {} · {count} configured · {}",
                if enabled { "enabled" } else { "disabled" },
                mew_tools::hooks::hooks_path(root).display()
            );
        }
        HooksSubcommand::Validate => {
            let (enabled, count) = mew_tools::hooks::validate_hook_file(root)?;
            println!("Valid hook configuration · enabled={enabled} · {count} hooks");
        }
    }
    Ok(())
}

pub fn init(root: &Path) -> Result<PathBuf> {
    let directory = root.join(".mew");
    if fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing to create hooks through a .mew symlink");
    }
    fs::create_dir_all(&directory)?;
    let path = mew_tools::hooks::hooks_path(root);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("create {} without overwriting", path.display()))?;
    file.write_all(TEMPLATE.as_bytes())?;
    file.sync_all()?;
    Ok(path)
}

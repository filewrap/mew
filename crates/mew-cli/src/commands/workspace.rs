use anyhow::{bail, Context, Result};
use mew_common::{MewConfig, MewPaths};
use std::io::{self, IsTerminal, Write};
use std::path::Path;

/// Require an explicit yes before Mew reads project files or enables workspace
/// tools for a canonical working directory. Trust is scoped to this exact path.
pub fn ensure_workspace_trusted(
    paths: &MewPaths,
    cfg: &mut MewConfig,
    canonical_root: &Path,
) -> Result<()> {
    if cfg.workspace_is_trusted(canonical_root) {
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(
            "workspace {} is not trusted; run `mew` in an interactive terminal to review and trust it",
            canonical_root.display()
        );
    }

    println!();
    println!("workspace access");
    println!("Mew can inspect project files and offer workspace tools in this folder:");
    println!("  {}", canonical_root.display());
    print!("Trust this folder? [y/N] ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("read workspace trust decision")?;
    if !trust_was_granted(&answer) {
        bail!(
            "workspace access was not granted for {}; no project tools were enabled",
            canonical_root.display()
        );
    }

    cfg.trust_workspace(canonical_root);
    cfg.save(paths).context("save workspace trust decision")?;
    println!("workspace trusted · action approvals still apply");
    Ok(())
}

fn trust_was_granted(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::trust_was_granted;

    #[test]
    fn workspace_trust_requires_an_explicit_yes() {
        assert!(!trust_was_granted(""));
        assert!(!trust_was_granted("n"));
        assert!(!trust_was_granted("no"));
        assert!(trust_was_granted("y"));
        assert!(trust_was_granted(" YES "));
    }
}

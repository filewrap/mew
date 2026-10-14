use crate::args::WorktreeSubcommand;
use anyhow::{anyhow, bail, Context, Result};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn run(root: &Path, command: WorktreeSubcommand) -> Result<()> {
    match command {
        WorktreeSubcommand::List => println!("{}", list_worktrees(root)?),
        WorktreeSubcommand::Create { branch, path } => {
            let path = create_worktree(root, &branch, path.as_deref())?;
            println!("Created worktree at {}", path.display());
            println!(
                "Start a separate session with `cd {}` then `mew`.",
                path.display()
            );
        }
        WorktreeSubcommand::Remove { path } => remove_worktree(root, &path)?,
    }
    Ok(())
}

pub fn list_worktrees(root: &Path) -> Result<String> {
    let output = git(root, &["worktree", "list", "--porcelain"])?;
    if !output.status.success() {
        bail!(
            "git worktree list failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut rows = Vec::new();
    for block in text.split("\n\n").filter(|block| !block.trim().is_empty()) {
        let path = block
            .lines()
            .find_map(|line| line.strip_prefix("worktree "))
            .unwrap_or("?");
        let branch = block
            .lines()
            .find_map(|line| line.strip_prefix("branch refs/heads/"))
            .unwrap_or("detached");
        let locked = block.lines().any(|line| line == "locked");
        rows.push(format!(
            "{path} · {branch}{}",
            if locked { " · locked" } else { "" }
        ));
    }
    if rows.is_empty() {
        Ok("No Git worktrees found.".into())
    } else {
        Ok(rows.join("\n"))
    }
}

pub fn create_worktree(
    root: &Path,
    branch: &str,
    requested_path: Option<&Path>,
) -> Result<PathBuf> {
    if branch.trim().is_empty() {
        bail!("branch name must not be empty");
    }
    let validation = Command::new("git")
        .current_dir(root)
        .args(["check-ref-format", "--branch", branch])
        .output()
        .context("validate Git branch name")?;
    if !validation.status.success() {
        bail!("invalid Git branch name {branch:?}");
    }
    let registered = worktree_paths(root)?;
    let primary = registered
        .first()
        .ok_or_else(|| anyhow!("Git repository has no worktrees"))?;
    let allowed_parent = primary
        .parent()
        .ok_or_else(|| anyhow!("primary worktree has no parent directory"))?;
    let proposed = if let Some(path) = requested_path {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        }
    } else {
        let workspace_name = primary
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("workspace");
        primary
            .parent()
            .unwrap()
            .join(format!("{workspace_name}-wt-{}", branch_slug(branch)))
    };
    let parent = proposed
        .parent()
        .ok_or_else(|| anyhow!("worktree path must have a parent directory"))?;
    let mut existing_parent = parent;
    while !existing_parent.exists() {
        existing_parent = existing_parent
            .parent()
            .ok_or_else(|| anyhow!("worktree path has no existing parent"))?;
    }
    let existing_parent = existing_parent
        .canonicalize()
        .context("canonicalize existing worktree parent")?;
    if !existing_parent.starts_with(allowed_parent) {
        bail!("worktree destination must be beneath the primary repository's parent directory");
    }
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create parent for {}", proposed.display()))?;
    let parent = parent
        .canonicalize()
        .context("canonicalize worktree parent")?;
    if !parent.starts_with(allowed_parent) {
        bail!("worktree destination must be beneath the primary repository's parent directory");
    }
    let name = proposed
        .file_name()
        .ok_or_else(|| anyhow!("worktree path needs a final directory name"))?;
    let target = parent.join(name);
    if registered
        .iter()
        .any(|worktree| target.starts_with(worktree))
    {
        bail!("worktree destination must be outside every existing worktree");
    }
    if std::fs::symlink_metadata(&target).is_ok() {
        bail!("worktree destination already exists: {}", target.display());
    }
    let target_text = target.to_string_lossy().to_string();
    let branch_ref = format!("refs/heads/{branch}");
    let branch_exists = git(root, &["show-ref", "--verify", "--quiet", &branch_ref])?
        .status
        .success();
    let output = if branch_exists {
        git(root, &["worktree", "add", &target_text, branch])?
    } else {
        git(
            root,
            &["worktree", "add", "-b", branch, &target_text, "HEAD"],
        )?
    };
    if !output.status.success() {
        bail!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(target)
}

pub fn remove_worktree(root: &Path, path: &Path) -> Result<()> {
    let root = root.canonicalize()?;
    let registered = worktree_paths(&root)?;
    let primary = registered
        .first()
        .ok_or_else(|| anyhow!("Git repository has no worktrees"))?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let target = candidate
        .canonicalize()
        .with_context(|| format!("resolve worktree {}", candidate.display()))?;
    if &target == primary {
        bail!("refusing to remove the primary worktree");
    }
    if !registered.iter().any(|candidate| candidate == &target) {
        bail!(
            "{} is not a worktree in this Git repository",
            target.display()
        );
    }
    let status = Command::new("git")
        .current_dir(&target)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .context("check worktree cleanliness")?;
    if !status.status.success() {
        bail!(
            "could not inspect worktree status: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        );
    }
    if !status.stdout.is_empty() {
        bail!("worktree is not clean; refusing removal so uncommitted and untracked files are preserved:\n{}", String::from_utf8_lossy(&status.stdout));
    }
    let ignored = Command::new("git")
        .current_dir(&target)
        .args(["clean", "-ndx"])
        .output()
        .context("inspect ignored worktree files")?;
    if !ignored.status.success() {
        bail!(
            "could not inspect ignored worktree files: {}",
            String::from_utf8_lossy(&ignored.stderr).trim()
        );
    }
    if !ignored.stdout.is_empty() {
        bail!(
            "worktree contains ignored files; refusing removal so they are preserved:\n{}",
            String::from_utf8_lossy(&ignored.stdout)
        );
    }
    if !io::stdin().is_terminal() {
        bail!("removal deletes the worktree directory; run this command in a terminal to confirm");
    }
    print!(
        "Remove the clean worktree directory at {}? This deletes its files. [y/N] ",
        target.display()
    );
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        println!("Removal cancelled.");
        return Ok(());
    }
    let target_text = target.to_string_lossy().to_string();
    let output = git(&root, &["worktree", "remove", "--", &target_text])?;
    if !output.status.success() {
        bail!(
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    println!("Removed worktree {}", target.display());
    Ok(())
}

fn worktree_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let output = git(root, &["worktree", "list", "--porcelain"])?;
    if !output.status.success() {
        bail!(
            "git worktree list failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .filter_map(|path| Path::new(path).canonicalize().ok())
        .collect())
}

fn git(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))
}

fn branch_slug(branch: &str) -> String {
    let slug = branch
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if slug.is_empty() {
        "branch".into()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn creates_new_and_existing_branch_worktrees_and_rejects_unsafe_paths() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "mew-worktree-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let run = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {:?}: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run(&["init", "--quiet"]);
        run(&["config", "user.name", "Mew Test"]);
        run(&["config", "user.email", "mew-test@example.invalid"]);
        std::fs::write(root.join("README.txt"), "test repository\n").unwrap();
        std::fs::write(root.join(".gitignore"), "ignored*\n").unwrap();
        run(&["add", "README.txt", ".gitignore"]);
        run(&["commit", "--quiet", "-m", "init"]);
        run(&["branch", "feature/existing"]);

        let created = create_worktree(&root, "feature/new", None).unwrap();
        let existing = root.parent().unwrap().join(format!(
            "{}-wt-existing-{}",
            root.file_name().unwrap().to_string_lossy(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let attached = create_worktree(&root, "feature/existing", Some(&existing)).unwrap();
        assert!(created.join("README.txt").exists());
        assert!(attached.join("README.txt").exists());
        assert!(remove_worktree(&root, &root)
            .unwrap_err()
            .to_string()
            .contains("primary"));
        std::fs::write(attached.join("ignored-cache"), "preserve me").unwrap();
        assert!(remove_worktree(&root, &attached)
            .unwrap_err()
            .to_string()
            .contains("ignored files"));
        std::fs::remove_file(attached.join("ignored-cache")).unwrap();
        assert!(create_worktree(
            &root,
            "feature/escape",
            Some(Path::new("/mew-worktree-outside"))
        )
        .is_err());
        assert!(list_worktrees(&root).unwrap().contains("feature/new"));

        std::fs::remove_dir_all(&created).unwrap();
        std::fs::remove_dir_all(&attached).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn branch_slug_is_path_safe_and_readable() {
        assert_eq!(branch_slug("feature/search-ui"), "feature-search-ui");
        assert_eq!(branch_slug("/"), "branch");
    }
}

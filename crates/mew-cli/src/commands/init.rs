use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_ui::{kv_table, phrase};
use std::fs;
use std::path::{Path, PathBuf};

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
                ("language", info.language.clone()),
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
                info.language,
                info.package
            ),
        )?;

        let memory_path = mew_dir.join("memory.md");
        if !memory_path.exists() {
            fs::write(memory_path, "# mew memory\n\n")?;
        }

        fs::write(
            mew_dir.join("repo-map.md"),
            format!(
                "# repo map\n\n- path: `{}`\n- language: `{}`\n- package: `{}`\n- files: `{}`\n",
                cwd.display(),
                info.language,
                info.package,
                info.file_count
            ),
        )?;

        println!();
        println!("saved .mew/");
    }

    Ok(())
}

struct SniffInfo {
    git: bool,
    file_count: usize,
    language: String,
    package: String,
    instructions: Vec<String>,
}

fn sniff(path: &Path) -> Result<SniffInfo> {
    let git = path.join(".git").exists();

    let mut files = Vec::new();
    collect(path, &mut files, 0)?;

    let language = detect_language(&files);
    let package = detect_package(path);
    let instructions = detect_instructions(path);

    Ok(SniffInfo {
        git,
        file_count: files.len(),
        language,
        package,
        instructions,
    })
}

fn collect(path: &Path, files: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 4 {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");

        if matches!(
            name,
            ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".mew" | ".cache"
        ) {
            continue;
        }

        if p.is_dir() {
            collect(&p, files, depth + 1)?;
        } else {
            files.push(p);
        }
    }

    Ok(())
}

fn detect_language(files: &[PathBuf]) -> String {
    let has_rs = files
        .iter()
        .any(|p| p.extension().map(|e| e == "rs").unwrap_or(false));
    let has_ts = files
        .iter()
        .any(|p| p.extension().map(|e| e == "ts").unwrap_or(false));
    let has_js = files
        .iter()
        .any(|p| p.extension().map(|e| e == "js").unwrap_or(false));
    let has_py = files
        .iter()
        .any(|p| p.extension().map(|e| e == "py").unwrap_or(false));
    let has_go = files
        .iter()
        .any(|p| p.extension().map(|e| e == "go").unwrap_or(false));

    if has_rs {
        "Rust".to_string()
    } else if has_ts {
        "TypeScript".to_string()
    } else if has_js {
        "JavaScript".to_string()
    } else if has_py {
        "Python".to_string()
    } else if has_go {
        "Go".to_string()
    } else {
        "unknown".to_string()
    }
}

fn detect_package(path: &Path) -> String {
    if path.join("Cargo.toml").exists() {
        "cargo".to_string()
    } else if path.join("pnpm-lock.yaml").exists() {
        "pnpm".to_string()
    } else if path.join("yarn.lock").exists() {
        "yarn".to_string()
    } else if path.join("package-lock.json").exists() || path.join("package.json").exists() {
        "npm".to_string()
    } else if path.join("pyproject.toml").exists() {
        "python".to_string()
    } else if path.join("go.mod").exists() {
        "go".to_string()
    } else {
        "unknown".to_string()
    }
}

fn detect_instructions(path: &Path) -> Vec<String> {
    let names = [
        "AGENT.md",
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        "OPENAI.md",
        "CODEX.md",
        ".windsurfrules",
    ];

    let mut found = Vec::new();

    for name in names {
        if path.join(name).exists() {
            found.push(name.to_string());
        }
    }

    if path
        .join(".github")
        .join("copilot-instructions.md")
        .exists()
    {
        found.push(".github/copilot-instructions.md".to_string());
    }

    if path.join(".cursor").join("rules").exists() {
        found.push(".cursor/rules".to_string());
    }

    if found.is_empty() {
        found.push("none".to_string());
    }

    found
}

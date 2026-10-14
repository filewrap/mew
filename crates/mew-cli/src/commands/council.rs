use anyhow::{anyhow, Context, Result};
use mew_common::MewConfig;
use mew_provider::ProviderRegistry;
use mew_ui::{meta_line, status_line, theme_by_name};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use uuid::Uuid;

pub async fn run(
    root: &Path,
    cfg: &MewConfig,
    prompt: String,
    model: Option<String>,
) -> Result<()> {
    let model_ref = model.unwrap_or_else(|| cfg.providers.active_model.clone());
    let (provider_id, model_id) = ProviderRegistry::parse_model_ref(&model_ref)?;
    let registry = ProviderRegistry::from_config(cfg);
    let provider = registry.get(&provider_id)?;
    if !provider.is_authorized() {
        return Err(anyhow!("provider {provider_id} is not authorized"));
    }
    let context = repository_context(root).await?;
    let context_tokens = provider
        .models()
        .into_iter()
        .find(|m| m.id == model_id)
        .map(|m| m.context)
        .filter(|n| *n > 0)
        .unwrap_or(32_000);
    let theme = theme_by_name(&cfg.style.theme);
    println!(
        "{}",
        status_line(
            &theme,
            &format!("council · {model_ref} · planner → coder → reviewer")
        )
    );
    let start = Instant::now();
    let transcript =
        mew_council::deliberate(provider, &model_id, &prompt, &context, context_tokens).await?;
    for turn in &transcript.turns {
        println!("\n## {}\n\n{}", turn.role, turn.content);
    }
    let dir = ensure_council_dir(root)?;
    let path = dir.join(format!("council-{}.md", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("create transcript {}", path.display()))?;
    file.write_all(transcript.to_markdown(&prompt, &model_ref).as_bytes())
        .with_context(|| format!("write transcript {}", path.display()))?;
    println!(
        "\n{}",
        meta_line(
            &theme,
            &format!(
                "transcript={} elapsed={}ms tokens=in:{} out:{}",
                path.strip_prefix(root).unwrap_or(&path).display(),
                start.elapsed().as_millis(),
                transcript
                    .input_tokens
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into()),
                transcript
                    .output_tokens
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into())
            )
        )
    );
    Ok(())
}

async fn repository_context(root: &Path) -> Result<String> {
    let mut sections = vec![format!("Workspace: {}", root.display())];
    for (name, args) in [
        ("git status", serde_json::json!({})),
        ("git diff", serde_json::json!({})),
    ] {
        if let Ok(out) =
            mew_tools::execute_tool(root, &name.replace(' ', "_"), &args, Default::default()).await
        {
            sections.push(format!("### {name}\n{}", cap(&out, 8_000)));
        }
    }
    if let Ok(info) = mew_index::sniff(root) {
        sections.push(format!(
            "### Project snapshot\nlanguages: {}\npackage: {}\nfiles: {}\ninstructions: {}\n",
            info.languages.join(", "),
            info.package,
            info.file_count,
            info.instructions.join(", ")
        ));
    }
    Ok(sections.join("\n\n"))
}

fn ensure_council_dir(root: &Path) -> Result<PathBuf> {
    let mew_dir = root.join(".mew");
    if let Ok(meta) = fs::symlink_metadata(&mew_dir) {
        if meta.file_type().is_symlink() {
            return Err(anyhow!(
                "refusing to write transcript through a .mew symlink"
            ));
        }
    }
    fs::create_dir_all(&mew_dir)?;
    let canonical_root = fs::canonicalize(root)?;
    let canonical_mew = fs::canonicalize(&mew_dir)?;
    if !canonical_mew.starts_with(&canonical_root) {
        return Err(anyhow!(".mew resolves outside the workspace"));
    }
    let dir = canonical_mew.join("council");
    if fs::symlink_metadata(&dir).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(anyhow!(
            "refusing to write transcript through a council symlink"
        ));
    }
    fs::create_dir_all(&dir)?;
    let canonical_dir = fs::canonicalize(&dir)?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err(anyhow!(
            "council export directory resolves outside workspace"
        ));
    }
    Ok(canonical_dir)
}

fn cap(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[truncated]", &s[..end])
}

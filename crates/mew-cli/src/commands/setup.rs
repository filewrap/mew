use anyhow::{bail, Context, Result};
use mew_common::{save_credential, MewConfig, MewPaths, ModelConfig};
use mew_provider::{OpenAiCompatibleProvider, ProviderModel};
use mew_ui::{
    clear_screen, miku_pixel_cat, read_searchable_selector, theme_by_name, MikuMotion,
    SelectOption, Theme,
};
use reqwest::Client;
use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::Duration;

const MODELS_TO_SHOW: usize = 12;

#[derive(Clone, Copy)]
enum BuiltinProvider {
    OpenRouter,
    OpenAi,
}

impl BuiltinProvider {
    fn from_choice(choice: &str) -> Option<Self> {
        match choice.trim() {
            "" | "1" | "openrouter" => Some(Self::OpenRouter),
            "2" | "openai" => Some(Self::OpenAi),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::OpenAi => "openai",
        }
    }

    fn env_name(self, cfg: &MewConfig) -> &str {
        match self {
            Self::OpenRouter => &cfg.providers.openrouter.api_key_env,
            Self::OpenAi => &cfg.providers.openai.api_key_env,
        }
    }

    fn base_url(self, cfg: &MewConfig) -> &str {
        match self {
            Self::OpenRouter => &cfg.providers.openrouter.base_url,
            Self::OpenAi => &cfg.providers.openai.base_url,
        }
    }

    fn preferred_model(self, cfg: &MewConfig) -> &str {
        match self {
            Self::OpenRouter => &cfg.providers.openrouter.default_model,
            Self::OpenAi => &cfg.providers.openai.default_model,
        }
    }
}

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig) -> Result<()> {
    if !io::stdin().is_terminal() {
        bail!("setup needs an interactive terminal; run `mew setup` in your terminal");
    }

    clear_screen();
    let theme = theme_by_name(&cfg.style.theme);
    println!("{}", theme.primary("Miku · mew setup"));
    println!("{}", theme.accent("your friendly coding cat"));
    for row in miku_pixel_cat(&theme, MikuMotion::Idle, 0) {
        println!("{}", theme.accent(row));
    }
    println!("one quick setup, then `mew` opens chat");
    println!();
    println!("Startup checks");
    println!("  ✓ mew {}", env!("CARGO_PKG_VERSION"));

    check_config_writable(paths)?;
    println!("  ✓ settings folder is writable");

    let initial_dir = std::env::current_dir()
        .context("find the current directory")?
        .canonicalize()
        .context("resolve the current directory")?;
    crate::commands::workspace::ensure_workspace_trusted(paths, cfg, &initial_dir)?;
    verify_workspace(&initial_dir)?;
    println!(
        "  ✓ current directory is accessible: {}",
        initial_dir.display()
    );

    match check_for_updates().await {
        Ok(UpdateStatus::Current) => println!("  ✓ version check: up to date"),
        Ok(UpdateStatus::Available(version)) => {
            println!("  ! update available: {version} (setup will not update files automatically)")
        }
        Err(_) => println!("  - version check skipped (offline or release service unavailable)"),
    }
    println!();

    println!();

    println!("Connect a model provider");
    let provider = if io::stdout().is_terminal() {
        let options = [
            SelectOption::new(
                "OpenRouter",
                "many models · one API key · recommended",
                "openrouter",
            ),
            SelectOption::new("OpenAI", "OpenAI models · API key", "openai"),
        ];
        match read_searchable_selector(&theme, "Choose a provider", &options)? {
            Some(choice) => BuiltinProvider::from_choice(&choice)
                .context("the selected provider is not supported")?,
            None => bail!("provider selection was cancelled; no changes were saved"),
        }
    } else {
        println!("  1) OpenRouter (recommended — many models, one API key)");
        println!("  2) OpenAI");
        loop {
            let choice = prompt_line("Choose provider [1]: ")?;
            if let Some(provider) = BuiltinProvider::from_choice(&choice) {
                break provider;
            }
            println!("Choose 1 for OpenRouter or 2 for OpenAI.");
        }
    };

    let env_name = provider.env_name(cfg).to_string();
    let base_url = provider.base_url(cfg).to_string();
    let key_from_environment = std::env::var(&env_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let mut use_environment = key_from_environment.is_some();
    if use_environment {
        println!("Using the existing {env_name} environment variable.");
    }

    let client = OpenAiCompatibleProvider {
        id: provider.id().to_string(),
        api_key_env: env_name.clone(),
        base_url,
        default_model: provider.preferred_model(cfg).to_string(),
        models: Vec::new(),
        app_title: "mew".to_string(),
        app_url: "https://github.com/filewrap/mew".to_string(),
    };

    let (api_key, models) = loop {
        let candidate = if use_environment {
            key_from_environment
                .clone()
                .expect("environment key was checked")
        } else {
            rpassword::prompt_password(format!("{env_name} (input hidden): "))
                .context("read API key")?
                .trim()
                .to_string()
        };
        if candidate.is_empty() {
            println!("The API key cannot be empty.");
            use_environment = false;
            continue;
        }

        println!("Checking the key and loading available models…");
        match client.list_remote_models_with_key(&candidate).await {
            Ok(models) => {
                let models = models.into_iter().filter(is_chat_model).collect::<Vec<_>>();
                if models.is_empty() {
                    bail!("the provider key worked, but the provider returned no chat models");
                }
                println!(
                    "  ✓ connection validated; found {} chat models",
                    models.len()
                );
                break (candidate, models);
            }
            Err(error) => {
                println!("  ✗ connection failed: {error:#}");
                if !yes_no("Try a different API key? [Y/n] ", true)? {
                    bail!("provider setup was not completed; no changes were saved");
                }
                use_environment = false;
            }
        }
    };

    let model = choose_model(&models, provider.preferred_model(cfg), &theme)?;
    persist_provider(
        paths,
        cfg,
        provider,
        &env_name,
        &model,
        &api_key,
        use_environment,
    )?;

    println!();
    println!("Setup complete.");
    println!("  Provider: {}", provider.id());
    println!("  Model:    {}", model.id);
    println!("  Folder:   {}", initial_dir.display());
    println!();
    println!("Run `mew` or `mew chat` to start chatting.");
    println!("Run `mew setup` any time to change providers or models.");
    Ok(())
}

fn check_config_writable(paths: &MewPaths) -> Result<()> {
    fs::create_dir_all(&paths.config_dir)
        .with_context(|| format!("create settings folder {}", paths.config_dir.display()))?;
    let probe = paths
        .config_dir
        .join(format!(".mew-setup-check-{}", std::process::id()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .with_context(|| {
            format!(
                "settings folder is not writable: {}",
                paths.config_dir.display()
            )
        })?;
    drop(file);
    fs::remove_file(probe).context("remove setup check file")?;
    Ok(())
}

fn verify_workspace(path: &Path) -> Result<()> {
    if !path.is_dir() {
        bail!("current directory is not a folder: {}", path.display());
    }
    fs::read_dir(path).with_context(|| format!("read current directory {}", path.display()))?;
    Ok(())
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().to_string())
}

fn yes_no(prompt: &str, default: bool) -> Result<bool> {
    let answer = prompt_line(prompt)?.to_ascii_lowercase();
    match answer.as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        "" => Ok(default),
        _ => {
            println!("Please answer yes or no.");
            yes_no(prompt, default)
        }
    }
}

fn prioritize_tool_capable_model(models: &mut [ProviderModel], preferred: &str) {
    let preferred_tool_model = models
        .iter()
        .position(|model| model.id == preferred && model.supports_tools);
    let first_tool_model = models.iter().position(|model| model.supports_tools);
    if let Some(index) = preferred_tool_model.or(first_tool_model) {
        models.swap(0, index);
    } else if let Some(index) = models.iter().position(|model| model.id == preferred) {
        models.swap(0, index);
    }
}

fn choose_model(models: &[ProviderModel], preferred: &str, theme: &Theme) -> Result<ProviderModel> {
    let mut ordered = models.to_vec();
    prioritize_tool_capable_model(&mut ordered, preferred);
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        let default_model = ordered
            .first()
            .map(|model| model.id.as_str())
            .unwrap_or(preferred);
        let options = ordered
            .iter()
            .map(|model| {
                SelectOption::new(
                    if model.id == default_model {
                        format!("● {}", model.id)
                    } else {
                        model.id.clone()
                    },
                    format!(
                        "{} context · {}",
                        if model.context > 0 {
                            format!("{}k", model.context / 1_000)
                        } else {
                            "context unknown".to_string()
                        },
                        if model.supports_tools {
                            "native tools"
                        } else {
                            "no native tools"
                        }
                    ),
                    model.id.clone(),
                )
            })
            .collect::<Vec<_>>();
        if let Some(selected) =
            read_searchable_selector(theme, "Choose a model · type to search", &options)?
        {
            return ordered
                .into_iter()
                .find(|model| model.id == selected)
                .context("the selected model is no longer in the provider catalog");
        }
        if io::stdout().is_terminal() {
            bail!("model selection was cancelled; no changes were saved");
        }
    }
    let mut visible = ordered
        .iter()
        .take(MODELS_TO_SHOW)
        .cloned()
        .collect::<Vec<_>>();
    println!();
    println!("Choose a model. Type a number, exact model ID, or search term:");
    print_model_choices(&visible, ordered.len());
    loop {
        let answer = prompt_line("Model [1]: ")?;
        if answer.is_empty() {
            return Ok(ordered[0].clone());
        }
        if let Ok(index) = answer.parse::<usize>() {
            if let Some(model) = index.checked_sub(1).and_then(|i| visible.get(i)) {
                return Ok(model.clone());
            }
        }
        if let Some(model) = ordered.iter().find(|model| model.id == answer) {
            return Ok(model.clone());
        }
        let query = answer.to_ascii_lowercase();
        let matches = ordered
            .iter()
            .filter(|model| model.id.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        if matches.is_empty() {
            println!("No model matches that search. Try another term or model ID.");
            continue;
        }
        let count = matches.len();
        visible = matches.into_iter().take(MODELS_TO_SHOW).collect();
        println!("Matching models ({count}):");
        print_model_choices(&visible, count);
    }
}

fn print_model_choices(models: &[ProviderModel], total: usize) {
    for (index, model) in models.iter().enumerate() {
        let marker = if index == 0 { " (default)" } else { "" };
        println!("  {}) {}{}", index + 1, model.id, marker);
    }
    if total > models.len() {
        println!(
            "  … {} more; narrow the list by typing a search term",
            total - models.len()
        );
    }
}

fn is_chat_model(model: &ProviderModel) -> bool {
    let id = model.id.to_ascii_lowercase();
    ![
        "embedding",
        "whisper",
        "tts",
        "text-to-speech",
        "dall-e",
        "image",
        "moderation",
        "transcription",
        "rerank",
        "guard",
        "audio",
    ]
    .iter()
    .any(|kind| id.contains(kind))
}

fn persist_provider(
    paths: &MewPaths,
    cfg: &mut MewConfig,
    provider: BuiltinProvider,
    env_name: &str,
    selected_model: &ProviderModel,
    api_key: &str,
    key_from_environment: bool,
) -> Result<()> {
    if !key_from_environment {
        save_credential(paths, env_name, api_key)?;
    }

    let model_config = ModelConfig {
        id: selected_model.id.clone(),
        context: selected_model.context,
        supports_tools: selected_model.supports_tools,
        supports_vision: selected_model.supports_vision,
        notes: "discovered from the provider during setup".to_string(),
    };
    match provider {
        BuiltinProvider::OpenRouter => {
            cfg.providers.openrouter.enabled = true;
            cfg.providers.openrouter.default_model = selected_model.id.clone();
            cfg.providers.openrouter.models = vec![model_config];
        }
        BuiltinProvider::OpenAi => {
            cfg.providers.openai.enabled = true;
            cfg.providers.openai.default_model = selected_model.id.clone();
            cfg.providers.openai.models = vec![model_config];
        }
    }
    cfg.providers.default = provider.id().to_string();
    cfg.providers.default_model = selected_model.id.clone();
    cfg.providers.active_model = format!("{}/{}", provider.id(), selected_model.id);
    cfg.setup_complete = true;
    cfg.save(paths).context("save setup configuration")
}

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: String,
}

enum UpdateStatus {
    Current,
    Available(String),
}

async fn check_for_updates() -> Result<UpdateStatus> {
    let client = Client::builder().timeout(Duration::from_secs(3)).build()?;
    let response = client
        .get("https://api.github.com/repos/filewrap/mew/releases/latest")
        .header(reqwest::header::USER_AGENT, "mew-setup")
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("release check returned HTTP {}", response.status());
    }
    let release: LatestRelease = response.json().await?;
    let latest = release.tag_name.trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");
    if compare_versions(latest, current).is_some_and(|order| order.is_gt()) {
        Ok(UpdateStatus::Available(release.tag_name))
    } else {
        Ok(UpdateStatus::Current)
    }
}

fn compare_versions(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    fn parts(value: &str) -> Option<Vec<u64>> {
        value
            .split('.')
            .map(|part| {
                let digits = part
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>();
                digits.parse().ok()
            })
            .collect()
    }
    let left = parts(left)?;
    let right = parts(right)?;
    let len = left.len().max(right.len());
    for index in 0..len {
        match left
            .get(index)
            .copied()
            .unwrap_or(0)
            .cmp(&right.get(index).copied().unwrap_or(0))
        {
            std::cmp::Ordering::Equal => {}
            order => return Some(order),
        }
    }
    Some(std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str) -> ProviderModel {
        ProviderModel {
            provider: "openrouter".into(),
            id: id.into(),
            context: 0,
            supports_tools: false,
            supports_vision: false,
            notes: String::new(),
        }
    }

    #[test]
    fn setup_defaults_to_a_tool_capable_model_when_available() {
        let mut models = vec![model("chat-model"), model("tool-model")];
        models[1].supports_tools = true;

        prioritize_tool_capable_model(&mut models, "chat-model");

        assert_eq!(models[0].id, "tool-model");
        assert!(models[0].supports_tools);
    }

    #[test]
    fn filters_non_chat_catalog_entries() {
        assert!(is_chat_model(&model("openai/gpt-4.1")));
        assert!(!is_chat_model(&model("openai/text-embedding-3-large")));
        assert!(!is_chat_model(&model("openai/whisper-1")));
        assert!(!is_chat_model(&model("openai/dall-e-3")));
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(compare_versions("1.10.0", "1.9.9").unwrap().is_gt());
        assert!(compare_versions("v1.0.0", "1.0.0").is_none());
        assert_eq!(
            compare_versions("1.0.0", "1.0.0"),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn provider_selection_defaults_to_openrouter() {
        assert!(matches!(
            BuiltinProvider::from_choice(""),
            Some(BuiltinProvider::OpenRouter)
        ));
        assert!(matches!(
            BuiltinProvider::from_choice("2"),
            Some(BuiltinProvider::OpenAi)
        ));
        assert!(BuiltinProvider::from_choice("3").is_none());
    }
}

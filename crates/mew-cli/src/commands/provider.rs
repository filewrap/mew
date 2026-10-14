use anyhow::Result;
use mew_common::{CustomProviderConfig, MewConfig, MewPaths, ModelConfig};
use mew_provider::ProviderRegistry;
use mew_ui::kv_table;

use crate::args::{ProviderCommand, ProviderSubcommand};

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, cmd: ProviderCommand) -> Result<()> {
    match cmd.command {
        ProviderSubcommand::List => {
            let reg = ProviderRegistry::from_config(cfg);
            for p in reg.list_info() {
                println!(
                    "{}",
                    kv_table(
                        &p.id,
                        &[
                            ("name", p.name),
                            ("enabled", p.enabled.to_string()),
                            ("authorized", p.authorized.to_string()),
                            ("auth", p.auth),
                            ("base", p.base_url),
                            ("default", p.default_model),
                        ],
                    )
                );
                println!();
            }
        }
        ProviderSubcommand::Test { provider } => {
            let reg = ProviderRegistry::from_config(cfg);
            let p = reg.get(&provider)?;
            p.test().await?;
            println!("ok");
        }
        ProviderSubcommand::Login { provider } => {
            if provider != "gemini" {
                anyhow::bail!(
                    "OAuth login is implemented only for the built-in Gemini API provider"
                );
            }
            println!("Google Gemini API OAuth uses your own Desktop OAuth client and project. Configure MEW_GEMINI_OAUTH_CLIENT_ID, MEW_GEMINI_OAUTH_CLIENT_SECRET, and MEW_GEMINI_OAUTH_PROJECT_ID first.");
            mew_provider::login_gemini_oauth().await?;
        }
        ProviderSubcommand::Logout { provider } => {
            if provider != "gemini" {
                anyhow::bail!(
                    "OAuth logout is implemented only for the built-in Gemini API provider"
                );
            }
            if mew_provider::logout_gemini_oauth()? {
                println!("Removed local Gemini OAuth tokens.");
            } else {
                println!("No local Gemini OAuth tokens were present.");
            }
        }
        ProviderSubcommand::AddOpenai {
            id,
            base_url,
            api_key_env,
            model,
        } => {
            add_openai_compatible(paths, cfg, &id, &base_url, &api_key_env, &model)?;
            println!("saved provider {id}");
            println!(
                "set the {api_key_env} environment variable, then run `mew provider test {id}`"
            );
        }
    }

    Ok(())
}

pub fn add_openai_compatible(
    paths: &MewPaths,
    cfg: &mut MewConfig,
    id: &str,
    base_url: &str,
    api_key_env: &str,
    model: &str,
) -> Result<()> {
    validate_provider_id(id)?;
    validate_provider_url(base_url)?;
    validate_env_name(api_key_env)?;
    let model = model.trim();
    if model.is_empty() || model.len() > 256 || model.chars().any(char::is_control) {
        anyhow::bail!("model id must be 1–256 characters and contain no control characters");
    }
    if matches!(id, "openai" | "openrouter" | "gemini") {
        anyhow::bail!("{id:?} is a built-in provider id and cannot be replaced");
    }
    if cfg.providers.custom.contains_key(id) {
        anyhow::bail!(
            "provider {id:?} already exists; edit the config file to replace it deliberately"
        );
    }

    let custom = CustomProviderConfig {
        enabled: true,
        kind: "openai-compatible".to_string(),
        api_key_env: api_key_env.to_string(),
        base_url: base_url.trim_end_matches('/').to_string(),
        default_model: model.to_string(),
        models: vec![ModelConfig {
            id: model.to_string(),
            context: 128_000,
            supports_tools: true,
            supports_vision: false,
            notes: "custom OpenAI-compatible model".to_string(),
        }],
    };
    let previous = cfg.providers.custom.clone();
    cfg.providers.custom.insert(id.to_string(), custom);
    if let Err(error) = cfg.save(paths) {
        cfg.providers.custom = previous;
        return Err(error);
    }
    Ok(())
}

fn validate_provider_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 48
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("provider id must use 1–48 ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}

fn validate_env_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let first = chars
        .next()
        .ok_or_else(|| anyhow::anyhow!("API key environment-variable name cannot be empty"))?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        anyhow::bail!("API key environment-variable name must be a valid shell identifier");
    }
    Ok(())
}

fn validate_provider_url(value: &str) -> Result<()> {
    let url = value.trim();
    if url.is_empty() || url.len() > 2_048 || url.chars().any(char::is_whitespace) {
        anyhow::bail!(
            "provider base URL must be a non-empty URL without whitespace (max 2048 characters)"
        );
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|error| anyhow::anyhow!("invalid provider base URL: {error}"))?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        anyhow::bail!("provider base URL must not contain credentials, a query, or a fragment");
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("provider base URL is missing a host"))?;
    if host.is_empty() {
        anyhow::bail!("provider base URL is missing a host");
    }
    match parsed.scheme() {
        "https" => {}
        "http" if matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]") => {}
        "http" => {
            anyhow::bail!("unencrypted HTTP is allowed only for localhost/loopback providers")
        }
        _ => anyhow::bail!("provider base URL must use HTTPS"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_provider_urls_require_https_except_loopback() {
        assert!(validate_provider_url("https://api.example.com/v1").is_ok());
        assert!(validate_provider_url("http://localhost:8080/v1").is_ok());
        assert!(validate_provider_url("http://127.0.0.1:1234/v1").is_ok());
        assert!(validate_provider_url("http://example.com/v1").is_err());
        assert!(validate_provider_url("https://user:pass@example.com/v1").is_err());
        assert!(validate_provider_url("https://example.com/v1?key=secret").is_err());
    }

    #[test]
    fn api_key_reference_must_be_an_environment_variable_name() {
        assert!(validate_env_name("MY_PROVIDER_KEY").is_ok());
        assert!(validate_env_name("9BAD_NAME").is_err());
        assert!(validate_env_name("key=value").is_err());
    }
}

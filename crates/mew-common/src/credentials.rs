use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::MewPaths;

#[derive(Debug, Default, Serialize, Deserialize)]
struct CredentialFile {
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

/// Load saved provider credentials without ever logging their values.
pub fn load_credentials(paths: &MewPaths) -> Result<BTreeMap<String, String>> {
    let file = credentials_path(paths);
    if !file.exists() {
        return Ok(BTreeMap::new());
    }
    restrict_permissions(&file)?;
    let raw = fs::read_to_string(&file)
        .with_context(|| format!("read private credentials file {}", file.display()))?;
    let stored: CredentialFile = toml::from_str(&raw)
        .with_context(|| format!("parse private credentials file {}", file.display()))?;
    Ok(stored.keys)
}

/// Store an API key separately from ordinary configuration, with user-only file permissions.
pub fn save_credential(paths: &MewPaths, name: &str, value: &str) -> Result<()> {
    validate_env_name(name)?;
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        anyhow::bail!("credential must be non-empty and contain no control characters");
    }

    fs::create_dir_all(&paths.config_dir)
        .with_context(|| format!("create config directory {}", paths.config_dir.display()))?;
    let mut stored = CredentialFile {
        keys: load_credentials(paths)?,
    };
    stored.keys.insert(name.to_string(), value.to_string());
    let serialized = toml::to_string(&stored).context("serialize credentials")?;
    let destination = credentials_path(paths);
    let temporary = destination.with_file_name(format!(
        ".credentials-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut output = options
            .open(&temporary)
            .with_context(|| format!("create private credentials file {}", temporary.display()))?;
        output
            .write_all(serialized.as_bytes())
            .context("write private credentials file")?;
        output
            .sync_all()
            .context("flush private credentials file")?;
        restrict_permissions(&temporary)?;
        replace_file(&temporary, &destination)?;
        restrict_permissions(&destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Import saved credentials into the current process only when the caller has not already set them.
pub fn hydrate_environment(paths: &MewPaths) -> Result<()> {
    for (name, value) in load_credentials(paths)? {
        if std::env::var_os(&name).is_none() {
            // The executable uses edition 2021; hydration is performed before async tasks start.
            std::env::set_var(name, value);
        }
    }
    Ok(())
}

fn credentials_path(paths: &MewPaths) -> std::path::PathBuf {
    paths.config_dir.join("credentials.toml")
}

fn validate_env_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let first = chars
        .next()
        .ok_or_else(|| anyhow::anyhow!("credential name cannot be empty"))?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        anyhow::bail!("credential name must be a valid environment-variable name");
    }
    Ok(())
}

fn replace_file(temporary: &Path, destination: &Path) -> Result<()> {
    #[cfg(windows)]
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(temporary, destination)
        .with_context(|| format!("replace credentials file {}", destination.display()))
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("restrict permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    // On Windows, the file inherits the current user's profile-directory ACL.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_paths() -> MewPaths {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!("mew-credentials-{unique}"));
        MewPaths {
            config_file: config_dir.join("config.toml"),
            config_dir: config_dir.clone(),
            data_dir: config_dir.join("data"),
            cache_dir: config_dir.join("cache"),
        }
    }

    #[test]
    fn credentials_round_trip_without_being_part_of_the_config_file() {
        let paths = test_paths();
        save_credential(&paths, "TEST_PROVIDER_KEY", "test-secret-value").unwrap();
        let loaded = load_credentials(&paths).unwrap();
        assert_eq!(
            loaded.get("TEST_PROVIDER_KEY").unwrap(),
            "test-secret-value"
        );
        assert!(!paths.config_file.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(credentials_path(&paths))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(paths.config_dir).unwrap();
    }

    #[test]
    fn credential_names_and_values_are_validated() {
        let paths = test_paths();
        assert!(save_credential(&paths, "BAD=NAME", "x").is_err());
        assert!(save_credential(&paths, "TEST_KEY", "").is_err());
        assert!(!paths.config_dir.exists());
    }
}

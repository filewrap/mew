use anyhow::Result;
use chrono::{DateTime, Utc};
use mew_common::MewPaths;
use mew_provider::ChatMessage;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MewSession {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub title: String,
    pub provider: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

impl MewSession {
    pub fn new(title: impl Into<String>, provider: String, model: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            title: title.into(),
            provider,
            model,
            messages: Vec::new(),
        }
    }

    pub fn push(&mut self, msg: ChatMessage) {
        self.updated_at = Utc::now();
        self.messages.push(msg);
    }
}

pub async fn save_session(paths: &MewPaths, session: &MewSession) -> Result<()> {
    let dir = paths.data_dir.join("sessions");
    fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}.json", session.id));
    let raw = serde_json::to_string_pretty(session)?;
    fs::write(path, raw).await?;
    Ok(())
}

pub async fn load_session(paths: &MewPaths, id: &str) -> Result<MewSession> {
    let path = paths.data_dir.join("sessions").join(format!("{}.json", id));
    let raw = fs::read_to_string(path).await?;
    let session = serde_json::from_str(&raw)?;
    Ok(session)
}

pub async fn list_sessions(paths: &MewPaths) -> Result<Vec<MewSession>> {
    let dir = paths.data_dir.join("sessions");
    fs::create_dir_all(&dir).await?;

    let mut out = Vec::new();
    let mut rd = fs::read_dir(dir).await?;

    while let Some(entry) = rd.next_entry().await? {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let raw = fs::read_to_string(path).await?;
        if let Ok(session) = serde_json::from_str::<MewSession>(&raw) {
            out.push(session);
        }
    }

    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}

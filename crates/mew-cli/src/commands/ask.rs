use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::{system_message, user_message, ChatRequest, ProviderRegistry};
use mew_session::{save_session, MewSession};
use mew_ui::phrase;

pub async fn run(
    paths: &MewPaths,
    cfg: &mut MewConfig,
    prompt: String,
    model: Option<String>,
) -> Result<()> {
    let model_ref = model.unwrap_or_else(|| cfg.providers.active_model.clone());
    let (provider_id, model_id) = ProviderRegistry::parse_model_ref(&model_ref)?;
    let reg = ProviderRegistry::from_config(cfg);
    let provider = reg.get(&provider_id)?;

    println!("{}", phrase("connecting"));

    let messages = vec![
        system_message("You are mew, a concise CLI-first AI coding agent. Be useful, direct, and token-efficient."),
        user_message(prompt.clone()),
    ];

    let res = provider
        .chat(ChatRequest {
            model: model_id.clone(),
            messages: messages.clone(),
            temperature: Some(0.2),
            max_tokens: None,
        })
        .await?;

    println!();
    println!("{}", res.text);

    let mut session = MewSession::new(prompt, provider_id, model_id);
    for msg in messages {
        session.push(msg);
    }
    session.push(mew_provider::ChatMessage {
        role: "assistant".to_string(),
        content: res.text,
    });

    save_session(paths, &session).await?;

    Ok(())
}

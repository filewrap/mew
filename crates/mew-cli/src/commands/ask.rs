use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::{system_message, user_message, ChatRequest, ProviderRegistry};
use mew_session::{save_session, MewSession};
use mew_ui::{assistant_bubble, phrase, render_markdown_light, status_line};
use std::io::{self, Write};

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

    println!("{}", status_line(phrase("connecting")));

    let messages = vec![
        system_message("You are mew, a concise CLI-first AI coding agent. Be useful, direct, cute, and token-efficient. Use markdown when helpful."),
        user_message(prompt.clone()),
    ];

    print!("{} ", "•".to_string());
    io::stdout().flush()?;

    let mut streamed = String::new();

    let res = provider
        .chat_stream(
            ChatRequest {
                model: model_id.clone(),
                messages: messages.clone(),
                temperature: Some(0.2),
                max_tokens: None,
            },
            &mut |delta| {
                streamed.push_str(delta);
                print!("{}", delta);
                let _ = io::stdout().flush();
            },
        )
        .await?;

    println!();
    println!();

    if streamed.trim().is_empty() {
        println!(
            "{}",
            assistant_bubble(&cfg.identity.display_name, &render_markdown_light(&res.text))
        );
    }

    let mut session = MewSession::new(prompt, provider_id, model_id);
    for msg in messages {
        session.push(msg);
    }
    session.push(mew_provider::ChatMessage {
        role: "assistant".to_string(),
        content: res.text,
    });

    save_session(paths, &session).await?;

    println!();
    println!(
        "{}",
        status_line(&format!("session saved: {}", session.id))
    );

    Ok(())
}

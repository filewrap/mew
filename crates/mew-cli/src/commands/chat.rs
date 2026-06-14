use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::{system_message, user_message, ChatMessage, ChatRequest, ProviderRegistry};
use mew_session::{save_session, MewSession};
use std::io::{self, Write};

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, model: Option<String>) -> Result<()> {
    let model_ref = model.unwrap_or_else(|| cfg.providers.active_model.clone());
    let (provider_id, model_id) = ProviderRegistry::parse_model_ref(&model_ref)?;
    let reg = ProviderRegistry::from_config(cfg);
    let provider = reg.get(&provider_id)?;

    let mut session = MewSession::new("chat", provider_id.clone(), model_id.clone());

    session.push(system_message(
        "You are mew, a concise CLI-first AI coding agent. Be useful, direct, and token-efficient.",
    ));

    println!("mew chat");
    println!("model: {}/{}", provider_id, model_id);
    println!("type /exit to leave");
    println!();

    loop {
        print!("mew › ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_string();

        if input.is_empty() {
            continue;
        }

        if matches!(input.as_str(), "/exit" | "/quit" | "exit" | "quit") {
            break;
        }

        if input == "/model" {
            println!("{}/{}", provider_id, model_id);
            continue;
        }

        session.push(user_message(input.clone()));

        let res = provider
            .chat(ChatRequest {
                model: model_id.clone(),
                messages: session.messages.clone(),
                temperature: Some(0.2),
                max_tokens: None,
            })
            .await?;

        println!();
        println!("{}", res.text);
        println!();

        session.push(ChatMessage {
            role: "assistant".to_string(),
            content: res.text,
        });

        save_session(paths, &session).await?;
    }

    save_session(paths, &session).await?;
    println!("saved session {}", session.id);

    Ok(())
}

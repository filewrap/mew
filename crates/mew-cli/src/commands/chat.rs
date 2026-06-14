use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::{system_message, user_message, ChatMessage, ChatRequest, ProviderRegistry};
use mew_session::{list_sessions, save_session, MewSession};
use mew_ui::{chat_banner, clear_screen, render_markdown_light, slash_menu, status_line, user_line};
use std::io::{self, Write};

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, model: Option<String>) -> Result<()> {
    let model_ref = model.unwrap_or_else(|| cfg.providers.active_model.clone());
    let (provider_id, model_id) = ProviderRegistry::parse_model_ref(&model_ref)?;
    let reg = ProviderRegistry::from_config(cfg);
    let provider = reg.get(&provider_id)?;

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string());

    clear_screen();
    println!("{}", chat_banner(cfg, &model_ref, &cwd));
    println!();

    let mut session = MewSession::new("chat", provider_id.clone(), model_id.clone());

    session.push(system_message(
        "You are mew, a concise CLI-first AI coding agent. Be useful, direct, cute, and token-efficient. Use markdown when helpful.",
    ));

    loop {
        print!("{} ", "›");
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

        if input == "/" || input == "/help" {
            println!();
            println!("{}", slash_menu());
            println!();
            continue;
        }

        if input == "/clear" {
            clear_screen();
            println!("{}", chat_banner(cfg, &model_ref, &cwd));
            println!();
            continue;
        }

        if input == "/model" {
            println!("{}", status_line(&format!("model: {}/{}", provider_id, model_id)));
            continue;
        }

        if input == "/providers" {
            let reg = ProviderRegistry::from_config(cfg);
            for p in reg.list_info() {
                println!(
                    "{} {} authorized={}",
                    if p.authorized { "•" } else { "×" },
                    p.id,
                    p.authorized
                );
            }
            println!();
            continue;
        }

        if input == "/models" {
            let reg = ProviderRegistry::from_config(cfg);
            for m in reg.list_models_for(&provider_id)? {
                println!("• {}/{}", m.provider, m.id);
            }
            println!();
            continue;
        }

        if input == "/sessions" {
            let sessions = list_sessions(paths).await?;
            for s in sessions.iter().take(8) {
                println!("• {}  {}  {}", s.id, s.model, s.title);
            }
            println!();
            continue;
        }

        println!("{}", user_line(&input));
        session.push(user_message(input.clone()));

        print!("{} ", "•");
        io::stdout().flush()?;

        let mut full = String::new();

        let res = provider
            .chat_stream(
                ChatRequest {
                    model: model_id.clone(),
                    messages: session.messages.clone(),
                    temperature: Some(0.2),
                    max_tokens: None,
                },
                &mut |delta| {
                    full.push_str(delta);
                    print!("{}", delta);
                    let _ = io::stdout().flush();
                },
            )
            .await?;

        println!();
        println!();

        if full.trim().is_empty() {
            println!("{}", render_markdown_light(&res.text));
            println!();
        }

        session.push(ChatMessage {
            role: "assistant".to_string(),
            content: res.text,
        });

        save_session(paths, &session).await?;
    }

    save_session(paths, &session).await?;
    println!("{}", status_line(&format!("saved session {}", session.id)));

    Ok(())
}

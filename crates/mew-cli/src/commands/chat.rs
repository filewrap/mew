use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::{system_message, user_message, ChatMessage, ChatRequest, ProviderRegistry};
use mew_session::{list_sessions, save_session, MewSession};
use mew_ui::{chat_banner, clear_screen, meta_line, slash_menu, status_line, user_line};
use std::io::{self, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

pub async fn run(paths: &MewPaths, cfg: &mut MewConfig, model: Option<String>) -> Result<()> {
    let model_ref = model.unwrap_or_else(|| cfg.providers.active_model.clone());
    let (provider_id, model_id) = ProviderRegistry::parse_model_ref(&model_ref)?;
    let reg = ProviderRegistry::from_config(cfg);
    let provider = reg.get(&provider_id)?;

    let cancelled = install_cancel_flag();

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string());

    clear_screen();
    println!("{}", chat_banner(cfg, &model_ref, &cwd));
    println!();
    println!("{}", status_line("type / for commands · /exit to leave"));
    println!();

    let mut session = MewSession::new("chat", provider_id.clone(), model_id.clone());

    session.push(system_message(
        "You are mew, a concise CLI-first AI coding agent. Be useful, direct, cute, and token-efficient. Use markdown when helpful.",
    ));

    chat_loop(
        paths,
        cfg,
        &provider_id,
        &model_id,
        provider,
        &mut session,
        cancelled,
    )
    .await
}

pub async fn resume(paths: &MewPaths, cfg: &mut MewConfig, id: String) -> Result<()> {
    let mut session = mew_session::load_session(paths, &id).await?;
    let provider_id = session.provider.clone();
    let model_id = session.model.clone();
    let model_ref = format!("{}/{}", provider_id, model_id);

    let reg = ProviderRegistry::from_config(cfg);
    let provider = reg.get(&provider_id)?;
    let cancelled = install_cancel_flag();

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string());

    clear_screen();
    println!("{}", chat_banner(cfg, &model_ref, &cwd));
    println!();
    println!("{}", status_line(&format!("resumed session {}", id)));
    println!("{}", status_line("type / for commands · /exit to leave"));
    println!();

    chat_loop(
        paths,
        cfg,
        &provider_id,
        &model_id,
        provider,
        &mut session,
        cancelled,
    )
    .await
}

async fn chat_loop(
    paths: &MewPaths,
    cfg: &mut MewConfig,
    provider_id: &str,
    model_id: &str,
    provider: std::sync::Arc<dyn mew_provider::Provider>,
    session: &mut MewSession,
    cancelled: Arc<AtomicBool>,
) -> Result<()> {
    loop {
        if cancelled.load(Ordering::SeqCst) {
            save_session(paths, session).await?;
            println!();
            println!("{}", status_line(&format!("saved session {}", session.id)));
            break;
        }

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
            for m in reg.list_models_for(provider_id)? {
                println!("• {}/{}", m.provider, m.id);
            }
            println!();
            continue;
        }

        if input == "/remote-models" {
            let reg = ProviderRegistry::from_config(cfg);
            match reg.list_remote_models_for(provider_id).await {
                Ok(models) => {
                    for (i, m) in models.iter().enumerate() {
                        println!("{} {} {}", i + 1, m.id.rsplit('/').next().unwrap_or(&m.id), m.id);
                    }
                }
                Err(err) => println!("error: {}", err),
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

        let start = Instant::now();
        let spinner = start_spinner("meowiinnggg~");

        let mut full = String::new();

        let res = provider
            .chat_stream(
                ChatRequest {
                    model: model_id.to_string(),
                    messages: session.messages.clone(),
                    temperature: Some(0.2),
                    max_tokens: None,
                },
                &mut |delta: String| {
                    spinner.store(false, Ordering::SeqCst);
                    full.push_str(&delta);
                    print!("{}", delta);
                    let _ = io::stdout().flush();
                },
            )
            .await?;

        spinner.store(false, Ordering::SeqCst);
        println!();
        println!();

        let elapsed = start.elapsed();

        session.push(ChatMessage {
            role: "assistant".to_string(),
            content: res.text,
        });

        save_session(paths, session).await?;

        println!(
            "{}",
            meta_line(&format!(
                "time={}ms model={}/{} tokens=in:{} out:{} session={}",
                elapsed.as_millis(),
                provider_id,
                model_id,
                res.input_tokens
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                res.output_tokens
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "?".to_string()),
                session.id
            ))
        );
        println!();
    }

    save_session(paths, session).await?;
    println!("{}", status_line(&format!("saved session {}", session.id)));

    Ok(())
}

fn install_cancel_flag() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let f = flag.clone();

    let _ = ctrlc::set_handler(move || {
        f.store(true, Ordering::SeqCst);
    });

    flag
}

fn start_spinner(text: &'static str) -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    std::thread::spawn(move || {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut i = 0usize;

        while r.load(Ordering::SeqCst) {
            eprint!("\r{} {}", frames[i % frames.len()], text);
            let _ = io::stderr().flush();
            i += 1;
            std::thread::sleep(std::time::Duration::from_millis(80));
        }

        eprint!("\r{}\r", " ".repeat(48));
        let _ = io::stderr().flush();
    });

    running
}

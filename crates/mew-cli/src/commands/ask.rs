use anyhow::Result;
use mew_common::{MewConfig, MewPaths};
use mew_provider::{system_message, user_message, ChatRequest, ProviderRegistry};
use mew_session::{save_session, MewSession};
use mew_ui::{assistant_bubble, meta_line, phrase, status_line, theme_by_name, Theme};
use std::io::{self, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

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
    let theme = theme_by_name(&cfg.style.theme);

    let cancelled = install_cancel_flag();

    let messages = vec![
        system_message("You are mew, a concise CLI-first AI coding agent. Be useful, direct, cute, and token-efficient. Use markdown when helpful."),
        user_message(prompt.clone()),
    ];

    println!("{}", status_line(&theme, phrase("connecting")));

    let start = Instant::now();
    let spinner = start_spinner("meowiinnggg~");

    let mut streamed = String::new();

    let res = provider
        .chat_stream(
            ChatRequest {
                model: model_id.clone(),
                messages: messages.clone(),
                temperature: Some(0.2),
                max_tokens: None,
            },
            &mut |_delta: String| {
                spinner.store(false, Ordering::SeqCst);
            },
        )
        .await?;

    spinner.store(false, Ordering::SeqCst);
    streamed = res.text.clone();

    if cancelled.load(Ordering::SeqCst) {
        println!();
        println!("{}", status_line(&theme, "cancelled"));
        return Ok(());
    }

    println!();
    println!("{}", assistant_bubble(&theme, &cfg.identity.display_name, &streamed));
    println!();

    let elapsed = start.elapsed();

    let mut session = MewSession::new(prompt, provider_id.clone(), model_id.clone());
    for msg in messages {
        session.push(msg);
    }
    session.push(mew_provider::ChatMessage {
        role: "assistant".to_string(),
        content: streamed,
    });

    save_session(paths, &session).await?;

    println!(
        "{}",
        meta_line(
            &theme,
            &format!(
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
            )
        )
    );

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

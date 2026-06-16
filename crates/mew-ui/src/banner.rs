use mew_common::MewConfig;
use owo_colors::OwoColorize;

use crate::layout::{center, fit, kv_line, line, ScreenClass, TerminalLayout};
use crate::phrases::phrase;

pub fn startup_banner(cfg: &MewConfig, project_status: &str) -> String {
    let layout = TerminalLayout::detect();
    match layout.class {
        ScreenClass::Tiny => tiny_banner(cfg, project_status, layout),
        ScreenClass::Narrow => narrow_banner(cfg, project_status, layout),
        ScreenClass::Normal => normal_banner(cfg, project_status, layout),
        ScreenClass::Wide => wide_banner(cfg, project_status, layout),
    }
}

pub fn chat_banner(cfg: &MewConfig, model: &str, directory: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let title = format!(">_ {} agent", cfg.identity.display_name);
    let model_line = format!("model:     {}   /model", model);
    let dir_line = format!("directory: {}", directory);

    vec![
        format!("{}{}{}", "╭".bright_black(), line('─', w - 2), "╮".bright_black()),
        format!(
            "{} {} {}",
            "│".bright_black(),
            fit(&title, inner).bright_cyan(),
            "│".bright_black()
        ),
        format!(
            "{} {} {}",
            "│".bright_black(),
            fit("", inner),
            "│".bright_black()
        ),
        format!(
            "{} {} {}",
            "│".bright_black(),
            fit(&model_line, inner),
            "│".bright_black()
        ),
        format!(
            "{} {} {}",
            "│".bright_black(),
            fit(&dir_line, inner),
            "│".bright_black()
        ),
        format!("{}{}{}", "╰".bright_black(), line('─', w - 2), "╯".bright_black()),
    ]
    .join("\n")
}

pub fn slash_menu() -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let rows = [
        ("/model", "show current model"),
        ("/providers", "list providers"),
        ("/models", "list models for current provider"),
        ("/remote-models", "fetch models from current provider"),
        ("/sessions", "list recent sessions"),
        ("/clear", "clear terminal"),
        ("/exit", "save and leave"),
    ];

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        "╭─ ".bright_black(),
        "commands".bright_magenta(),
        format!(" {}", line('─', w.saturating_sub(12))).bright_black()
    ));

    for (cmd, desc) in rows {
        out.push_str(&format!(
            "{} {} {}\n",
            "│".bright_black(),
            fit(&format!("{:<15} {}", cmd, desc), inner),
            "│".bright_black()
        ));
    }

    out.push_str(&format!(
        "{}{}{}",
        "╰".bright_black(),
        line('─', w - 2),
        "╯".bright_black()
    ));

    out
}

fn tiny_banner(cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", "╭".bright_black()) + &line('─', w - 2) + &format!("{}", "╮".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit("/\\_/\\\\", inner), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit("( o.o )", inner), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit(" > ^ <", inner), "│".bright_black()),
        format!("{}{}{}", "├".bright_black(), line('─', w - 2), "┤".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit(&name, inner).bright_cyan(), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit(tagline, inner), "│".bright_black()),
        format!("{}{}{}", "├".bright_black(), line('─', w - 2), "┤".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit(&kv_line("model", &cfg.providers.active_model, inner), inner), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit(&kv_line("project", project_status, inner), inner), "│".bright_black()),
        format!("{}", "╰".bright_black()) + &line('─', w - 2) + &format!("{}", "╯".bright_black()),
    ]
    .join("\n")
}

fn narrow_banner(cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", "╭".bright_black()) + &line('─', w - 2) + &format!("{}", "╮".bright_black()),
        format!("{} {} {}", "│".bright_black(), center("╱|、", inner).bright_magenta(), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), center("(˚ˎ 。7", inner).bright_magenta(), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), center("|、˜〵", inner).bright_magenta(), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), center("じしˍ,)ノ", inner).bright_magenta(), "│".bright_black()),
        format!("{}{}{}", "├".bright_black(), line('─', w - 2), "┤".bright_black()),
        format!("{} {} {}", "│".bright_black(), center(&name, inner).bright_cyan(), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), center(tagline, inner), "│".bright_black()),
        format!("{}{}{}", "├".bright_black(), line('─', w - 2), "┤".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit(&kv_line("model", &cfg.providers.active_model, inner), inner), "│".bright_black()),
        format!("{} {} {}", "│".bright_black(), fit(&kv_line("project", project_status, inner), inner), "│".bright_black()),
        format!("{}", "╰".bright_black()) + &line('─', w - 2) + &format!("{}", "╯".bright_black()),
    ]
    .join("\n")
}

fn normal_banner(cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let left = 26;
    let right = inner - left - 2;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", "╭".bright_black()) + &line('─', w - 2) + &format!("{}", "╮".bright_black()),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit("        *      /\\_/\\\\", left).bright_magenta(),
            fit(&name, right).bright_cyan(),
            "│".bright_black()
        ),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit("   *          ( o.o )", left).bright_magenta(),
            fit(tagline, right),
            "│".bright_black()
        ),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit("        ░░░    > ^ <", left).bright_magenta(),
            fit("cute shell · sharp claws", right).bright_black(),
            "│".bright_black()
        ),
        format!("{}{}{}", "├".bright_black(), line('─', w - 2), "┤".bright_black()),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit(&kv_line("model", &cfg.providers.active_model, left), left),
            fit(&kv_line("project", project_status, right), right),
            "│".bright_black()
        ),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit(&kv_line("style", &cfg.style.theme, left), left),
            fit(&kv_line("exit", "/exit or ctrl+c", right), right),
            "│".bright_black()
        ),
        format!("{}", "╰".bright_black()) + &line('─', w - 2) + &format!("{}", "╯".bright_black()),
    ]
    .join("\n")
}

fn wide_banner(cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let left = 34;
    let right = inner - left - 2;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", "╭".bright_black()) + &line('─', w - 2) + &format!("{}", "╮".bright_black()),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit("   *              ╱|、", left).bright_magenta(),
            fit(&name, right).bright_cyan(),
            "│".bright_black()
        ),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit("        ░░░     (˚ˎ 。7", left).bright_magenta(),
            fit(tagline, right),
            "│".bright_black()
        ),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit("    ░░░░░░░░      |、˜〵", left).bright_magenta(),
            fit("CLI-first · token-smart · guard-protected", right).bright_black(),
            "│".bright_black()
        ),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit("  ░░░░░░░░░░░░    じしˍ,)ノ", left).bright_magenta(),
            fit("from Termux caves to x86 castles", right).bright_black(),
            "│".bright_black()
        ),
        format!("{}{}{}", "├".bright_black(), line('─', w - 2), "┤".bright_black()),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit(&kv_line("model", &cfg.providers.active_model, left), left),
            fit(&kv_line("project", project_status, right), right),
            "│".bright_black()
        ),
        format!(
            "{} {}  {} {}",
            "│".bright_black(),
            fit(&kv_line("style", &cfg.style.theme, left), left),
            fit(&kv_line("exit", "/exit or ctrl+c", right), right),
            "│".bright_black()
        ),
        format!("{}", "╰".bright_black()) + &line('─', w - 2) + &format!("{}", "╯".bright_black()),
    ]
    .join("\n")
}

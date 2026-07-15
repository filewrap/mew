use mew_common::MewConfig;

use crate::layout::{center, fit, kv_line, line, ScreenClass, TerminalLayout};
use crate::phrases::phrase;
use crate::theme::Theme;

pub fn startup_banner(theme: &Theme, cfg: &MewConfig, project_status: &str) -> String {
    let layout = TerminalLayout::detect();
    match layout.class {
        ScreenClass::Tiny => tiny_banner(theme, cfg, project_status, layout),
        ScreenClass::Narrow => narrow_banner(theme, cfg, project_status, layout),
        ScreenClass::Normal => normal_banner(theme, cfg, project_status, layout),
        ScreenClass::Wide => wide_banner(theme, cfg, project_status, layout),
    }
}

pub fn chat_banner(theme: &Theme, cfg: &MewConfig, model: &str, directory: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let title = format!(">_ {} agent", cfg.identity.display_name);
    let model_line = format!("model:     {}   /model", model);
    let dir_line = format!("directory: {}", directory);

    vec![
        format!(
            "{}{}{}",
            theme.border("╭"),
            theme.border(line('─', w - 2)),
            theme.border("╮")
        ),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit(&title, inner)),
            theme.border("│")
        ),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit("", inner)),
            theme.border("│")
        ),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit(&model_line, inner)),
            theme.border("│")
        ),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit(&dir_line, inner)),
            theme.border("│")
        ),
        format!(
            "{}{}{}",
            theme.border("╰"),
            theme.border(line('─', w - 2)),
            theme.border("╯")
        ),
    ]
    .join("\n")
}

pub fn slash_menu(theme: &Theme) -> String {
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
        theme.border("╭─ "),
        theme.accent("commands"),
        theme.border(format!(" {}", line('─', w.saturating_sub(12))))
    ));

    for (cmd, desc) in rows {
        out.push_str(&format!(
            "{} {} {}\n",
            theme.border("│"),
            fit(&format!("{:<15} {}", cmd, desc), inner),
            theme.border("│")
        ));
    }

    out.push_str(&format!(
        "{}{}{}",
        theme.border("╰"),
        theme.border(line('─', w - 2)),
        theme.border("╯")
    ));

    out
}

fn tiny_banner(theme: &Theme, cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", theme.border("╭")) + &theme.border(line('─', w - 2)) + &theme.border("╮"),
        format!("{} {} {}", theme.border("│"), theme.accent(fit("/\\_/\\", inner)), theme.border("│")),
        format!("{} {} {}", theme.border("│"), theme.accent(fit("( o.o )", inner)), theme.border("│")),
        format!("{} {} {}", theme.border("│"), theme.accent(fit(" > ^ <", inner)), theme.border("│")),
        format!("{}{}{}", theme.border("├"), theme.border(line('─', w - 2)), theme.border("┤")),
        format!("{} {} {}", theme.border("│"), theme.primary(fit(&name, inner)), theme.border("│")),
        format!("{} {} {}", theme.border("│"), theme.dim(fit(tagline, inner)), theme.border("│")),
        format!("{}{}{}", theme.border("├"), theme.border(line('─', w - 2)), theme.border("┤")),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("model", &cfg.providers.active_model, inner), inner)),
            theme.border("│")
        ),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("project", project_status, inner), inner)),
            theme.border("│")
        ),
        format!("{}", theme.border("╰")) + &theme.border(line('─', w - 2)) + &theme.border("╯"),
    ]
    .join("\n")
}

fn narrow_banner(theme: &Theme, cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", theme.border("╭")) + &theme.border(line('─', w - 2)) + &theme.border("╮"),
        format!("{} {} {}", theme.border("│"), theme.accent(center("╱|、", inner)), theme.border("│")),
        format!("{} {} {}", theme.border("│"), theme.accent(center("(˚ˎ 。7", inner)), theme.border("│")),
        format!("{} {} {}", theme.border("│"), theme.accent(center("|、˜〵", inner)), theme.border("│")),
        format!("{} {} {}", theme.border("│"), theme.accent(center("じしˍ,)ノ", inner)), theme.border("│")),
        format!("{}{}{}", theme.border("├"), theme.border(line('─', w - 2)), theme.border("┤")),
        format!("{} {} {}", theme.border("│"), theme.primary(center(&name, inner)), theme.border("│")),
        format!("{} {} {}", theme.border("│"), theme.dim(center(tagline, inner)), theme.border("│")),
        format!("{}{}{}", theme.border("├"), theme.border(line('─', w - 2)), theme.border("┤")),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("model", &cfg.providers.active_model, inner), inner)),
            theme.border("│")
        ),
        format!(
            "{} {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("project", project_status, inner), inner)),
            theme.border("│")
        ),
        format!("{}", theme.border("╰")) + &theme.border(line('─', w - 2)) + &theme.border("╯"),
    ]
    .join("\n")
}

fn normal_banner(theme: &Theme, cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let left = 26;
    let right = inner - left - 2;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", theme.border("╭")) + &theme.border(line('─', w - 2)) + &theme.border("╮"),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.accent(fit("        *      /\\_/\\", left)),
            theme.primary(fit(&name, right)),
            theme.border("│")
        ),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.accent(fit("   *          ( o.o )", left)),
            theme.primary(fit(tagline, right)),
            theme.border("│")
        ),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.accent(fit("        ░░░    > ^ <", left)),
            theme.dim(fit("cute shell · sharp claws", right)),
            theme.border("│")
        ),
        format!("{}{}{}", theme.border("├"), theme.border(line('─', w - 2)), theme.border("┤")),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("model", &cfg.providers.active_model, left), left)),
            theme.primary(fit(&kv_line("project", project_status, right), right)),
            theme.border("│")
        ),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("style", &cfg.style.theme, left), left)),
            theme.primary(fit(&kv_line("exit", "/exit or ctrl+c", right), right)),
            theme.border("│")
        ),
        format!("{}", theme.border("╰")) + &theme.border(line('─', w - 2)) + &theme.border("╯"),
    ]
    .join("\n")
}

fn wide_banner(theme: &Theme, cfg: &MewConfig, project_status: &str, layout: TerminalLayout) -> String {
    let w = layout.card_width();
    let inner = w - 4;
    let left = 34;
    let right = inner - left - 2;
    let name = format!("{} agent", cfg.identity.display_name);
    let tagline = phrase("startup");

    vec![
        format!("{}", theme.border("╭")) + &theme.border(line('─', w - 2)) + &theme.border("╮"),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.accent(fit("   *              ╱|、", left)),
            theme.primary(fit(&name, right)),
            theme.border("│")
        ),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.accent(fit("        ░░░     (˚ˎ 。7", left)),
            theme.primary(fit(tagline, right)),
            theme.border("│")
        ),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.accent(fit("    ░░░░░░░░      |、˜〵", left)),
            theme.dim(fit("CLI-first · token-smart · guard-protected", right)),
            theme.border("│")
        ),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.accent(fit("  ░░░░░░░░░░░░    じしˍ,)ノ", left)),
            theme.dim(fit("from Termux caves to x86 castles", right)),
            theme.border("│")
        ),
        format!("{}{}{}", theme.border("├"), theme.border(line('─', w - 2)), theme.border("┤")),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("model", &cfg.providers.active_model, left), left)),
            theme.primary(fit(&kv_line("project", project_status, right), right)),
            theme.border("│")
        ),
        format!(
            "{} {}  {} {}",
            theme.border("│"),
            theme.primary(fit(&kv_line("style", &cfg.style.theme, left), left)),
            theme.primary(fit(&kv_line("exit", "/exit or ctrl+c", right), right)),
            theme.border("│")
        ),
        format!("{}", theme.border("╰")) + &theme.border(line('─', w - 2)) + &theme.border("╯"),
    ]
    .join("\n")
}

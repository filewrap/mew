use crate::layout::{fit, line, wrap_lines, TerminalLayout};
use crate::theme::Theme;

pub fn hint_card(theme: &Theme, lines: &[&str]) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;
    let mut out = String::new();

    out.push_str(&format!(
        "{}{}{}\n",
        theme.border("╭─ "),
        theme.accent("hint"),
        theme.border(format!(" {}", line('─', w.saturating_sub(8))))
    ));

    for line_text in lines {
        for wrapped in wrap_lines(line_text, inner) {
            out.push_str(&format!(
                "{} {} {}\n",
                theme.border("│"),
                fit(&wrapped, inner),
                theme.border("│")
            ));
        }
    }

    out.push_str(&format!(
        "{}{}{}",
        theme.border("╰"),
        theme.border(line('─', w - 2)),
        theme.border("╯")
    ));

    out
}

pub fn error_card(theme: &Theme, title: &str, body: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;
    let mut out = String::new();

    out.push_str(&format!(
        "{}{}{}\n",
        theme.border("╭─ "),
        theme.err(format!("hiss! {title}")),
        theme.border(format!(" {}", line('─', w.saturating_sub(title.len() + 10))))
    ));

    for wrapped in wrap_lines(body, inner) {
        out.push_str(&format!(
            "{} {} {}\n",
            theme.border("│"),
            fit(&wrapped, inner),
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

pub fn tool_card(theme: &Theme, tool: &str, target: &str, risk: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let rows = [
        format!("tool    {tool}"),
        format!("target  {target}"),
        format!("risk    {risk}"),
    ];

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        theme.border("╭─ "),
        theme.accent("paw step"),
        theme.border(format!(" {}", line('─', w.saturating_sub(12))))
    ));

    for row in rows {
        out.push_str(&format!(
            "{} {} {}\n",
            theme.border("│"),
            fit(&row, inner),
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

pub fn code_block(theme: &Theme, lang: &str, code: &str) -> String {
    crate::render::render_code(theme, lang, code)
}

pub fn diff_sample(theme: &Theme) -> String {
    crate::render::render_diff(
        theme,
        r#"diff --git a/src/main.rs b/src/main.rs
- println!("hello");
+ println!("mew~");"#,
    )
}

use crate::layout::{fit, line, wrap_lines, TerminalLayout};
use crate::theme::Theme;

pub fn render_code(theme: &Theme, lang: &str, code: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        theme.border("╭─ "),
        theme.primary(lang),
        theme.border(format!(" {}", line('─', w.saturating_sub(lang.len() + 5))))
    ));

    for raw in code.lines() {
        out.push_str(&format!(
            "{} {} {}\n",
            theme.border("│"),
            fit(raw, inner),
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

pub fn render_diff(theme: &Theme, diff: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        theme.border("╭─ "),
        theme.accent("diff"),
        theme.border(format!(" {}", line('─', w.saturating_sub(7))))
    ));

    for raw in diff.lines() {
        let styled = if raw.starts_with('+') {
            theme.ok(raw)
        } else if raw.starts_with('-') {
            theme.err(raw)
        } else if raw.starts_with("diff") || raw.starts_with("@@") {
            theme.dim(raw)
        } else {
            raw.to_string()
        };

        out.push_str(&format!(
            "{} {} {}\n",
            theme.border("│"),
            fit(&styled, inner),
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

pub fn render_markdown_light(theme: &Theme, input: &str) -> String {
    let layout = TerminalLayout::detect();
    let width = layout.card_width().saturating_sub(4).max(24);

    let mut out = String::new();
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code = String::new();

    for raw in input.lines() {
        let line = raw.trim_end();

        if line.starts_with("```") {
            if in_code {
                out.push_str(&render_code(theme, &code_lang, code.trim_end()));
                out.push('\n');
                code.clear();
                code_lang.clear();
                in_code = false;
            } else {
                in_code = true;
                code_lang = line.trim_start_matches("```").trim().to_string();
                if code_lang.is_empty() {
                    code_lang = "text".to_string();
                }
            }
            continue;
        }

        if in_code {
            code.push_str(line);
            code.push('\n');
            continue;
        }

        if line.trim().is_empty() {
            out.push('\n');
            continue;
        }

        if line.starts_with("# ") {
            out.push_str(&theme.h1(line.trim_start_matches("# ")));
            out.push('\n');
            continue;
        }

        if line.starts_with("## ") {
            out.push_str(&theme.h2(line.trim_start_matches("## ")));
            out.push('\n');
            continue;
        }

        if line.starts_with("### ") {
            out.push_str(&theme.h3(line.trim_start_matches("### ")));
            out.push('\n');
            continue;
        }

        if line.starts_with("- ") || line.starts_with("* ") {
            let item = line[2..].trim();
            for (i, wrapped) in wrap_lines(item, width.saturating_sub(4)).iter().enumerate() {
                if i == 0 {
                    out.push_str(&format!(
                        "  {} {}\n",
                        theme.bullet("•"),
                        wrapped
                    ));
                } else {
                    out.push_str(&format!("    {}\n", wrapped));
                }
            }
            continue;
        }

        if line.starts_with("> ") {
            let quote = line.trim_start_matches("> ").trim();
            for wrapped in wrap_lines(quote, width.saturating_sub(3)) {
                out.push_str(&format!("{} {}\n", theme.border("│"), theme.quote(wrapped)));
            }
            continue;
        }

        for wrapped in wrap_lines(line, width) {
            out.push_str(&wrapped);
            out.push('\n');
        }
    }

    if in_code && !code.is_empty() {
        out.push_str(&render_code(theme, &code_lang, code.trim_end()));
        out.push('\n');
    }

    out.trim_end().to_string()
}

pub fn assistant_bubble(theme: &Theme, name: &str, text: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;
    let rendered = render_markdown_light(theme, text);

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        theme.border("╭─ "),
        theme.primary(name),
        theme.border(format!(" {}", line('─', w.saturating_sub(name.len() + 5))))
    ));

    for raw in rendered.lines() {
        for wrapped in wrap_lines(raw, inner) {
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

pub fn user_line(theme: &Theme, text: &str) -> String {
    format!("{} {}", theme.accent("›"), text)
}

pub fn status_line(theme: &Theme, text: &str) -> String {
    format!("{} {}", theme.primary("•"), theme.dim(text))
}

pub fn meta_line(theme: &Theme, text: &str) -> String {
    format!("{}", theme.dim(text))
}

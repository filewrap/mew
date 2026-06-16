use owo_colors::OwoColorize;

use crate::layout::{fit, line, wrap_lines, TerminalLayout};

pub fn render_code(lang: &str, code: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        "╭─ ".bright_black(),
        lang.bright_cyan(),
        format!(" {}", line('─', w.saturating_sub(lang.len() + 5))).bright_black()
    ));

    for raw in code.lines() {
        out.push_str(&format!(
            "{} {} {}\n",
            "│".bright_black(),
            fit(raw, inner),
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

pub fn render_diff(diff: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        "╭─ ".bright_black(),
        "diff".bright_magenta(),
        format!(" {}", line('─', w.saturating_sub(7))).bright_black()
    ));

    for raw in diff.lines() {
        let styled = if raw.starts_with('+') {
            raw.bright_green().to_string()
        } else if raw.starts_with('-') {
            raw.bright_red().to_string()
        } else if raw.starts_with("diff") || raw.starts_with("@@") {
            raw.bright_black().to_string()
        } else {
            raw.to_string()
        };

        out.push_str(&format!(
            "{} {} {}\n",
            "│".bright_black(),
            fit(&styled, inner),
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

pub fn render_markdown_light(input: &str) -> String {
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
                out.push_str(&render_code(&code_lang, code.trim_end()));
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
            out.push_str(&line.trim_start_matches("# ").bright_cyan().bold().to_string());
            out.push('\n');
            continue;
        }

        if line.starts_with("## ") {
            out.push_str(&line.trim_start_matches("## ").bright_magenta().bold().to_string());
            out.push('\n');
            continue;
        }

        if line.starts_with("### ") {
            out.push_str(&line.trim_start_matches("### ").bright_blue().bold().to_string());
            out.push('\n');
            continue;
        }

        if line.starts_with("- ") || line.starts_with("* ") {
            let item = line[2..].trim();
            for (i, wrapped) in wrap_lines(item, width.saturating_sub(4)).iter().enumerate() {
                if i == 0 {
                    out.push_str(&format!("  {} {}\n", "•".bright_magenta(), wrapped));
                } else {
                    out.push_str(&format!("    {}\n", wrapped));
                }
            }
            continue;
        }

        if line.starts_with("> ") {
            let quote = line.trim_start_matches("> ").trim();
            for wrapped in wrap_lines(quote, width.saturating_sub(3)) {
                out.push_str(&format!("{} {}\n", "│".bright_black(), wrapped.bright_black()));
            }
            continue;
        }

        for wrapped in wrap_lines(line, width) {
            out.push_str(&wrapped);
            out.push('\n');
        }
    }

    if in_code && !code.is_empty() {
        out.push_str(&render_code(&code_lang, code.trim_end()));
        out.push('\n');
    }

    out.trim_end().to_string()
}

pub fn assistant_bubble(name: &str, text: &str) -> String {
    let layout = TerminalLayout::detect();
    let w = layout.card_width();
    let inner = w - 4;
    let rendered = render_markdown_light(text);

    let mut out = String::new();
    out.push_str(&format!(
        "{}{}{}\n",
        "╭─ ".bright_black(),
        name.bright_cyan(),
        format!(" {}", line('─', w.saturating_sub(name.len() + 5))).bright_black()
    ));

    for raw in rendered.lines() {
        for wrapped in wrap_lines(raw, inner) {
            out.push_str(&format!(
                "{} {} {}\n",
                "│".bright_black(),
                fit(&wrapped, inner),
                "│".bright_black()
            ));
        }
    }

    out.push_str(&format!(
        "{}{}{}",
        "╰".bright_black(),
        line('─', w - 2),
        "╯".bright_black()
    ));

    out
}

pub fn user_line(text: &str) -> String {
    format!("{} {}", "›".bright_magenta(), text)
}

pub fn status_line(text: &str) -> String {
    format!("{} {}", "•".bright_cyan(), text.bright_black())
}

pub fn meta_line(text: &str) -> String {
    format!("{}", text.bright_black())
}

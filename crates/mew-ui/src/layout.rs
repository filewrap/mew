use std::io::{self, IsTerminal, Write};
use terminal_size::{terminal_size, Width};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenClass {
    Tiny,
    Narrow,
    Normal,
    Wide,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalLayout {
    pub width: usize,
    pub class: ScreenClass,
}

impl TerminalLayout {
    pub fn detect() -> Self {
        let width = terminal_size()
            .map(|(Width(w), _)| w as usize)
            .unwrap_or(80)
            .clamp(28, 180);

        let class = if width < 46 {
            ScreenClass::Tiny
        } else if width < 72 {
            ScreenClass::Narrow
        } else if width < 112 {
            ScreenClass::Normal
        } else {
            ScreenClass::Wide
        };

        Self { width, class }
    }

    pub fn inner_width(&self) -> usize {
        self.width.saturating_sub(4).max(20)
    }

    pub fn card_width(&self) -> usize {
        match self.class {
            ScreenClass::Tiny => self.width.min(42),
            ScreenClass::Narrow => self.width.min(64),
            ScreenClass::Normal => 72.min(self.width),
            ScreenClass::Wide => 88.min(self.width),
        }
        .max(28)
    }

    pub fn is_tiny(&self) -> bool {
        self.class == ScreenClass::Tiny
    }

    pub fn is_wide(&self) -> bool {
        self.class == ScreenClass::Wide
    }
}

pub fn clear_screen() {
    if io::stdout().is_terminal() {
        print!("\x1b[2J\x1b[H");
        let _ = io::stdout().flush();
    }
}

pub fn visible_width(s: &str) -> usize {
    let bytes = strip_ansi_escapes::strip(s);
    let clean = String::from_utf8(bytes).unwrap_or_else(|_| s.to_string());
    UnicodeWidthStr::width(clean.as_str())
}

pub fn fit(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let w = visible_width(s);

    if w == width {
        s.to_string()
    } else if w < width {
        format!("{s}{}", " ".repeat(width - w))
    } else {
        let clean =
            String::from_utf8(strip_ansi_escapes::strip(s)).unwrap_or_else(|_| s.to_string());
        let mut out = String::new();
        let mut used = 0usize;

        for ch in clean.chars() {
            let cw = UnicodeWidthStr::width(ch.to_string().as_str());
            if used + cw + 1 >= width {
                break;
            }
            out.push(ch);
            used += cw;
        }

        out.push('…');
        fit(&out, width)
    }
}

pub fn fit_middle(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if visible_width(s) <= width {
        return fit(s, width);
    }
    if width == 1 {
        return "…".to_string();
    }
    let clean = String::from_utf8(strip_ansi_escapes::strip(s)).unwrap_or_else(|_| s.to_string());
    let content_width = width - 1;
    let left_width = content_width / 3;
    let right_width = content_width - left_width;
    let left = take_width_from_start(&clean, left_width);
    let right = take_width_from_end(&clean, right_width);
    format!("{left}…{right}")
}

fn take_width_from_start(text: &str, width: usize) -> String {
    let mut used = 0;
    let mut output = String::new();
    for character in text.chars() {
        let character_width = UnicodeWidthStr::width(character.to_string().as_str());
        if used + character_width > width {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output
}

fn take_width_from_end(text: &str, width: usize) -> String {
    let mut used = 0;
    let mut output = Vec::new();
    for character in text.chars().rev() {
        let character_width = UnicodeWidthStr::width(character.to_string().as_str());
        if used + character_width > width {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output.into_iter().rev().collect()
}

pub fn wrap_lines(text: &str, width: usize) -> Vec<String> {
    textwrap::wrap(text, width)
        .into_iter()
        .map(|x| x.to_string())
        .collect()
}

pub fn line(ch: char, width: usize) -> String {
    ch.to_string().repeat(width)
}

pub fn center(text: &str, width: usize) -> String {
    let w = visible_width(text);
    if w >= width {
        fit(text, width)
    } else {
        let left = (width - w) / 2;
        let right = width - w - left;
        format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
    }
}

pub fn kv_line(key: &str, value: &str, width: usize) -> String {
    let key_w = 10usize.min(width / 3).max(6);
    let rest = width.saturating_sub(key_w + 1);
    format!("{} {}", fit(key, key_w), fit(value, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_short() {
        assert_eq!(fit("mew", 5), "mew  ");
    }

    #[test]
    fn fit_long() {
        assert_eq!(visible_width(&fit("hello world", 5)), 5);
    }

    #[test]
    fn fit_handles_zero_and_wide_glyph_widths() {
        assert_eq!(fit("mew", 0), "");
        assert_eq!(visible_width(&fit("にゃー", 2)), 2);
    }

    #[test]
    fn fit_middle_keeps_the_path_tail_and_respects_width() {
        let result = fit_middle("/home/user/very/long/project", 14);
        assert!(result.starts_with("/ho"));
        assert!(result.ends_with("project"));
        assert_eq!(visible_width(&result), 14);
    }

    #[test]
    fn fit_never_returns_a_partial_ansi_escape() {
        let result = fit("\x1b[31mabcdef\x1b[0m", 4);
        assert_eq!(visible_width(&result), 4);
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn wrap_basic() {
        assert!(!wrap_lines("tiny paws serious patches", 8).is_empty());
    }
}

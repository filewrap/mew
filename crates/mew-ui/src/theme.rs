use owo_colors::{OwoColorize, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Plain,
    Dim,
    Black,
    BrightBlack,
    Red,
    BrightRed,
    Green,
    BrightGreen,
    Yellow,
    BrightYellow,
    Blue,
    BrightBlue,
    Magenta,
    BrightMagenta,
    Cyan,
    BrightCyan,
    White,
    BrightWhite,
}

impl Palette {
    fn style(self) -> Style {
        match self {
            Palette::Plain => Style::new(),
            Palette::Dim => Style::new().dimmed(),
            Palette::Black => Style::new().black(),
            Palette::BrightBlack => Style::new().bright_black(),
            Palette::Red => Style::new().red(),
            Palette::BrightRed => Style::new().bright_red(),
            Palette::Green => Style::new().green(),
            Palette::BrightGreen => Style::new().bright_green(),
            Palette::Yellow => Style::new().yellow(),
            Palette::BrightYellow => Style::new().bright_yellow(),
            Palette::Blue => Style::new().blue(),
            Palette::BrightBlue => Style::new().bright_blue(),
            Palette::Magenta => Style::new().magenta(),
            Palette::BrightMagenta => Style::new().bright_magenta(),
            Palette::Cyan => Style::new().cyan(),
            Palette::BrightCyan => Style::new().bright_cyan(),
            Palette::White => Style::new().white(),
            Palette::BrightWhite => Style::new().bright_white(),
        }
    }

    pub fn paint(self, s: impl AsRef<str>) -> String {
        let s = s.as_ref();
        s.style(self.style()).to_string()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub border: Palette,
    pub primary: Palette,
    pub accent: Palette,
    pub dim: Palette,
    pub h1: Palette,
    pub h2: Palette,
    pub h3: Palette,
    pub ok: Palette,
    pub warn: Palette,
    pub err: Palette,
    pub bold_primary: bool,
    pub bold_headers: bool,
}

impl Theme {
    pub fn border(&self, s: impl AsRef<str>) -> String {
        if self.border == Palette::BrightBlack {
            Palette::Dim.paint(s)
        } else {
            self.border.paint(s)
        }
    }

    pub fn primary(&self, s: impl AsRef<str>) -> String {
        let s = s.as_ref();
        let st = self.primary.style();
        if self.bold_primary {
            s.style(st.bold()).to_string()
        } else {
            s.style(st).to_string()
        }
    }

    pub fn accent(&self, s: impl AsRef<str>) -> String {
        self.accent.paint(s)
    }

    pub fn dim(&self, s: impl AsRef<str>) -> String {
        if self.dim == Palette::BrightBlack {
            Palette::Dim.paint(s)
        } else {
            self.dim.paint(s)
        }
    }

    fn header(&self, p: Palette, s: impl AsRef<str>) -> String {
        let s = s.as_ref();
        let st = p.style();
        if self.bold_headers {
            s.style(st.bold()).to_string()
        } else {
            s.style(st).to_string()
        }
    }

    pub fn h1(&self, s: impl AsRef<str>) -> String {
        self.header(self.h1, s)
    }

    pub fn h2(&self, s: impl AsRef<str>) -> String {
        self.header(self.h2, s)
    }

    pub fn h3(&self, s: impl AsRef<str>) -> String {
        self.header(self.h3, s)
    }

    pub fn bullet(&self, s: impl AsRef<str>) -> String {
        self.accent.paint(s)
    }

    pub fn quote(&self, s: impl AsRef<str>) -> String {
        self.dim.paint(s)
    }

    pub fn ok(&self, s: impl AsRef<str>) -> String {
        self.ok.paint(s)
    }

    pub fn warn(&self, s: impl AsRef<str>) -> String {
        self.warn.paint(s)
    }

    pub fn err(&self, s: impl AsRef<str>) -> String {
        self.err.paint(s)
    }
}

pub fn theme_by_name(name: &str) -> Theme {
    match name {
        "claude-minimal" => Theme {
            name: "claude-minimal",
            border: Palette::BrightBlack,
            primary: Palette::BrightWhite,
            accent: Palette::BrightBlue,
            dim: Palette::BrightBlack,
            h1: Palette::BrightWhite,
            h2: Palette::BrightBlue,
            h3: Palette::BrightCyan,
            ok: Palette::BrightGreen,
            warn: Palette::BrightYellow,
            err: Palette::BrightRed,
            bold_primary: true,
            bold_headers: true,
        },
        "vector-mocha" => Theme {
            name: "vector-mocha",
            border: Palette::BrightBlack,
            primary: Palette::BrightYellow,
            accent: Palette::BrightMagenta,
            dim: Palette::BrightBlack,
            h1: Palette::BrightYellow,
            h2: Palette::BrightMagenta,
            h3: Palette::BrightCyan,
            ok: Palette::BrightGreen,
            warn: Palette::BrightYellow,
            err: Palette::BrightRed,
            bold_primary: false,
            bold_headers: true,
        },
        "crush-rose" => Theme {
            name: "crush-rose",
            border: Palette::BrightBlack,
            primary: Palette::BrightMagenta,
            accent: Palette::BrightRed,
            dim: Palette::BrightBlack,
            h1: Palette::BrightMagenta,
            h2: Palette::BrightRed,
            h3: Palette::BrightYellow,
            ok: Palette::BrightGreen,
            warn: Palette::BrightYellow,
            err: Palette::BrightRed,
            bold_primary: false,
            bold_headers: true,
        },
        "mew-dark" => Theme {
            name: "mew-dark",
            border: Palette::BrightBlack,
            primary: Palette::BrightWhite,
            accent: Palette::BrightCyan,
            dim: Palette::BrightBlack,
            h1: Palette::BrightWhite,
            h2: Palette::BrightCyan,
            h3: Palette::BrightBlue,
            ok: Palette::BrightGreen,
            warn: Palette::BrightYellow,
            err: Palette::BrightRed,
            bold_primary: false,
            bold_headers: false,
        },
        "mew-cave" => Theme {
            name: "mew-cave",
            border: Palette::BrightBlack,
            primary: Palette::BrightWhite,
            accent: Palette::BrightYellow,
            dim: Palette::BrightBlack,
            h1: Palette::BrightWhite,
            h2: Palette::BrightYellow,
            h3: Palette::BrightWhite,
            ok: Palette::BrightGreen,
            warn: Palette::BrightYellow,
            err: Palette::BrightRed,
            bold_primary: false,
            bold_headers: false,
        },
        "mono" => Theme {
            name: "mono",
            border: Palette::White,
            primary: Palette::BrightWhite,
            accent: Palette::White,
            dim: Palette::BrightBlack,
            h1: Palette::BrightWhite,
            h2: Palette::White,
            h3: Palette::White,
            ok: Palette::White,
            warn: Palette::White,
            err: Palette::White,
            bold_primary: true,
            bold_headers: true,
        },
        "no-color" => Theme {
            name: "no-color",
            border: Palette::Plain,
            primary: Palette::Plain,
            accent: Palette::Plain,
            dim: Palette::Plain,
            h1: Palette::Plain,
            h2: Palette::Plain,
            h3: Palette::Plain,
            ok: Palette::Plain,
            warn: Palette::Plain,
            err: Palette::Plain,
            bold_primary: false,
            bold_headers: false,
        },
        _ => crush_catppuccin(),
    }
}

fn crush_catppuccin() -> Theme {
    Theme {
        name: "crush-catppuccin",
        border: Palette::BrightBlack,
        primary: Palette::BrightCyan,
        accent: Palette::BrightMagenta,
        dim: Palette::BrightBlack,
        h1: Palette::BrightCyan,
        h2: Palette::BrightMagenta,
        h3: Palette::BrightBlue,
        ok: Palette::BrightGreen,
        warn: Palette::BrightYellow,
        err: Palette::BrightRed,
        bold_primary: false,
        bold_headers: true,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeInfo {
    pub name: &'static str,
    pub description: &'static str,
}

pub const THEMES: &[ThemeInfo] = &[
    ThemeInfo {
        name: "mew-dark",
        description: "high-contrast white and cyan for dark terminals",
    },
    ThemeInfo {
        name: "claude-minimal",
        description: "quiet, high-contrast minimal palette",
    },
    ThemeInfo {
        name: "vector-mocha",
        description: "warm mocha with bright vector accents",
    },
    ThemeInfo {
        name: "crush-rose",
        description: "rose and magenta accents",
    },
    ThemeInfo {
        name: "crush-catppuccin",
        description: "soft catppuccin colors with a rich CLI shell",
    },
    ThemeInfo {
        name: "mew-cave",
        description: "plain, compact caveman palette",
    },
    ThemeInfo {
        name: "mono",
        description: "monochrome terminal output",
    },
    ThemeInfo {
        name: "no-color",
        description: "disable ANSI styling",
    },
];

pub fn theme_exists(name: &str) -> bool {
    THEMES.iter().any(|theme| theme.name == name)
}

pub fn default_theme() -> Theme {
    theme_by_name("mew-dark")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_mew_dark_with_bright_primary_text() {
        let theme = default_theme();
        assert_eq!(theme.name, "mew-dark");
        assert_eq!(theme.primary, Palette::BrightWhite);
        assert_eq!(theme.accent, Palette::BrightCyan);
    }

    #[test]
    fn bright_black_muted_roles_are_rendered_as_terminal_relative_dim() {
        let theme = theme_by_name("crush-catppuccin");
        assert_eq!(theme.border, Palette::BrightBlack);
        assert_eq!(theme.dim, Palette::BrightBlack);
        assert!(!theme.border("│").contains("[90m"));
        assert!(!theme.dim("ready to send").contains("[90m"));
    }
}

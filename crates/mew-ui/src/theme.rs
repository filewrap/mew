use owo_colors::{OwoColorize, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Plain,
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

    pub fn paint(self, s: &str) -> String {
        self.style().paint(s).to_string()
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
    pub fn border(&self, s: &str) -> String {
        self.border.paint(s)
    }

    pub fn primary(&self, s: &str) -> String {
        let st = self.primary.style();
        if self.bold_primary {
            st.bold().paint(s).to_string()
        } else {
            st.paint(s)
        }
    }

    pub fn accent(&self, s: &str) -> String {
        self.accent.paint(s)
    }

    pub fn dim(&self, s: &str) -> String {
        self.dim.paint(s)
    }

    fn header(&self, p: Palette, s: &str) -> String {
        let st = p.style();
        if self.bold_headers {
            st.bold().paint(s).to_string()
        } else {
            st.paint(s)
        }
    }

    pub fn h1(&self, s: &str) -> String {
        self.header(self.h1, s)
    }

    pub fn h2(&self, s: &str) -> String {
        self.header(self.h2, s)
    }

    pub fn h3(&self, s: &str) -> String {
        self.header(self.h3, s)
    }

    pub fn bullet(&self, s: &str) -> String {
        self.accent.paint(s)
    }

    pub fn quote(&self, s: &str) -> String {
        self.dim.paint(s)
    }

    pub fn ok(&self, s: &str) -> String {
        self.ok.paint(s)
    }

    pub fn warn(&self, s: &str) -> String {
        self.warn.paint(s)
    }

    pub fn err(&self, s: &str) -> String {
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
            primary: Palette::BrightCyan,
            accent: Palette::BrightBlue,
            dim: Palette::BrightBlack,
            h1: Palette::BrightCyan,
            h2: Palette::BrightBlue,
            h3: Palette::BrightWhite,
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

pub fn default_theme() -> Theme {
    crush_catppuccin()
}

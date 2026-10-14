use crate::shell::{render_chat_shell, ChatShellFrame};
use crate::theme::Theme;
use crossterm::cursor::{MoveLeft, MoveTo, MoveToColumn};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Read one chat prompt with history and slash-command completion when attached
/// to a terminal. Piped input keeps the ordinary line-oriented behavior.
pub fn read_prompt_line(
    prompt: &str,
    history: &[String],
    commands: &[String],
) -> io::Result<Option<String>> {
    read_prompt_line_with_animation(prompt, history, commands, true)
}

/// Read a chat prompt while respecting the user's animation preference.
pub fn read_prompt_line_with_animation(
    prompt: &str,
    history: &[String],
    commands: &[String],
    animations: bool,
) -> io::Result<Option<String>> {
    match read_prompt_event_with_keybindings(
        prompt,
        history,
        commands,
        animations,
        &BTreeMap::new(),
    )? {
        PromptEvent::Submitted(value) => Ok(Some(value)),
        PromptEvent::OpenCommandPalette => Ok(Some("/".into())),
        PromptEvent::Cancelled => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptEvent {
    Submitted(String),
    OpenCommandPalette,
    Cancelled,
}

/// Read a prompt using persisted, action-keyed terminal keybindings.
pub fn read_prompt_event_with_keybindings(
    prompt: &str,
    history: &[String],
    commands: &[String],
    animations: bool,
    keybindings: &BTreeMap<String, String>,
) -> io::Result<PromptEvent> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        print!("{prompt}");
        io::stdout().flush()?;
        let mut input = String::new();
        return if io::stdin().read_line(&mut input)? == 0 {
            Ok(PromptEvent::Cancelled)
        } else {
            Ok(PromptEvent::Submitted(
                input.trim_end_matches(['\r', '\n']).to_string(),
            ))
        };
    }
    read_interactive_line(prompt, history, commands, None, animations, keybindings)
}

/// Read a chat prompt inside the full-screen shell on an interactive terminal.
/// Non-TTY input deliberately keeps the line-oriented behavior used by scripts.
pub fn read_chat_shell_line(
    prompt: &str,
    history: &[String],
    commands: &[String],
    theme: &Theme,
    shell: &ChatShellFrame,
) -> io::Result<Option<String>> {
    match read_chat_shell_event(prompt, history, commands, theme, shell, &BTreeMap::new())? {
        PromptEvent::Submitted(value) => Ok(Some(value)),
        PromptEvent::OpenCommandPalette => Ok(Some("/".into())),
        PromptEvent::Cancelled => Ok(None),
    }
}

/// Full-screen chat input with configurable keybindings and an explicit palette event.
pub fn read_chat_shell_event(
    prompt: &str,
    history: &[String],
    commands: &[String],
    theme: &Theme,
    shell: &ChatShellFrame,
    keybindings: &BTreeMap<String, String>,
) -> io::Result<PromptEvent> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return read_prompt_event_with_keybindings(
            prompt,
            history,
            commands,
            shell.animations,
            keybindings,
        );
    }
    read_interactive_line(
        prompt,
        history,
        commands,
        Some((theme, shell)),
        shell.animations,
        keybindings,
    )
}

/// Own the alternate screen for the lifetime of a full-screen chat session.
/// Raw mode stays scoped to prompt reads so Ctrl+C can cancel work between prompts.
pub struct FullscreenSession;

impl FullscreenSession {
    pub fn enter() -> io::Result<Self> {
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableBracketedPaste,
            crossterm::cursor::Hide,
            Clear(ClearType::All)
        )?;
        if let Ok(mut previous) = snapshot_cache().lock() {
            *previous = None;
        }
        Ok(Self)
    }
}

impl Drop for FullscreenSession {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            DisableBracketedPaste,
            crossterm::cursor::Show,
            LeaveAlternateScreen
        );
        if let Ok(mut previous) = snapshot_cache().lock() {
            *previous = None;
        }
    }
}

fn snapshot_cache() -> &'static Mutex<Option<(usize, usize, Vec<String>)>> {
    static CACHE: OnceLock<Mutex<Option<(usize, usize, Vec<String>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Redraw a stable full-screen shell frame outside the input loop. The chat
/// runtime uses this for request progress, cancellation, and error states.
pub fn draw_chat_shell_snapshot(theme: &Theme, shell: &ChatShellFrame) -> io::Result<()> {
    let (columns, rows) = terminal::size().unwrap_or((80, 24));
    let input = InputBuffer::default();
    let animation_frame = crate::miku::animation_frame_now();
    let rendered = render_chat_shell(
        theme,
        shell,
        &input,
        animation_frame,
        usize::from(columns),
        usize::from(rows),
        0,
    );
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::cursor::Hide)?;
    let mut previous = snapshot_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let size_changed = previous.as_ref().is_none_or(|(old_width, old_height, _)| {
        *old_width != usize::from(columns) || *old_height != usize::from(rows)
    });
    if size_changed {
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        *previous = None;
    }
    for (row, text) in rendered.lines.iter().enumerate() {
        if previous
            .as_ref()
            .and_then(|(_, _, old_rows)| old_rows.get(row))
            != Some(text)
        {
            execute!(
                stdout,
                MoveTo(0, row.min(u16::MAX as usize) as u16),
                Clear(ClearType::CurrentLine)
            )?;
            write!(stdout, "{text}")?;
        }
    }
    *previous = Some((usize::from(columns), usize::from(rows), rendered.lines));
    execute!(stdout, MoveTo(rendered.cursor.0, rendered.cursor.1))?;
    stdout.flush()
}

fn read_interactive_line(
    prompt: &str,
    history: &[String],
    commands: &[String],
    shell: Option<(&Theme, &ChatShellFrame)>,
    animations: bool,
    keybindings: &BTreeMap<String, String>,
) -> io::Result<PromptEvent> {
    let _raw_mode = RawMode::enable()?;
    let mut stdout = io::stdout();
    let mut input = InputBuffer::default();
    let mut history_position = history.len();
    let mut history_draft = String::new();
    let mut frame = crate::miku::animation_frame_now();
    let mut transcript_scroll = 0usize;
    let mut previous_shell_rows = None;
    let mut live_prompt = miku_prompt(prompt, frame);
    redraw_interactive(
        &live_prompt,
        &input,
        &mut stdout,
        shell,
        frame,
        transcript_scroll,
        &mut previous_shell_rows,
    )?;

    loop {
        if !event::poll(Duration::from_millis(180))? {
            if animations {
                frame = crate::miku::animation_frame_now();
                live_prompt = miku_prompt(prompt, frame);
                redraw_interactive(
                    &live_prompt,
                    &input,
                    &mut stdout,
                    shell,
                    frame,
                    transcript_scroll,
                    &mut previous_shell_rows,
                )?;
            }
            continue;
        }
        let key = match event::read()? {
            Event::Paste(text) => {
                input.insert_text(&text, shell.is_some());
                history_position = history.len();
                frame = crate::miku::animation_frame_now();
                live_prompt = miku_prompt(prompt, frame);
                redraw_interactive(
                    &live_prompt,
                    &input,
                    &mut stdout,
                    shell,
                    frame,
                    transcript_scroll,
                    &mut previous_shell_rows,
                )?;
                continue;
            }
            Event::Resize(_, _) => {
                frame = crate::miku::animation_frame_now();
                redraw_interactive(
                    &live_prompt,
                    &input,
                    &mut stdout,
                    shell,
                    frame,
                    transcript_scroll,
                    &mut previous_shell_rows,
                )?;
                continue;
            }
            Event::Key(key) => key,
            _ => continue,
        };
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }

        let is_bound = |action: &str, default: &str| {
            let binding = keybindings
                .get(action)
                .map(String::as_str)
                .unwrap_or(default);
            key_matches_action_binding(key.code, key.modifiers, action, binding, default)
        };
        if shell.is_some()
            && key.code == KeyCode::Enter
            && (key.modifiers.contains(KeyModifiers::SHIFT)
                || key.modifiers.contains(KeyModifiers::ALT))
            && !is_bound("submit", "enter")
            && !is_bound("command_palette", "ctrl+p")
            && !is_bound("cancel", "ctrl+c")
        {
            input.insert('\n');
            history_position = history.len();
        } else if is_instant_command_trigger(&input, key.code, key.modifiers) {
            execute!(stdout, crossterm::style::Print("\r\n"))?;
            return Ok(PromptEvent::OpenCommandPalette);
        } else if is_bound("command_palette", "ctrl+p") {
            execute!(stdout, crossterm::style::Print("\r\n"))?;
            return Ok(PromptEvent::OpenCommandPalette);
        } else if is_bound("submit", "enter") {
            execute!(stdout, crossterm::style::Print("\r\n"))?;
            return Ok(PromptEvent::Submitted(input.into_string()));
        } else if is_bound("cancel", "ctrl+c") {
            execute!(stdout, crossterm::style::Print("\r\n"))?;
            return Ok(PromptEvent::Cancelled);
        } else if is_bound("eof", "ctrl+d") && input.is_empty() {
            execute!(stdout, crossterm::style::Print("\r\n"))?;
            return Ok(PromptEvent::Cancelled);
        } else if is_bound("move_home", "ctrl+a") {
            input.move_home();
        } else if key.code == KeyCode::Home {
            input.move_line_home();
        } else if is_bound("move_end", "ctrl+e") {
            input.move_end();
        } else if key.code == KeyCode::End {
            input.move_line_end();
        } else if is_bound("clear_line", "ctrl+u") {
            input.clear();
            history_position = history.len();
        } else if is_bound("delete_word", "ctrl+w") {
            input.delete_word_before_cursor();
            history_position = history.len();
        } else if is_bound("move_left", "left") {
            input.move_left();
        } else if is_bound("move_right", "right") {
            input.move_right();
        } else if is_bound("transcript_up", "pageup") && shell.is_some() {
            transcript_scroll = transcript_scroll.saturating_add(4);
        } else if is_bound("transcript_down", "pagedown") && shell.is_some() {
            transcript_scroll = transcript_scroll.saturating_sub(4);
        } else if is_bound("backspace", "backspace") {
            input.backspace();
            history_position = history.len();
        } else if is_bound("delete", "delete") {
            input.delete();
            history_position = history.len();
        } else if shell.is_some() && input.has_newline() && key.code == KeyCode::Up {
            input.move_vertical(-1);
        } else if shell.is_some() && input.has_newline() && key.code == KeyCode::Down {
            input.move_vertical(1);
        } else if is_bound("history_previous", "up") && !history.is_empty() {
            if history_position == history.len() {
                history_draft = input.value().to_string();
            }
            if history_position > 0 {
                history_position -= 1;
                input.set_value(history[history_position].clone());
            }
        } else if is_bound("history_next", "down") && history_position < history.len() {
            history_position += 1;
            if history_position == history.len() {
                input.set_value(history_draft.clone());
            } else {
                input.set_value(history[history_position].clone());
            }
        } else if is_bound("complete", "tab") {
            let previous = input.value().to_string();
            let candidates = input.complete_command(commands);
            if input.value() != previous {
                history_position = history.len();
            }
            if candidates.len() > 1 && input.value() == previous {
                write!(stdout, "\r\n  {}\r\n", candidates.join("  "))?;
                previous_shell_rows = None;
            }
        } else if key.modifiers.is_empty() {
            if let KeyCode::Char(character) = key.code {
                if !character.is_control() {
                    input.insert(character);
                    history_position = history.len();
                }
            }
        }
        frame = crate::miku::animation_frame_now();
        live_prompt = miku_prompt(prompt, frame);
        redraw_interactive(
            &live_prompt,
            &input,
            &mut stdout,
            shell,
            frame,
            transcript_scroll,
            &mut previous_shell_rows,
        )?;
    }
}

fn redraw_interactive(
    prompt: &str,
    input: &InputBuffer,
    stdout: &mut io::Stdout,
    shell: Option<(&Theme, &ChatShellFrame)>,
    animation_frame: usize,
    transcript_scroll: usize,
    previous_shell_rows: &mut Option<Vec<String>>,
) -> io::Result<()> {
    if let Some((theme, shell)) = shell {
        let (columns, rows) = terminal::size().unwrap_or((80, 24));
        let rendered = render_chat_shell(
            theme,
            shell,
            input,
            animation_frame,
            usize::from(columns),
            usize::from(rows),
            transcript_scroll,
        );
        if previous_shell_rows.is_none() {
            execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        }
        for (row, text) in rendered.lines.iter().enumerate() {
            if previous_shell_rows
                .as_ref()
                .and_then(|previous| previous.get(row))
                != Some(text)
            {
                execute!(
                    stdout,
                    MoveTo(0, row.min(u16::MAX as usize) as u16),
                    Clear(ClearType::CurrentLine)
                )?;
                write!(stdout, "{text}")?;
            }
        }
        *previous_shell_rows = Some(rendered.lines);
        execute!(
            stdout,
            MoveTo(rendered.cursor.0, rendered.cursor.1),
            crossterm::cursor::Show
        )?;
        return stdout.flush();
    }
    redraw(prompt, input, stdout)
}

fn key_matches_binding(code: KeyCode, modifiers: KeyModifiers, binding: &str) -> bool {
    binding
        .split(',')
        .map(str::trim)
        .any(|chord| key_matches_chord(code, modifiers, chord))
}

fn key_matches_action_binding(
    code: KeyCode,
    modifiers: KeyModifiers,
    action: &str,
    binding: &str,
    default: &str,
) -> bool {
    key_matches_binding(code, modifiers, binding)
        || (binding.eq_ignore_ascii_case(default)
            && default_function_key(action)
                .is_some_and(|function_key| key_matches_binding(code, modifiers, function_key)))
}

fn default_function_key(action: &str) -> Option<&'static str> {
    match action {
        "command_palette" => Some("f1"),
        "complete" => Some("f2"),
        _ => None,
    }
}

fn is_instant_command_trigger(input: &InputBuffer, code: KeyCode, modifiers: KeyModifiers) -> bool {
    input.is_empty() && code == KeyCode::Char('/') && modifiers.is_empty()
}

fn key_matches_chord(code: KeyCode, modifiers: KeyModifiers, binding: &str) -> bool {
    let parts = binding
        .to_ascii_lowercase()
        .split('+')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let Some(key) = parts.last().map(String::as_str) else {
        return false;
    };
    let wants_control = parts.iter().any(|part| part == "ctrl" || part == "control");
    let wants_alt = parts.iter().any(|part| part == "alt");
    let wants_shift = parts.iter().any(|part| part == "shift");
    if modifiers.contains(KeyModifiers::CONTROL) != wants_control
        || modifiers.contains(KeyModifiers::ALT) != wants_alt
        || modifiers.contains(KeyModifiers::SHIFT) != wants_shift
    {
        return false;
    }
    match key {
        "enter" | "return" => code == KeyCode::Enter,
        "esc" | "escape" => code == KeyCode::Esc,
        "tab" => code == KeyCode::Tab,
        "up" => code == KeyCode::Up,
        "down" => code == KeyCode::Down,
        "left" => code == KeyCode::Left,
        "right" => code == KeyCode::Right,
        "home" => code == KeyCode::Home,
        "end" => code == KeyCode::End,
        "pageup" => code == KeyCode::PageUp,
        "pagedown" => code == KeyCode::PageDown,
        "backspace" => code == KeyCode::Backspace,
        "delete" => code == KeyCode::Delete,
        "space" => code == KeyCode::Char(' '),
        value if value.starts_with('f') => value[1..]
            .parse::<u8>()
            .ok()
            .filter(|number| (1..=24).contains(number))
            .is_some_and(|number| code == KeyCode::F(number)),
        value if value.chars().count() == 1 => {
            code == KeyCode::Char(value.chars().next().unwrap_or_default())
        }
        _ => false,
    }
}

fn miku_prompt(prompt: &str, frame: usize) -> String {
    let full_prompt = format!(
        "{} Miku · {}",
        crate::miku::miku_expression(crate::miku::MikuMotion::Idle, frame),
        prompt
    );
    let columns = terminal::size()
        .map(|(columns, _)| columns as usize)
        .unwrap_or(80);
    if crate::layout::visible_width(&full_prompt).saturating_add(1) >= columns {
        prompt.to_string()
    } else {
        full_prompt
    }
}

fn redraw(prompt: &str, input: &InputBuffer, stdout: &mut io::Stdout) -> io::Result<()> {
    let columns = terminal::size()
        .map(|(columns, _)| columns as usize)
        .unwrap_or(80);
    let available = columns
        .saturating_sub(crate::layout::visible_width(prompt))
        .max(1);
    let (visible, cursor_column, suffix_width) = input.visible_window(available);
    execute!(stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
    write!(stdout, "{prompt}{visible}")?;
    if suffix_width > 0 {
        execute!(stdout, MoveLeft(suffix_width.min(u16::MAX as usize) as u16))?;
    }
    let _ = cursor_column;
    stdout.flush()
}

struct RawMode;

impl RawMode {
    fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InputBuffer {
    value: String,
    cursor: usize,
    preferred_column: Option<usize>,
}

impl InputBuffer {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn has_newline(&self) -> bool {
        self.value.contains('\n')
    }

    pub fn insert(&mut self, character: char) {
        self.preferred_column = None;
        self.value.insert(self.cursor, character);
        self.cursor += character.len_utf8();
        self.snap_cursor_forward();
    }

    pub fn insert_text(&mut self, text: &str, multiline: bool) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        for character in normalized.chars() {
            match character {
                '\n' if multiline => self.insert('\n'),
                '\n' | '\t' => self.insert(' '),
                character if !character.is_control() => self.insert(character),
                _ => {}
            }
        }
    }

    pub fn backspace(&mut self) {
        self.preferred_column = None;
        if let Some((start, _)) = self.value[..self.cursor].grapheme_indices(true).last() {
            self.value.replace_range(start..self.cursor, "");
            self.cursor = start;
            self.snap_cursor_forward();
        }
    }

    pub fn delete(&mut self) {
        self.preferred_column = None;
        if let Some((_, grapheme)) = self.value[self.cursor..].grapheme_indices(true).next() {
            let end = self.cursor + grapheme.len();
            self.value.replace_range(self.cursor..end, "");
            self.snap_cursor_forward();
        }
    }

    pub fn move_left(&mut self) {
        self.preferred_column = None;
        if let Some((start, _)) = self.value[..self.cursor].grapheme_indices(true).last() {
            self.cursor = start;
        }
    }

    pub fn move_right(&mut self) {
        self.preferred_column = None;
        if let Some((_, grapheme)) = self.value[self.cursor..].grapheme_indices(true).next() {
            self.cursor += grapheme.len();
        }
    }

    pub fn move_home(&mut self) {
        self.preferred_column = None;
        self.cursor = 0;
    }

    pub fn move_line_home(&mut self) {
        self.preferred_column = None;
        self.cursor = self.value[..self.cursor]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
    }

    pub fn move_end(&mut self) {
        self.preferred_column = None;
        self.cursor = self.value.len();
    }

    pub fn move_line_end(&mut self) {
        self.preferred_column = None;
        self.cursor += self.value[self.cursor..]
            .find('\n')
            .unwrap_or(self.value.len() - self.cursor);
    }

    pub fn move_vertical(&mut self, direction: isize) {
        let current_start = self.value[..self.cursor]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let current_end = self.cursor
            + self.value[self.cursor..]
                .find('\n')
                .unwrap_or(self.value.len() - self.cursor);
        let current_column = UnicodeWidthStr::width(&self.value[current_start..self.cursor]);
        let desired_column = *self.preferred_column.get_or_insert(current_column);
        let (target_start, target_end) = if direction < 0 {
            let Some(previous_newline) = current_start.checked_sub(1) else {
                return;
            };
            let start = self.value[..previous_newline]
                .rfind('\n')
                .map(|index| index + 1)
                .unwrap_or(0);
            (start, previous_newline)
        } else {
            if current_end >= self.value.len() {
                return;
            }
            let start = current_end + 1;
            let end = start
                + self.value[start..]
                    .find('\n')
                    .unwrap_or(self.value.len() - start);
            (start, end)
        };
        let mut cursor = target_start;
        let mut column = 0usize;
        for (offset, grapheme) in self.value[target_start..target_end].grapheme_indices(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if column + grapheme_width > desired_column {
                break;
            }
            column += grapheme_width;
            cursor = target_start + offset + grapheme.len();
        }
        self.cursor = cursor;
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
        self.preferred_column = None;
    }

    pub fn set_value(&mut self, value: String) {
        self.value = value;
        self.cursor = self.value.len();
        self.preferred_column = None;
    }

    fn snap_cursor_forward(&mut self) {
        let cursor = self.cursor;
        let next_boundary = self
            .value
            .grapheme_indices(true)
            .find_map(|(start, grapheme)| {
                let end = start + grapheme.len();
                (start < cursor && cursor < end).then_some(end)
            });
        if let Some(next_boundary) = next_boundary {
            self.cursor = next_boundary;
        }
    }

    pub fn delete_word_before_cursor(&mut self) {
        self.preferred_column = None;
        let before = &self.value[..self.cursor];
        let mut word_start = 0;
        for (index, grapheme) in before.grapheme_indices(true) {
            if grapheme
                .chars()
                .next()
                .is_some_and(|character| character.is_whitespace())
            {
                word_start = index + grapheme.len();
            }
        }
        self.value.replace_range(word_start..self.cursor, "");
        self.cursor = word_start;
        self.snap_cursor_forward();
    }

    /// Complete a slash-command prefix and return matching candidates for a
    /// compact suggestion row when the prefix is still ambiguous.
    pub fn complete_command(&mut self, commands: &[String]) -> Vec<String> {
        if self.cursor != self.value.len()
            || !self.value.starts_with('/')
            || self.value.chars().any(char::is_whitespace)
        {
            return Vec::new();
        }
        let matches = commands
            .iter()
            .filter(|command| command.starts_with(&self.value))
            .cloned()
            .collect::<Vec<_>>();
        if matches.iter().any(|command| command == &self.value) {
            return vec![self.value.clone()];
        }
        if matches.len() == 1 {
            self.set_value(matches[0].clone());
        } else if matches.len() > 1 {
            let common = common_prefix(&matches);
            if common.len() > self.value.len() {
                self.set_value(common);
            }
        }
        matches
    }

    fn into_string(self) -> String {
        self.value
    }

    pub(crate) fn visible_window(&self, width: usize) -> (String, usize, usize) {
        let graphemes = self.value.graphemes(true).collect::<Vec<_>>();
        let widths = graphemes
            .iter()
            .map(|grapheme| UnicodeWidthStr::width(*grapheme))
            .collect::<Vec<_>>();
        let cursor_index = self.value[..self.cursor].graphemes(true).count();
        let total_width = widths.iter().copied().sum::<usize>();
        if total_width <= width {
            let before = widths_for(&widths, 0, cursor_index);
            let after = widths_for(&widths, cursor_index, widths.len());
            return (self.value.clone(), before, after);
        }

        let mut start = cursor_index;
        let mut left_width = 0usize;
        let right_budget = widths[cursor_index..].iter().sum::<usize>().min(width / 2);
        let left_budget = width.saturating_sub(right_budget);
        while start > 0 {
            let grapheme_width = widths[start - 1];
            if left_width + grapheme_width > left_budget {
                break;
            }
            start -= 1;
            left_width += grapheme_width;
        }

        let mut end = cursor_index;
        let mut right_width = 0usize;
        while end < graphemes.len() {
            let grapheme_width = widths[end];
            if left_width + right_width + grapheme_width > width {
                break;
            }
            end += 1;
            right_width += grapheme_width;
        }

        if start == end && end < graphemes.len() && width > 0 {
            let grapheme_width = widths[end];
            if grapheme_width <= width {
                end += 1;
            }
        }
        let visible = graphemes[start..end].concat();
        let suffix = widths_for(&widths, cursor_index.min(end), end);
        (visible, left_width, suffix)
    }

    pub(crate) fn visible_lines(
        &self,
        width: usize,
        height: usize,
    ) -> (Vec<String>, (usize, usize)) {
        if height == 0 {
            return (Vec::new(), (0, 0));
        }
        let mut starts = vec![0usize];
        for (index, character) in self.value.char_indices() {
            if character == '\n' {
                starts.push(index + character.len_utf8());
            }
        }
        let cursor_line = self.value[..self.cursor]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        let first_line = cursor_line.saturating_sub(height.saturating_sub(1));
        let last_line = (first_line + height).min(starts.len());
        let mut visible = Vec::with_capacity(last_line.saturating_sub(first_line));
        let mut cursor_position = (0usize, 0usize);
        for line_index in first_line..last_line {
            let start = starts[line_index];
            let end = starts
                .get(line_index + 1)
                .map(|next| next.saturating_sub(1))
                .unwrap_or(self.value.len());
            let line_text = &self.value[start..end];
            let local_cursor = if line_index == cursor_line {
                self.cursor.saturating_sub(start).min(line_text.len())
            } else {
                line_text.len()
            };
            let line = Self {
                value: line_text.to_string(),
                cursor: local_cursor,
                preferred_column: None,
            };
            let (text, cursor_column, _) = line.visible_window(width);
            if line_index == cursor_line {
                cursor_position = (cursor_column, line_index - first_line);
            }
            visible.push(text);
        }
        (visible, cursor_position)
    }
}

fn widths_for(widths: &[usize], start: usize, end: usize) -> usize {
    widths[start..end].iter().copied().sum()
}

fn common_prefix(values: &[String]) -> String {
    let Some(first) = values.first() else {
        return String::new();
    };
    let mut prefix = first.chars().collect::<Vec<_>>();
    for value in values.iter().skip(1) {
        let candidate = value.chars().collect::<Vec<_>>();
        let shared = prefix
            .iter()
            .zip(candidate.iter())
            .take_while(|(left, right)| left == right)
            .count();
        prefix.truncate(shared);
    }
    prefix.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_buffer_edits_utf8_without_splitting_characters() {
        let mut input = InputBuffer::default();
        input.insert('c');
        input.insert('a');
        input.insert('猫');
        input.move_left();
        input.backspace();
        assert_eq!(input.value(), "c猫");
        input.move_home();
        input.delete();
        assert_eq!(input.value(), "猫");
    }

    #[test]
    fn multiline_paste_and_vertical_navigation_preserve_utf8_and_line_boundaries() {
        let mut input = InputBuffer::default();
        input.insert_text("first\r\nsecond\t猫", true);
        assert_eq!(input.value(), "first\nsecond 猫");
        input.move_vertical(-1);
        assert_eq!(&input.value()[..input.cursor], "first");
        input.move_line_end();
        input.move_vertical(1);
        assert_eq!(&input.value()[..input.cursor], "first\nsecon");
        input.move_line_home();
        assert_eq!(&input.value()[..input.cursor], "first\n");
    }

    #[test]
    fn visible_multiline_window_tracks_the_cursor_and_obeys_display_width() {
        let mut input = InputBuffer::default();
        input.set_value("line one\nline 猫 two\nthird-longer".into());
        let (visible, (cursor_x, cursor_y)) = input.visible_lines(6, 2);
        assert_eq!(visible.len(), 2);
        assert!(visible
            .iter()
            .all(|line| crate::layout::visible_width(line) <= 6));
        assert_eq!(visible.last().map(String::as_str), Some("longer"));
        assert_eq!(cursor_x, 6);
        assert_eq!(cursor_y, 1);
    }

    #[test]
    fn vertical_moves_keep_the_original_column_when_crossing_a_short_line() {
        let mut input = InputBuffer::default();
        input.set_value("abcdefgh\nxy\n12345678".into());
        input.move_vertical(-1);
        input.move_vertical(-1);
        assert_eq!(&input.value()[..input.cursor], "abcdefgh");
        input.move_vertical(1);
        assert_eq!(&input.value()[..input.cursor], "abcdefgh\nxy");
        input.move_vertical(1);
        assert_eq!(&input.value()[..input.cursor], "abcdefgh\nxy\n12345678");
    }

    #[test]
    fn cursor_navigation_and_deletion_keep_combining_and_emoji_graphemes_intact() {
        let mut input = InputBuffer::default();
        input.set_value("A👩🏽‍💻e\u{301}B".to_string());
        input.move_home();
        input.move_right();
        assert_eq!(&input.value()[..input.cursor], "A");
        input.move_right();
        assert_eq!(&input.value()[..input.cursor], "A👩🏽‍💻");
        input.delete();
        assert_eq!(input.value(), "A👩🏽‍💻B");
        input.backspace();
        assert_eq!(input.value(), "AB");

        input.set_value("e\u{301}猫".to_string());
        input.move_home();
        input.move_right();
        assert_eq!(&input.value()[..input.cursor], "e\u{301}");
        input.backspace();
        assert_eq!(input.value(), "猫");
    }

    #[test]
    fn visible_window_never_splits_a_joined_emoji_grapheme() {
        let mut input = InputBuffer::default();
        input.set_value("x👨‍👩‍👧‍👦y".to_string());
        input.move_home();
        input.move_right();
        let (visible, cursor, _) = input.visible_window(2);
        assert_eq!(visible, "x");
        assert_eq!(cursor, 1);
        assert!(crate::layout::visible_width(&visible) <= 2);
    }

    #[test]
    fn slash_completion_expands_unique_and_ambiguous_prefixes() {
        let commands = [
            "/model".to_string(),
            "/mode".to_string(),
            "/mcp".to_string(),
        ];
        let mut input = InputBuffer::default();
        input.insert('/');
        input.insert('m');
        let matches = input.complete_command(&commands);
        assert_eq!(matches.len(), 3);
        assert_eq!(input.value(), "/m");

        input.set_value("/mode".to_string());
        let one = input.complete_command(&commands);
        assert_eq!(one, vec!["/mode"]);
        assert_eq!(input.value(), "/mode");
    }

    #[test]
    fn function_keys_and_comma_separated_keybinding_aliases_match() {
        assert!(key_matches_binding(
            KeyCode::F(1),
            KeyModifiers::empty(),
            "ctrl+p,f1"
        ));
        assert!(key_matches_binding(
            KeyCode::F(12),
            KeyModifiers::SHIFT,
            "shift+f12"
        ));
        assert!(!key_matches_binding(
            KeyCode::F(0),
            KeyModifiers::empty(),
            "f0"
        ));
        assert!(key_matches_action_binding(
            KeyCode::F(1),
            KeyModifiers::empty(),
            "command_palette",
            "ctrl+p",
            "ctrl+p"
        ));
        assert!(!key_matches_action_binding(
            KeyCode::F(1),
            KeyModifiers::empty(),
            "command_palette",
            "ctrl+o",
            "ctrl+p"
        ));
    }

    #[test]
    fn instant_command_palette_only_triggers_for_a_leading_slash() {
        let empty = InputBuffer::default();
        assert!(is_instant_command_trigger(
            &empty,
            KeyCode::Char('/'),
            KeyModifiers::empty()
        ));
        assert!(!is_instant_command_trigger(
            &empty,
            KeyCode::Char('/'),
            KeyModifiers::SHIFT
        ));

        let mut started = InputBuffer::default();
        started.insert('h');
        assert!(!is_instant_command_trigger(
            &started,
            KeyCode::Char('/'),
            KeyModifiers::empty()
        ));
        assert!(!is_instant_command_trigger(
            &empty,
            KeyCode::Char('a'),
            KeyModifiers::empty()
        ));
    }

    #[test]
    fn input_window_tracks_cursor_and_respects_display_width() {
        let mut input = InputBuffer::default();
        input.set_value("a猫bcdef".to_string());
        input.move_home();
        input.move_right();
        let (visible, cursor, suffix) = input.visible_window(5);
        assert!(crate::layout::visible_width(&visible) <= 5);
        assert_eq!(cursor, 1);
        assert!(suffix > 0);
    }

    #[test]
    fn input_window_scrolls_long_input_to_keep_cursor_visible_and_fill_row() {
        let mut input = InputBuffer::default();
        input.set_value("abcdefghij".to_string());
        let (at_end, end_cursor, end_suffix) = input.visible_window(5);
        assert_eq!(at_end, "fghij");
        assert_eq!(end_cursor, 5);
        assert_eq!(end_suffix, 0);

        input.move_home();
        let (at_start, start_cursor, start_suffix) = input.visible_window(5);
        assert_eq!(at_start, "abcde");
        assert_eq!(start_cursor, 0);
        assert_eq!(start_suffix, 5);
    }
}

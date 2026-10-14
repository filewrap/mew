use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, IsTerminal, Write};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

use crate::layout::fit;
use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub label: String,
    pub description: String,
    pub value: String,
}

impl SelectOption {
    pub fn new(
        label: impl Into<String>,
        description: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            description: description.into(),
            value: value.into(),
        }
    }
}

/// Display a small searchable selector in the current terminal area.
/// Returns `None` when input is non-interactive, cancelled, or has no options.
pub fn read_searchable_selector(
    theme: &Theme,
    title: &str,
    options: &[SelectOption],
) -> io::Result<Option<String>> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() || options.is_empty() {
        return Ok(None);
    }

    let _raw_mode = SelectorRawMode::enable()?;
    let mut stdout = io::stdout();
    let mut query = String::new();
    let mut selected = 0usize;
    let mut rendered_rows = 0usize;

    loop {
        let filtered = rank_matches(&query, options);
        selected = selected.min(filtered.len().saturating_sub(1));
        let (columns, rows) = terminal::size().unwrap_or((80, 24));
        let width = (columns as usize).saturating_sub(1).max(1);
        let list_limit = (rows as usize).saturating_sub(4).clamp(1, 8);
        let start = selected.saturating_sub(list_limit.saturating_sub(1));
        let end = (start + list_limit).min(filtered.len());

        let mut lines = vec![theme.primary(fit(title, width))];
        lines.push(theme.dim(fit(&format!("Search: {query}"), width)));
        if filtered.is_empty() {
            lines.push(theme.dim(fit("No matching commands or actions", width)));
        } else {
            for (index, option) in filtered.iter().enumerate().take(end).skip(start) {
                let row = fit(
                    &format!("{} — {}", option.label, option.description),
                    width.saturating_sub(2),
                );
                lines.push(if index == selected {
                    theme.accent(fit(&format!("› {row}"), width))
                } else {
                    theme.dim(fit(&format!("  {row}"), width))
                });
            }
        }
        let count = if filtered.is_empty() {
            "0 results".to_string()
        } else {
            format!("{} / {}", selected + 1, filtered.len())
        };
        lines.push(theme.dim(fit(
            &format!("{count} · ↑/↓ move · type to search · enter choose · esc close"),
            width,
        )));

        erase_menu(&mut stdout, rendered_rows)?;
        for line in &lines {
            write!(stdout, "\r{line}\r\n")?;
        }
        rendered_rows = lines.len();
        stdout.flush()?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match (key.code, control) {
            (KeyCode::Esc, _) | (KeyCode::Char('c'), true) => {
                erase_menu(&mut stdout, rendered_rows)?;
                writeln!(stdout)?;
                stdout.flush()?;
                return Ok(None);
            }
            (KeyCode::Enter, _) => {
                let value = filtered.get(selected).map(|option| option.value.clone());
                erase_menu(&mut stdout, rendered_rows)?;
                writeln!(stdout)?;
                stdout.flush()?;
                return Ok(value);
            }
            (KeyCode::Up, _) if !filtered.is_empty() => {
                selected = selected.saturating_sub(1);
            }
            (KeyCode::Down, _) if !filtered.is_empty() => {
                selected = (selected + 1).min(filtered.len() - 1);
            }
            (KeyCode::Backspace, _) => {
                pop_last_grapheme(&mut query);
                selected = 0;
            }
            (KeyCode::Char('u'), true) => {
                query.clear();
                selected = 0;
            }
            (KeyCode::Char(character), false) if !character.is_control() => {
                query.push(character);
                selected = 0;
            }
            _ => {}
        }
    }
}

fn pop_last_grapheme(text: &mut String) {
    if let Some((start, _)) = text.grapheme_indices(true).last() {
        text.truncate(start);
    }
}

fn erase_menu(stdout: &mut io::Stdout, rendered_rows: usize) -> io::Result<()> {
    if rendered_rows > 0 {
        execute!(
            stdout,
            MoveToColumn(0),
            MoveUp(rendered_rows.min(u16::MAX as usize) as u16),
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown)
        )?;
    }
    Ok(())
}

fn rank_matches<'a>(query: &str, options: &'a [SelectOption]) -> Vec<&'a SelectOption> {
    let mut matches = options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            option_score(query, option).map(|score| (score, index, option))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(score, index, _)| (*score, *index));
    matches.into_iter().map(|(_, _, option)| option).collect()
}

fn option_score(query: &str, option: &SelectOption) -> Option<usize> {
    let fields = [
        (option.label.as_str(), 0usize),
        (option.value.as_str(), 3usize),
        (option.description.as_str(), 12usize),
    ];
    let mut total = 0usize;
    for token in query.split_whitespace() {
        let needle = token.to_lowercase().chars().collect::<Vec<_>>();
        let best = fields
            .iter()
            .filter_map(|(text, weight)| {
                subsequence_score(&needle, text).map(|score| score.saturating_add(*weight))
            })
            .min()?;
        total = total.saturating_add(best);
    }
    Some(total)
}

fn subsequence_score(needle: &[char], text: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let haystack = text.to_lowercase().chars().collect::<Vec<_>>();
    let mut positions = Vec::with_capacity(needle.len());
    let mut needle_index = 0usize;
    for (index, character) in haystack.iter().enumerate() {
        if *character == needle[needle_index] {
            positions.push(index);
            needle_index += 1;
            if needle_index == needle.len() {
                break;
            }
        }
    }
    if needle_index != needle.len() {
        return None;
    }

    let first = positions[0];
    let last = *positions.last()?;
    let gaps = positions
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]).saturating_sub(1))
        .sum::<usize>();
    let starts_inside_word = first > 0 && haystack[first - 1].is_alphanumeric();
    let ends_inside_word = last + 1 < haystack.len() && haystack[last + 1].is_alphanumeric();

    Some(
        first
            .saturating_mul(2)
            .saturating_add(gaps.saturating_mul(3))
            .saturating_add(usize::from(starts_inside_word) * 2)
            .saturating_add(usize::from(ends_inside_word) * 2),
    )
}

struct SelectorRawMode;

impl SelectorRawMode {
    fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for SelectorRawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_search_matches_command_ids_and_descriptions() {
        assert!(subsequence_score(&['m', 'd', 'l'], "/model").is_some());
        assert!(subsequence_score(
            &"provider".chars().collect::<Vec<_>>(),
            "connect or replace a provider key"
        )
        .is_some());
        assert!(option_score(
            "model open",
            &SelectOption::new("/model", "OpenRouter open model", "/model")
        )
        .is_some());
        assert!(option_score(
            "workflow",
            &SelectOption::new("/model", "select a model", "/model")
        )
        .is_none());
    }

    #[test]
    fn exact_label_matches_rank_ahead_of_loose_description_matches() {
        let options = vec![
            SelectOption::new("/help", "Get help with the model settings", "/help"),
            SelectOption::new("/models", "List available models", "/models"),
            SelectOption::new("/model", "Switch the active model", "/model"),
        ];
        let ranked = rank_matches("model", &options);
        assert_eq!(ranked[0].value, "/model");
        assert_eq!(ranked[1].value, "/models");
        assert_eq!(ranked[2].value, "/help");
    }

    #[test]
    fn search_keeps_source_order_for_equal_scores_and_supports_multiple_fields() {
        let options = vec![
            SelectOption::new("/model", "Switch current provider model", "/model"),
            SelectOption::new("/model", "Switch current provider model", "/model"),
        ];
        let ranked = rank_matches("model switch", &options);
        assert_eq!(ranked.len(), 2);
        assert!(std::ptr::eq(ranked[0], &options[0]));
        assert!(std::ptr::eq(ranked[1], &options[1]));
    }

    #[test]
    fn picker_backspace_removes_one_grapheme_at_a_time() {
        let mut query = "猫👩🏽‍💻e\u{301}".to_string();
        pop_last_grapheme(&mut query);
        assert_eq!(query, "猫👩🏽‍💻");
        pop_last_grapheme(&mut query);
        assert_eq!(query, "猫");
    }
}

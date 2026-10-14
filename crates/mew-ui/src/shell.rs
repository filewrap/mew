use crate::layout::{fit, fit_middle, line, visible_width, wrap_lines};
use crate::prompt::InputBuffer;
use crate::render::render_markdown_light;
use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptEntry {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatShellFrame {
    pub agent_name: String,
    pub provider: String,
    pub model: String,
    pub directory: String,
    pub mode: String,
    pub tools: String,
    pub session: String,
    pub status: String,
    pub animations: bool,
    pub transcript: Vec<TranscriptEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellRender {
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub max_scroll: usize,
    pub scroll_offset: usize,
}

/// Render a full-screen chat view into fixed-width rows. The caller owns terminal
/// mode and redraw behavior, keeping this layout deterministic and testable.
pub fn render_chat_shell(
    theme: &Theme,
    frame: &ChatShellFrame,
    input: &InputBuffer,
    animation_frame: usize,
    width: usize,
    height: usize,
    requested_scroll: usize,
) -> ShellRender {
    let width = width.max(1);
    let height = height.max(1);
    let mut rows = vec![" ".repeat(width); height];
    let animation_frame = if frame.animations { animation_frame } else { 0 };

    if height < 8 {
        let title = format!("{} {}", theme.primary("mew"), theme.dim("· chat"));
        if height > 1 {
            put_row(&mut rows, 0, &title, width);
        }
        let processing_status = is_processing_status(&frame.status) && height >= 3;
        let composer_row = height.saturating_sub(1 + usize::from(processing_status));
        let prefix = theme.accent("› ");
        let available = width.saturating_sub(visible_width(&prefix)).max(1);
        let (visible_lines, (cursor_column, _)) = input.visible_lines(available, 1);
        let visible = visible_lines.first().map(String::as_str).unwrap_or("");
        put_row(
            &mut rows,
            composer_row,
            &format!("{prefix}{visible}"),
            width,
        );
        if processing_status {
            let blocks = processing_blocks(animation_frame, frame.animations);
            put_row(
                &mut rows,
                height - 1,
                &format!(
                    "{} {}",
                    theme.primary(processing_label(&frame.status)),
                    theme.accent(blocks)
                ),
                width,
            );
        }
        return ShellRender {
            lines: rows,
            cursor: (
                visible_width(&prefix)
                    .saturating_add(cursor_column)
                    .min(width.saturating_sub(1)) as u16,
                composer_row.min(u16::MAX as usize) as u16,
            ),
            max_scroll: 0,
            scroll_offset: 0,
        };
    }

    let expression_frame = animation_frame / 5;
    let expression =
        crate::miku::miku_expression(shell_miku_motion(&frame.status), expression_frame);
    put_row(
        &mut rows,
        0,
        &format!(
            "{} {} {}",
            theme.accent("▌"),
            theme.primary(format!("mew  {}", frame.agent_name)),
            theme.dim(format!("· {expression}")),
        ),
        width,
    );

    let tool_label = if frame.tools == "unavailable" {
        theme.dim("tools unavailable")
    } else {
        theme.ok("tools")
    };
    let mode_label = match frame.mode.as_str() {
        "ask" => theme.accent("ask"),
        "edit" => theme.warn("edit"),
        "auto" => theme.err("auto"),
        _ => theme.accent(&frame.mode),
    };
    let controls = format!(
        "  {}  {}  {}",
        mode_label,
        tool_label,
        theme.dim(format!("session {}", frame.session)),
    );
    let model_width = width.saturating_sub(visible_width(&controls));
    let model_label = fit_middle(&format!("{}/{}", frame.provider, frame.model), model_width);
    let details = format!("{}{}", theme.primary(model_label), controls);
    put_row(&mut rows, 1, &details, width);

    let state = status_label(theme, &frame.status);
    let location_width = width.saturating_sub(visible_width(&state) + 2);
    let location = theme.dim(fit_middle(
        &format!("⌂ {}", frame.directory),
        location_width,
    ));
    put_row(&mut rows, 2, &format!("{}  {}", state, location), width);
    put_row(&mut rows, 3, &theme.border(line('─', width)), width);

    let max_composer_lines = if height >= 16 {
        3
    } else if height >= 11 {
        2
    } else {
        1
    };
    let composer_lines = input
        .value()
        .split('\n')
        .count()
        .clamp(1, max_composer_lines);
    let processing_status = is_processing_status(&frame.status);
    let reserved_status_rows = usize::from(processing_status);
    let composer_top = height
        .saturating_sub(composer_lines + 2 + reserved_status_rows)
        .max(4);
    let body_start = 4.min(height);
    let body_rows = composer_top.saturating_sub(body_start);
    let content = shell_content_rows(theme, frame, width, body_rows, animation_frame);
    let max_scroll = content.len().saturating_sub(body_rows);
    let scroll_offset = requested_scroll.min(max_scroll);
    let end = content.len().saturating_sub(scroll_offset);
    let start = end.saturating_sub(body_rows);
    for (offset, line_text) in content[start..end].iter().enumerate() {
        put_row(&mut rows, body_start + offset, line_text, width);
    }

    let draft_status = if input.has_newline() {
        theme.warn("multiline draft")
    } else {
        theme.ok("ready to send")
    };
    let prompt_label = format!(
        "{} {} {} {} {}",
        theme.primary("message"),
        theme.accent("·"),
        theme.accent(format!("{} mode", frame.mode)),
        theme.accent("·"),
        draft_status,
    );
    put_row(
        &mut rows,
        composer_top,
        &format!("{}{}", theme.accent("╭─ "), prompt_label),
        width,
    );

    let input_top = composer_top + 1;
    let prefix = theme.accent("│ › ");
    let continuation_prefix = theme.accent("│   ");
    let input_width = width.saturating_sub(visible_width(&prefix)).max(1);
    let (visible_lines, (cursor_column, cursor_line)) =
        input.visible_lines(input_width, composer_lines);
    for line_index in 0..composer_lines {
        let value = visible_lines
            .get(line_index)
            .map(String::as_str)
            .unwrap_or("");
        let line_prefix = if line_index == 0 {
            &prefix
        } else {
            &continuation_prefix
        };
        put_row(
            &mut rows,
            input_top + line_index,
            &format!("{line_prefix}{value}"),
            width,
        );
    }

    if processing_status {
        let status_row = input_top + composer_lines;
        let status_text = processing_label(&frame.status);
        let filled = processing_blocks(animation_frame, frame.animations);
        let blocks = format!(
            "{}{}",
            theme.accent(
                filled
                    .chars()
                    .filter(|cell| *cell == '▰')
                    .collect::<String>()
            ),
            theme.dim(
                filled
                    .chars()
                    .filter(|cell| *cell == '▱')
                    .collect::<String>()
            )
        );
        put_row(
            &mut rows,
            status_row,
            &format!(
                "{}  {}  {}",
                theme.border("│"),
                theme.primary(status_text),
                blocks
            ),
            width,
        );
    }

    let bottom_hint = if width >= 92 {
        "Enter send · Shift+Enter newline · Ctrl+P/F1 actions · / commands · PgUp/PgDn scroll"
    } else if width >= 56 {
        "Enter send · Shift+Enter newline · Ctrl+P/F1 menu"
    } else if width >= 36 {
        "Enter send · Shift+Enter newline"
    } else {
        "Enter sends"
    };
    put_row(
        &mut rows,
        height - 1,
        &format!("{}{}", theme.accent("╰─ "), theme.primary(bottom_hint)),
        width,
    );

    let cursor_x = visible_width(&prefix)
        .saturating_add(cursor_column)
        .min(width.saturating_sub(1));
    let cursor_y = input_top
        .saturating_add(cursor_line)
        .min(height.saturating_sub(1));
    ShellRender {
        lines: rows,
        cursor: (
            cursor_x.min(u16::MAX as usize) as u16,
            cursor_y.min(u16::MAX as usize) as u16,
        ),
        max_scroll,
        scroll_offset,
    }
}

fn shell_content_rows(
    theme: &Theme,
    frame: &ChatShellFrame,
    width: usize,
    available_rows: usize,
    animation_frame: usize,
) -> Vec<String> {
    let mut rows = if frame.transcript.is_empty() {
        welcome_rows(
            theme,
            width,
            available_rows,
            animation_frame,
            shell_miku_motion(&frame.status),
        )
    } else {
        transcript_rows(theme, frame, width.saturating_sub(2).max(1))
    };

    if frame.status.starts_with("error") {
        rows.extend(error_card_rows(theme, &frame.status, width));
    }
    rows
}

fn welcome_rows(
    theme: &Theme,
    width: usize,
    available_rows: usize,
    animation_frame: usize,
    motion: crate::miku::MikuMotion,
) -> Vec<String> {
    if available_rows >= 16 && width >= 64 {
        let sprite = crate::miku::miku_pixel_cat(theme, motion, animation_frame);
        let copy = [
            theme.primary("Welcome to Mew."),
            theme.dim("Your curious coding companion"),
            theme.dim("Ask, explore, and build together."),
            String::new(),
            theme.dim("Workspace tools need your trust first."),
            String::new(),
            theme.accent("/model · /sessions · /skills"),
            theme.dim("/help · Ctrl+P/F1 opens actions"),
            theme.dim("Shift+Enter adds a line"),
            theme.dim("Enter sends your message"),
            String::new(),
            theme.dim("small steps, careful edits"),
            theme.dim("soft paws, sharp claws"),
            String::new(),
            String::new(),
        ];
        let copy_width = copy.iter().map(|row| visible_width(row)).max().unwrap_or(0);
        let cat_width = sprite
            .iter()
            .map(|row| visible_width(row))
            .max()
            .unwrap_or(0);
        let total_width = cat_width + 4 + copy_width;
        let left = width.saturating_sub(total_width) / 2;
        return sprite
            .iter()
            .zip(copy.iter())
            .map(|(cat, text)| fit(&format!("{}{}    {}", " ".repeat(left), cat, text), width))
            .collect();
    }

    let short = [
        theme.primary("Welcome to Mew."),
        theme.dim("Type a question or start with /"),
        theme.dim("Ctrl+P/F1 opens commands · Shift+Enter adds a line"),
    ];
    short
        .iter()
        .map(|text| {
            let padding = width.saturating_sub(visible_width(text)) / 2;
            fit(&format!("{}{}", " ".repeat(padding), text), width)
        })
        .collect()
}

fn transcript_rows(theme: &Theme, frame: &ChatShellFrame, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for entry in &frame.transcript {
        match entry.role.as_str() {
            "user" => {
                rows.push(fit(
                    &format!(
                        "{}{}{}",
                        theme.accent("╭─ "),
                        theme.primary("you"),
                        theme.accent(" ─")
                    ),
                    width,
                ));
                let text = if entry.content.trim().is_empty() {
                    "(empty message)"
                } else {
                    entry.content.as_str()
                };
                for wrapped in wrap_lines(text, width.saturating_sub(4).max(1)) {
                    rows.push(fit(&format!("{}  {wrapped}", theme.accent("│")), width));
                }
                rows.push(fit(&theme.accent("╰────────────────"), width));
            }
            "assistant" => {
                rows.push(fit(
                    &format!(
                        "{}{}{}",
                        theme.border("╭─ "),
                        theme.primary(&frame.agent_name),
                        theme.border(" ─")
                    ),
                    width,
                ));
                let rendered = render_markdown_light(theme, &entry.content);
                let plain = String::from_utf8(strip_ansi_escapes::strip(&rendered))
                    .unwrap_or_else(|_| rendered.clone());
                if plain.trim().is_empty() {
                    rows.push(fit(&format!("{}  (no text)", theme.border("│")), width));
                } else {
                    for raw in plain.lines() {
                        if raw.trim().is_empty() {
                            rows.push(String::new());
                            continue;
                        }
                        for wrapped in wrap_lines(raw, width.saturating_sub(4).max(1)) {
                            rows.push(fit(&format!("{}  {wrapped}", theme.border("│")), width));
                        }
                    }
                }
                rows.push(fit(&theme.border("╰────────────────"), width));
            }
            "tool" => {
                rows.push(fit(
                    &format!(
                        "{}{}{}",
                        theme.warn("╭─ "),
                        theme.warn("tool output"),
                        theme.warn(" ─")
                    ),
                    width,
                ));
                for wrapped in wrap_lines(&entry.content, width.saturating_sub(4).max(1)) {
                    rows.push(fit(&format!("{}  {wrapped}", theme.warn("│")), width));
                }
                rows.push(fit(&theme.warn("╰────────────────"), width));
            }
            _ => {}
        }
        rows.push(String::new());
    }
    rows
}

fn error_card_rows(theme: &Theme, status: &str, width: usize) -> Vec<String> {
    let inner = width.saturating_sub(4).max(1);
    let detail = status
        .strip_prefix("error ·")
        .or_else(|| status.strip_prefix("error"))
        .unwrap_or(status)
        .trim();
    let mut rows = vec![fit(
        &format!(
            "{}{}{}",
            theme.err("╭─ "),
            theme.err("request needs attention"),
            theme.err(" ─")
        ),
        width,
    )];
    for wrapped in wrap_lines(detail, inner) {
        rows.push(fit(
            &format!("{}  {}", theme.err("│"), theme.err(wrapped)),
            width,
        ));
    }
    rows.push(fit(
        &theme.err("╰────────────────────────────────────────"),
        width,
    ));
    rows
}

fn processing_blocks(frame: usize, animations: bool) -> String {
    const WIDTH: usize = 6;
    let filled = if animations {
        match (frame / 3) % 10 {
            phase @ 0..=5 => phase + 1,
            phase => 11 - phase,
        }
    } else {
        WIDTH / 2
    };
    format!("{}{}", "▰".repeat(filled), "▱".repeat(WIDTH - filled))
}

fn processing_label(status: &str) -> String {
    if status.contains("approval") {
        return "waiting for approval · review the requested action".into();
    }
    if status.starts_with("tool") {
        return format!("running · {}", status.trim());
    }
    if status.starts_with("workflow") || status.starts_with("pipeline") {
        return format!("running · {}", status.trim());
    }
    if status.starts_with("running") {
        return status.trim().to_string();
    }
    if status.starts_with("planning") || status.starts_with("step ") {
        return format!("working · {}", status.trim());
    }
    if status.contains("model request") {
        let detail = status
            .split_once("model request")
            .map(|(_, detail)| detail.trim())
            .filter(|detail| !detail.is_empty())
            .map(|detail| format!(" {detail}"))
            .unwrap_or_default();
        return format!("processing · model request{detail}");
    }
    if status.starts_with("preparing response") {
        return "processing · preparing response".into();
    }
    "processing · thinking".into()
}

fn status_label(theme: &Theme, status: &str) -> String {
    if status.starts_with("error") {
        return theme.err("✕ error");
    }
    if status.contains("approval") {
        return theme.warn("◆ approval");
    }
    if status.starts_with("complete") {
        return theme.ok("✓ complete");
    }
    if status.starts_with("cancelled") {
        return theme.warn("■ cancelled");
    }
    if status == "ready" {
        return theme.ok("● ready");
    }
    if status.starts_with("tool") {
        return theme.accent("◈ tool");
    }
    if status.starts_with("workflow")
        || status.starts_with("pipeline")
        || status.starts_with("step ")
    {
        return theme.accent("◈ workflow");
    }
    if status.starts_with("thinking")
        || status.starts_with("model")
        || status.starts_with("running")
        || status.contains("model request")
        || status.starts_with("preparing response")
    {
        return theme.primary("◌ working");
    }
    theme.dim(status.split('·').next().unwrap_or("ready").trim())
}

fn is_processing_status(status: &str) -> bool {
    is_active_status(status) && !status.starts_with("complete")
}

fn is_active_status(status: &str) -> bool {
    status.starts_with("thinking")
        || status.starts_with("model")
        || status.contains("model request")
        || status.starts_with("preparing response")
        || status.starts_with("running")
        || status.starts_with("tool")
        || status.starts_with("planning")
        || status.starts_with("workflow")
        || status.starts_with("pipeline")
        || status.starts_with("step ")
        || status.contains("approval")
        || status.starts_with("complete")
}

fn shell_miku_motion(status: &str) -> crate::miku::MikuMotion {
    use crate::miku::MikuMotion;
    if status.starts_with("error") {
        MikuMotion::Error
    } else if status.starts_with("complete") {
        MikuMotion::Celebrate
    } else if status.contains("approval")
        || status.starts_with("thinking")
        || status.starts_with("model")
        || status.contains("model request")
        || status.starts_with("preparing response")
        || status.starts_with("planning")
    {
        MikuMotion::Thinking
    } else if status.starts_with("tool")
        || status.starts_with("workflow")
        || status.starts_with("pipeline")
        || status.starts_with("running")
        || status.ends_with("starting")
    {
        MikuMotion::Running
    } else {
        MikuMotion::Idle
    }
}

fn put_row(rows: &mut [String], index: usize, text: &str, width: usize) {
    if let Some(row) = rows.get_mut(index) {
        *row = fit(text, width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::theme_by_name;

    fn sample_frame() -> ChatShellFrame {
        ChatShellFrame {
            agent_name: "mew".into(),
            provider: "openrouter".into(),
            model: "free-model".into(),
            directory: "/workspace/mew".into(),
            mode: "ask".into(),
            tools: "tools".into(),
            session: "01234567".into(),
            status: "ready".into(),
            animations: true,
            transcript: vec![
                TranscriptEntry {
                    role: "user".into(),
                    content: "hello from the cat".into(),
                },
                TranscriptEntry {
                    role: "assistant".into(),
                    content: "A concise reply.".into(),
                },
            ],
        }
    }

    #[test]
    fn shell_has_fixed_size_rows_and_keeps_multiline_cursor_inside_the_composer() {
        let theme = theme_by_name("claude-minimal");
        let mut input = InputBuffer::default();
        input.set_value("draft 猫\nsecond line".into());
        let view = render_chat_shell(&theme, &sample_frame(), &input, 0, 72, 18, 0);
        assert_eq!(view.lines.len(), 18);
        assert!(view.lines.iter().all(|row| visible_width(row) == 72));
        assert!(usize::from(view.cursor.0) < 72);
        assert!(usize::from(view.cursor.1) >= 14);
        assert!(usize::from(view.cursor.1) <= 16);
    }

    #[test]
    fn compact_shell_keeps_processing_status_below_the_prompt() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.status = "running · agent".into();
        let mut input = InputBuffer::default();
        input.set_value("hello".into());

        let view = render_chat_shell(&theme, &frame, &input, 0, 36, 7, 0);

        assert!(view.lines[5].contains("› hello"));
        assert!(view.lines[6].contains("running · agent"));
        assert!(view.lines.iter().all(|row| visible_width(row) == 36));
    }

    #[test]
    fn multiline_composer_uses_one_prompt_marker_and_aligns_continuation_lines() {
        let theme = theme_by_name("no-color");
        let mut input = InputBuffer::default();
        input.set_value("first line\nsecond line\nthird line".into());
        let view = render_chat_shell(&theme, &sample_frame(), &input, 0, 72, 20, 0);
        let composer_top = 20 - (3 + 2);
        assert!(view.lines[composer_top + 1].contains("│ › first line"));
        assert!(view.lines[composer_top + 2].contains("│   second line"));
        assert!(view.lines[composer_top + 3].contains("│   third line"));
        assert_eq!(
            view.lines[composer_top + 1..composer_top + 4]
                .iter()
                .map(|row| row.matches('›').count())
                .sum::<usize>(),
            1
        );
    }

    #[test]
    fn slash_draft_with_blank_continuations_still_has_only_one_prompt_marker() {
        let theme = theme_by_name("no-color");
        let mut input = InputBuffer::default();
        input.set_value("/se\n\n".into());
        let view = render_chat_shell(&theme, &sample_frame(), &input, 0, 72, 20, 0);
        let composer_top = 20 - (3 + 2);
        let composer_input = &view.lines[composer_top + 1..composer_top + 4];

        assert!(composer_input[0].contains("│ › /se"));
        assert!(composer_input[1].contains("│   "));
        assert!(composer_input[2].contains("│   "));
        assert!(composer_input
            .iter()
            .all(|row| row.matches('│').count() == 1));
        assert_eq!(
            composer_input
                .iter()
                .map(|row| row.matches('›').count())
                .sum::<usize>(),
            1
        );
    }

    #[test]
    fn idle_composer_is_compact_and_narrow_footer_hints_are_not_clipped() {
        let theme = theme_by_name("no-color");
        let view = render_chat_shell(
            &theme,
            &sample_frame(),
            &InputBuffer::default(),
            0,
            48,
            20,
            0,
        );
        let composer_top = 20 - (1 + 2);
        assert!(view.lines[composer_top].contains("message"));
        assert!(view.lines[composer_top + 1].contains("│ › "));
        assert_eq!(view.lines[composer_top + 1].matches('│').count(), 1);
        assert!(view.lines[19].contains("Enter send · Shift+Enter newline"));
        assert!(view.lines[19].contains("╰─"));
        assert!(view.lines.iter().all(|row| visible_width(row) == 48));
    }

    #[test]
    fn footer_hint_breakpoints_keep_the_full_hint_visible() {
        let theme = theme_by_name("no-color");
        let frame = sample_frame();
        for (width, expected) in [
            (36, "Shift+Enter newline"),
            (91, "Ctrl+P/F1 menu"),
            (92, "PgUp/PgDn scroll"),
        ] {
            let view = render_chat_shell(&theme, &frame, &InputBuffer::default(), 0, width, 20, 0);
            assert!(view.lines[19].contains(expected), "width {width}");
            assert_eq!(visible_width(&view.lines[19]), width);
        }
    }

    #[test]
    fn shell_viewport_scroll_is_bounded_and_resize_aware() {
        let theme = theme_by_name("vector-mocha");
        let mut frame = sample_frame();
        frame
            .transcript
            .extend((0..30).map(|index| TranscriptEntry {
                role: "user".into(),
                content: format!("line {index} with a longer message to wrap"),
            }));
        let input = InputBuffer::default();
        let normal = render_chat_shell(&theme, &frame, &input, 1, 80, 18, usize::MAX);
        assert!(normal.max_scroll > 0);
        assert_eq!(normal.scroll_offset, normal.max_scroll);
        assert_eq!(normal.lines.len(), 18);
        let tiny = render_chat_shell(&theme, &frame, &input, 0, 28, 8, 0);
        assert_eq!(tiny.lines.len(), 8);
        assert!(tiny.lines.iter().all(|row| visible_width(row) == 28));
    }

    #[test]
    fn errors_are_rendered_once_in_a_dedicated_card_without_repeating_in_the_header() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.status = "error · provider request failed (HTTP 401): expired API key".into();
        let view = render_chat_shell(&theme, &frame, &InputBuffer::default(), 0, 72, 18, 0);
        let text = view.lines.join("\n");
        assert!(view.lines[2].contains("error"));
        assert_eq!(text.matches("HTTP 401").count(), 1);
        assert!(text.contains("expired API key"));
        assert!(view.lines.iter().all(|row| visible_width(row) == 72));
    }

    #[test]
    fn processing_status_stays_below_multiline_input_with_a_slow_fixed_width_meter() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.status = "thinking · tools enabled".into();
        let mut input = InputBuffer::default();
        input.set_value("first line\nsecond line\nthird line".into());
        let first = render_chat_shell(&theme, &frame, &input, 1, 72, 20, 0);
        let same_tick = render_chat_shell(&theme, &frame, &input, 2, 72, 20, 0);
        let later = render_chat_shell(&theme, &frame, &input, 4, 72, 20, 0);
        assert_eq!(
            first.lines, same_tick.lines,
            "redraws inside 540 ms do not pulse"
        );
        assert_ne!(first.lines[18], later.lines[18]);
        assert_eq!(
            first.lines[18].chars().filter(|cell| *cell == '▰').count(),
            1
        );
        assert_eq!(
            later.lines[18].chars().filter(|cell| *cell == '▰').count(),
            2
        );
        assert_eq!(
            first.lines[18].find('▰'),
            later.lines[18].find('▰'),
            "the status label and meter anchor never move"
        );
        assert!(first.lines[15].contains("│ › first line"));
        assert!(first.lines[16].contains("│   second line"));
        assert!(first.lines[17].contains("│   third line"));
        assert!(first.lines[18].contains("processing · thinking"));
        assert!(first.lines[18].contains("▰▱▱▱▱▱"));
        assert!(first.lines[19].contains("╰─"));
        assert!(first.lines.iter().all(|row| visible_width(row) == 72));

        frame.animations = false;
        let reduced_motion_first = render_chat_shell(&theme, &frame, &input, 1, 72, 20, 0);
        let reduced_motion_later = render_chat_shell(&theme, &frame, &input, 24, 72, 20, 0);
        assert_eq!(reduced_motion_first.lines, reduced_motion_later.lines);
    }

    #[test]
    fn runtime_model_request_status_appears_below_the_composer() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.status = "main · model request 1".into();
        assert!(is_active_status(&frame.status));
        assert!(status_label(&theme, &frame.status).contains("working"));
        assert_eq!(
            shell_miku_motion(&frame.status),
            crate::miku::MikuMotion::Thinking
        );
        let view = render_chat_shell(&theme, &frame, &InputBuffer::default(), 2, 80, 20, 0);
        assert!(view.lines[18].contains("processing · model request 1"));
    }

    #[test]
    fn empty_home_has_a_large_detailed_cat_and_command_shortcuts() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.transcript.clear();
        let view = render_chat_shell(&theme, &frame, &InputBuffer::default(), 2, 80, 24, 0);
        let text = view.lines.join("\n");
        assert!(text.contains("Your curious coding companion"));
        assert!(text.contains("soft paws, sharp claws"));
        assert!(text.contains("Workspace tools need your trust first."));
        assert!(text.contains("●"));
        assert!(text.contains("▚"));
        assert!(text.contains("/model"));
        assert!(text.contains("Ctrl+P"));
    }

    #[test]
    fn empty_home_cat_advances_only_after_slow_wall_clock_frames() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.transcript.clear();
        let first = render_chat_shell(&theme, &frame, &InputBuffer::default(), 2, 80, 24, 0);
        let before_pose = render_chat_shell(&theme, &frame, &InputBuffer::default(), 3, 80, 24, 0);
        let next_pose = render_chat_shell(&theme, &frame, &InputBuffer::default(), 6, 80, 24, 0);

        assert_eq!(first.lines, before_pose.lines);
        assert_ne!(first.lines, next_pose.lines);
        assert!(next_pose.lines.join("\n").contains("Welcome to Mew."));

        frame.status = "thinking · model request".into();
        let processing_cat =
            render_chat_shell(&theme, &frame, &InputBuffer::default(), 6, 80, 24, 0);
        assert!(processing_cat
            .lines
            .join("\n")
            .contains("Your curious coding companion"));
        assert!(processing_cat.lines[22].contains("processing · model request"));
    }

    #[test]
    fn composer_discloses_multiline_and_instant_command_interactions() {
        let theme = theme_by_name("no-color");
        let view = render_chat_shell(
            &theme,
            &sample_frame(),
            &InputBuffer::default(),
            0,
            100,
            20,
            0,
        );
        let text = view.lines.join("\n");
        assert!(text.contains("Shift+Enter newline"));
        assert!(text.contains("Ctrl+P/F1 actions"));
        assert!(text.contains("PgUp/PgDn scroll"));
    }

    #[test]
    fn shell_uses_lowercase_labels_and_does_not_present_text_only_as_a_mode() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.tools = "unavailable".into();
        let view = render_chat_shell(&theme, &frame, &InputBuffer::default(), 0, 100, 20, 0);
        let text = view.lines.join("\n");

        assert!(view.lines[0].contains("mew"));
        assert!(view.lines[1].contains("ask"));
        assert!(view.lines[1].contains("tools unavailable"));
        assert!(view.lines[2].contains("ready"));
        assert!(text.contains("message · ask mode · ready to send"));
        assert!(!text.contains("text-only"));
        assert!(!text.contains("ASK"));
        assert!(!text.contains("TOOLS"));
        assert!(!text.contains("MESSAGE"));
    }

    #[test]
    fn long_model_identifiers_do_not_hide_mode_tools_or_session_controls() {
        let theme = theme_by_name("no-color");
        let mut frame = sample_frame();
        frame.model = "a-very-long-provider-model-name-that-would-overflow-the-header".into();
        let view = render_chat_shell(&theme, &frame, &InputBuffer::default(), 0, 72, 18, 0);
        assert!(view.lines[1].contains("ask"));
        assert!(view.lines[1].contains("tools"));
        assert!(view.lines[1].contains("session 01234567"));
        assert_eq!(visible_width(&view.lines[1]), 72);
    }
}

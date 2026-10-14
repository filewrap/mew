//! Mew's silent rich-response layer over ordinary Markdown.
//!
//! Markdown remains the canonical body. A small, HTML-comment-based extension
//! carries optional UI suggestions without exposing protocol syntax in ordinary
//! Markdown renderers. Suggestions are data only; hosts decide how to present
//! them and must not treat them as tool calls or permissions.

use serde::{Deserialize, Serialize};

/// Instructions appended to Mew agent/chat system prompts. Keep this guidance
/// optional and additive: normal Markdown is always a valid response.
pub const MODEL_INSTRUCTIONS: &str = r#"## Mew response format (Markdown v4 layer)
Write the response body in ordinary Markdown. This is the base format and must
remain useful if a client ignores Mew extensions. Use Markdown blockquotes for
quoted material; Mew-capable clients may render those as distinct quote blocks.

Optionally, add up to eight follow-up suggestion buttons at the end of a reply.
Each button is exactly one standalone HTML comment line in this form:
<!-- mew:v4:button {"label":"Show an example","prompt":"Give a concrete example of the previous explanation."} -->
The JSON object must contain only a short `label` (1–80 UTF-8 bytes) and a
single-line `prompt` (1–1000 UTF-8 bytes). These are user-visible follow-up
suggestions only, never tool calls, URLs, executable actions, or grants of
permission. Omit buttons when they do not add value. Never emit any other Mew
protocol syntax."#;

const BUTTON_PREFIX: &str = "<!-- mew:v4:button ";
const BUTTON_SUFFIX: &str = " -->";
pub const MAX_BUTTONS: usize = 8;
pub const MAX_BUTTON_LABEL_BYTES: usize = 80;
pub const MAX_BUTTON_PROMPT_BYTES: usize = 1_000;

/// Parsed response: `markdown` is the safe baseline body; `blocks` preserves
/// richer presentation semantics for clients that understand Mew v4.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponseDocument {
    pub markdown: String,
    #[serde(default)]
    pub blocks: Vec<ResponseBlock>,
}

impl ResponseDocument {
    /// Return ordinary Markdown with valid v4 button annotations removed.
    pub fn as_markdown(&self) -> &str {
        &self.markdown
    }

    /// Follow-up suggestions are inert data; this helper exposes them without
    /// assigning execution semantics.
    pub fn buttons(&self) -> impl Iterator<Item = &SuggestedButton> {
        self.blocks.iter().filter_map(|block| match block {
            ResponseBlock::Button { button } => Some(button),
            _ => None,
        })
    }
}

/// A block either retains ordinary Markdown or has a Mew-specific presentation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseBlock {
    Markdown { text: String },
    Quote { text: String },
    Button { button: SuggestedButton },
}

/// A suggested user follow-up. This value is not an executable action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SuggestedButton {
    pub label: String,
    pub prompt: String,
}

/// Parse ordinary Markdown plus Mew's optional comment-encoded buttons.
///
/// Malformed or over-limit annotations are kept as ordinary Markdown instead
/// of being silently discarded. Fenced code is never interpreted as protocol.
pub fn parse_response(source: &str) -> ResponseDocument {
    let mut document = ResponseDocument::default();
    let mut markdown_block = String::new();
    let mut in_fence = false;
    let mut fence_marker: Option<&str> = None;
    let mut button_count = 0usize;
    let mut index = 0usize;
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();

    while index < lines.len() {
        let raw = lines[index];
        let has_newline = raw.ends_with('\n');
        let without_newline = raw.strip_suffix('\n').unwrap_or(raw);
        let line = without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline);
        let trimmed = line.trim_start();

        if let Some(marker) = fence_marker {
            document.markdown.push_str(raw);
            markdown_block.push_str(raw);
            if trimmed.starts_with(marker) {
                fence_marker = None;
                in_fence = false;
            }
            index += 1;
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = if trimmed.starts_with("```") {
                "```"
            } else {
                "~~~"
            };
            in_fence = true;
            fence_marker = Some(marker);
            document.markdown.push_str(raw);
            markdown_block.push_str(raw);
            index += 1;
            continue;
        }
        debug_assert!(!in_fence);

        if button_count < MAX_BUTTONS {
            if let Some(button) = parse_button(line) {
                flush_markdown(&mut document.blocks, &mut markdown_block);
                document.blocks.push(ResponseBlock::Button { button });
                button_count += 1;
                if has_newline {
                    document.markdown.push('\n');
                }
                index += 1;
                continue;
            }
        }

        if is_quote_line(line) {
            flush_markdown(&mut document.blocks, &mut markdown_block);
            let mut quote = String::new();
            while index < lines.len() {
                let raw_quote = lines[index];
                let content = raw_quote.strip_suffix('\n').unwrap_or(raw_quote);
                let content = content.strip_suffix('\r').unwrap_or(content);
                if !is_quote_line(content) {
                    break;
                }
                document.markdown.push_str(raw_quote);
                quote.push_str(quote_content(content));
                if raw_quote.ends_with('\n') {
                    quote.push('\n');
                }
                index += 1;
            }
            document.blocks.push(ResponseBlock::Quote {
                text: quote.trim_end_matches('\n').to_string(),
            });
            continue;
        }

        document.markdown.push_str(raw);
        markdown_block.push_str(raw);
        index += 1;
    }
    flush_markdown(&mut document.blocks, &mut markdown_block);
    document
}

fn parse_button(line: &str) -> Option<SuggestedButton> {
    let annotation = line.trim();
    let payload = annotation
        .strip_prefix(BUTTON_PREFIX)?
        .strip_suffix(BUTTON_SUFFIX)?;
    let button = serde_json::from_str::<SuggestedButton>(payload).ok()?;
    let label = validate_single_line(button.label, MAX_BUTTON_LABEL_BYTES)?;
    let prompt = validate_single_line(button.prompt, MAX_BUTTON_PROMPT_BYTES)?;
    Some(SuggestedButton { label, prompt })
}

fn validate_single_line(value: String, max_bytes: usize) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_owned())
}

fn is_quote_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed == ">" || trimmed.starts_with("> ")
}

fn quote_content(line: &str) -> &str {
    let trimmed = line.trim_start();
    if trimmed == ">" {
        ""
    } else {
        trimmed.strip_prefix("> ").unwrap_or(trimmed)
    }
}

fn flush_markdown(blocks: &mut Vec<ResponseBlock>, pending: &mut String) {
    if !pending.trim().is_empty() {
        blocks.push(ResponseBlock::Markdown {
            text: std::mem::take(pending),
        });
    } else {
        pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quote_and_button_but_keeps_markdown_baseline() {
        let source = concat!(
            "Answer **bold**.\n\n",
            "> quoted line\n",
            "> second line\n\n",
            "<!-- mew:v4:button {\"label\":\"Example\",\"prompt\":\"Show an example.\"} -->\n"
        );
        let parsed = parse_response(source);
        assert!(parsed.markdown.contains("Answer **bold**."));
        assert!(parsed.markdown.contains("> quoted line\n> second line"));
        assert!(!parsed.markdown.contains("mew:v4:button"));
        assert!(parsed.blocks.iter().any(|block| matches!(
            block,
            ResponseBlock::Quote { text } if text == "quoted line\nsecond line"
        )));
        let button = parsed.buttons().next().unwrap();
        assert_eq!(button.label, "Example");
        assert_eq!(button.prompt, "Show an example.");
    }

    #[test]
    fn malformed_and_over_limit_annotations_remain_visible_markdown() {
        let malformed = "<!-- mew:v4:button {not json} -->";
        let parsed = parse_response(malformed);
        assert_eq!(parsed.markdown, malformed);
        assert!(parsed.buttons().next().is_none());

        let too_long = format!(
            "<!-- mew:v4:button {{\"label\":\"{}\",\"prompt\":\"next\"}} -->",
            "x".repeat(MAX_BUTTON_LABEL_BYTES + 1)
        );
        assert_eq!(parse_response(&too_long).markdown, too_long);
    }

    #[test]
    fn does_not_interpret_protocol_syntax_inside_fenced_code() {
        let source = concat!(
            "```html\n",
            "<!-- mew:v4:button {\"label\":\"Run\",\"prompt\":\"Run it\"} -->\n",
            "```\n"
        );
        let parsed = parse_response(source);
        assert_eq!(parsed.markdown, source);
        assert!(parsed.buttons().next().is_none());
    }

    #[test]
    fn rejects_control_characters_in_button_fields() {
        let source = "<!-- mew:v4:button {\"label\":\"bad\\u001b[31m\",\"prompt\":\"next\"} -->";
        assert_eq!(parse_response(source).markdown, source);
    }

    #[test]
    fn model_guidance_keeps_markdown_as_the_base_and_actions_inert() {
        assert!(MODEL_INSTRUCTIONS.contains("ordinary Markdown"));
        assert!(MODEL_INSTRUCTIONS.contains("never tool calls"));
        assert!(MODEL_INSTRUCTIONS.contains("<!-- mew:v4:button"));
    }
}

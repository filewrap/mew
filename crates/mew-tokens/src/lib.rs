use mew_provider::ChatMessage;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub context_tokens: usize,
    pub response_reserve: usize,
}

impl Budget {
    pub fn input_limit(self) -> usize {
        self.context_tokens
            .saturating_sub(self.response_reserve)
            .max(512)
    }
}

/// Approximate tokens without a provider-specific tokenizer; provider usage wins when available.
pub fn estimate_text(text: &str) -> usize {
    text.chars().count().div_ceil(4).max(1)
}

pub fn estimate_messages(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|m| {
            estimate_text(&m.content)
                + estimate_text(&m.role)
                + m.tool_calls
                    .iter()
                    .map(|t| {
                        estimate_text(&t.id) + estimate_text(&t.name) + estimate_text(&t.arguments)
                    })
                    .sum::<usize>()
                + m.tool_call_id.as_deref().map(estimate_text).unwrap_or(0)
                + m.name.as_deref().map(estimate_text).unwrap_or(0)
                + 4
        })
        .sum()
}

/// Retain all system messages and the newest complete turns that fit.
/// Structured assistant tool-call messages and their following observations stay together.
pub fn compact_messages(messages: &[ChatMessage], budget: Budget) -> (Vec<ChatMessage>, usize) {
    if messages.is_empty() {
        return (Vec::new(), 0);
    }
    let limit = budget.input_limit();
    let systems: Vec<ChatMessage> = messages
        .iter()
        .filter(|m| m.role == "system")
        .cloned()
        .collect();
    let mut remaining = limit.saturating_sub(estimate_messages(&systems));

    let mut turns: Vec<Vec<ChatMessage>> = Vec::new();
    let mut current = Vec::new();
    for message in messages.iter().filter(|m| m.role != "system") {
        if message.role == "user" && !current.is_empty() {
            turns.push(std::mem::take(&mut current));
        }
        current.push(message.clone());
    }
    if !current.is_empty() {
        turns.push(current);
    }

    let mut kept_reversed = Vec::new();
    for turn in turns.iter().rev() {
        let cost = estimate_messages(turn);
        if cost <= remaining {
            remaining -= cost;
            kept_reversed.push(turn.clone());
            continue;
        }
        if kept_reversed.is_empty() {
            if let Some(compact) = fit_latest_turn(turn, remaining) {
                kept_reversed.push(compact);
            }
        }
        break;
    }

    kept_reversed.reverse();
    let mut compacted = systems;
    for turn in kept_reversed {
        compacted.extend(turn);
    }
    let estimated = estimate_messages(&compacted);
    (compacted, estimated)
}

fn fit_latest_turn(turn: &[ChatMessage], budget: usize) -> Option<Vec<ChatMessage>> {
    if turn.is_empty() {
        return None;
    }
    let mut result = Vec::new();
    let mut used = 0usize;
    let has_user = turn.first().is_some_and(|m| m.role == "user");
    let start = if has_user {
        let mut user = turn[0].clone();
        if estimate_messages(std::slice::from_ref(&user)) > budget {
            user.content = truncate_tokens(&user.content, budget.saturating_sub(8));
        }
        used = estimate_messages(std::slice::from_ref(&user));
        result.push(user);
        1
    } else {
        0
    };

    let mut segments: Vec<Vec<ChatMessage>> = Vec::new();
    for message in turn.iter().skip(start) {
        if message.role == "assistant" || segments.is_empty() {
            segments.push(Vec::new());
        }
        segments.last_mut().unwrap().push(message.clone());
    }
    let mut kept = Vec::new();
    for segment in segments.iter().rev() {
        let remaining = budget.saturating_sub(used);
        if let Some(segment) = fit_segment(segment, remaining) {
            used += estimate_messages(&segment);
            kept.push(segment);
        } else {
            break;
        }
    }
    kept.reverse();
    for segment in kept {
        result.extend(segment);
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn fit_segment(segment: &[ChatMessage], budget: usize) -> Option<Vec<ChatMessage>> {
    if estimate_messages(segment) <= budget {
        return Some(segment.to_vec());
    }
    if budget < 16 {
        return None;
    }
    let mut compact = segment.to_vec();
    // Reduce only observation/final-text content; never alter call IDs, names, or JSON arguments.
    for index in (0..compact.len()).rev() {
        {
            let message = &mut compact[index];
            if !matches!(message.role.as_str(), "tool" | "assistant") {
                continue;
            }
            let fixed = estimate_messages(std::slice::from_ref(message))
                .saturating_sub(estimate_text(&message.content));
            let allowance = budget.saturating_sub(fixed + 8).saturating_mul(4);
            if message.content.chars().count() > allowance {
                message.content = truncate_tokens(&message.content, allowance / 4);
            }
        }
        if estimate_messages(&compact) <= budget {
            return Some(compact);
        }
    }
    (estimate_messages(&compact) <= budget).then_some(compact)
}

fn truncate_tokens(text: &str, tokens: usize) -> String {
    let max_chars = tokens.saturating_mul(4);
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out = text
        .chars()
        .take(max_chars.saturating_sub(16))
        .collect::<String>();
    out.push_str("\n[context truncated]");
    out
}

pub fn usage_report(input: Option<u64>, output: Option<u64>, estimate: usize) -> String {
    match (input, output) {
        (Some(i), Some(o)) => format!("tokens=in:{i} out:{o}"),
        _ => format!("tokens≈{estimate} (input estimate; provider usage unavailable)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mew_provider::{user_message, ToolCall};

    fn system() -> ChatMessage {
        ChatMessage {
            role: "system".into(),
            content: "rules".into(),
            tool_calls: vec![],
            tool_call_id: None,
            name: None,
        }
    }

    #[test]
    fn compact_keeps_system_and_recent_turn() {
        let mut messages = vec![system()];
        for i in 0..20 {
            messages.push(user_message(format!("message {i} {}", "x".repeat(500))));
        }
        let (compact, _) = compact_messages(
            &messages,
            Budget {
                context_tokens: 700,
                response_reserve: 100,
            },
        );
        assert_eq!(compact.first().unwrap().role, "system");
        assert!(compact.last().unwrap().content.contains("message 19"));
        assert!(compact.len() < messages.len());
    }

    #[test]
    fn compaction_never_keeps_tool_observation_without_its_call() {
        let call = ChatMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: "t1".into(),
                name: "read_file".into(),
                arguments: "{\"path\":\"a\"}".into(),
            }],
            tool_call_id: None,
            name: None,
        };
        let mut messages = vec![
            system(),
            user_message("inspect"),
            call,
            ChatMessage {
                role: "tool".into(),
                content: "x".repeat(5_000),
                tool_calls: vec![],
                tool_call_id: Some("t1".into()),
                name: Some("read_file".into()),
            },
        ];
        let (compact, _) = compact_messages(
            &messages,
            Budget {
                context_tokens: 1_200,
                response_reserve: 200,
            },
        );
        let tool_result = compact.iter().position(|m| m.role == "tool");
        if let Some(index) = tool_result {
            assert!(compact[..index]
                .iter()
                .any(|m| m.role == "assistant" && m.tool_calls.iter().any(|call| call.id == "t1")));
        }
        messages[3].content = "small".into();
        let (compact, _) = compact_messages(
            &messages,
            Budget {
                context_tokens: 1_200,
                response_reserve: 200,
            },
        );
        assert!(compact.iter().any(|m| m.role == "tool"));
    }
}

use mew_common::MewConfig;
use mew_ui::{
    approval_panel, chat_banner, hint_card, render_markdown_light, startup_banner, theme_by_name,
    tool_card, visible_width,
};

#[test]
fn banner_renders() {
    let cfg = MewConfig::default();
    let theme = theme_by_name(&cfg.style.theme);
    let out = startup_banner(&theme, &cfg, "test");
    assert!(out.contains("mew"));
}

#[test]
fn chat_banner_rows_stay_aligned_for_long_paths() {
    let cfg = MewConfig::default();
    let theme = theme_by_name(&cfg.style.theme);
    let output = chat_banner(
        &theme,
        &cfg,
        "custom-provider/a-very-long-model-name",
        "/home/user/a/very/long/workspace/path/that/should/show/the/project-tail",
    );
    assert!(output.contains("Miku"));
    assert!(output.contains('█'));
    let widths = output.lines().map(visible_width).collect::<Vec<_>>();
    assert!(widths.iter().all(|width| *width == widths[0]));
}

#[test]
fn cards_render() {
    let theme = theme_by_name("crush-catppuccin");
    assert!(hint_card(&theme, &["hello"]).contains("hello"));
    let approval = approval_panel(&theme, "workspace change", "Review the exact diff.");
    assert!(approval.contains("approval"));
    assert!(approval.contains("Review the exact diff."));
    assert!(tool_card(&theme, "fs.read", "a.rs", "safe").contains("fs.read"));
}

#[test]
fn markdown_v4_annotations_render_as_suggestions() {
    let theme = theme_by_name("crush-catppuccin");
    let output = render_markdown_light(
        &theme,
        "Answer.\n\n> A quote.\n\n<!-- mew:v4:button {\"label\":\"Show example\",\"prompt\":\"Give a concrete example.\"} -->",
    );
    assert!(output.contains("A quote."));
    assert!(output.contains("Show example"));
    assert!(output.contains("Give a concrete example."));
    assert!(!output.contains("mew:v4:button"));
}

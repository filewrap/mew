use mew_common::MewConfig;
use mew_ui::{hint_card, startup_banner, theme_by_name, tool_card};

#[test]
fn banner_renders() {
    let cfg = MewConfig::default();
    let theme = theme_by_name(&cfg.style.theme);
    let out = startup_banner(&theme, &cfg, "test");
    assert!(out.contains("mew"));
}

#[test]
fn cards_render() {
    let theme = theme_by_name("crush-catppuccin");
    assert!(hint_card(&theme, &["hello"]).contains("hello"));
    assert!(tool_card(&theme, "fs.read", "a.rs", "safe").contains("fs.read"));
}

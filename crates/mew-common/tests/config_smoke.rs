use mew_common::MewConfig;

#[test]
fn default_config_is_mew() {
    let cfg = MewConfig::default();
    assert_eq!(cfg.identity.display_name, "mew");
    assert_eq!(cfg.providers.default, "openai");
}

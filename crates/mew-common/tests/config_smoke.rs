use mew_common::MewConfig;

#[test]
fn default_config_is_mew() {
    let cfg = MewConfig::default();
    assert_eq!(cfg.identity.display_name, "mew");
    assert_eq!(cfg.providers.default, "openai");
    assert_eq!(cfg.style.theme, "mew-dark");
    assert!(cfg.trusted_workspaces.is_empty());
    assert_eq!(cfg.tui.keybindings["command_palette"], "ctrl+p,f1");
    assert_eq!(cfg.tui.keybindings["complete"], "tab,f2");
    let default_model = cfg
        .providers
        .openai
        .models
        .iter()
        .find(|model| model.id == "codex-mini-latest")
        .expect("the default model is configured");
    assert!(default_model.supports_tools);
}

#[test]
fn workspace_trust_is_exact_path_scoped_and_deduplicated() {
    let mut cfg = MewConfig::default();
    let project = std::path::Path::new("/work/project");
    let child = std::path::Path::new("/work/project/child");

    assert!(!cfg.workspace_is_trusted(project));
    cfg.trust_workspace(project);
    cfg.trust_workspace(project);

    assert!(cfg.workspace_is_trusted(project));
    assert!(!cfg.workspace_is_trusted(child));
    assert_eq!(cfg.trusted_workspaces, ["/work/project"]);
}

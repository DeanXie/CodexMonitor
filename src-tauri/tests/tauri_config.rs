use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn read_json(path: &PathBuf) -> Value {
    let contents =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("Failed to read {path:?}: {error}"));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("Failed to parse {path:?}: {error}"))
}

#[test]
fn macos_private_api_feature_matches_config() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = manifest_dir.join("tauri.conf.json");
    let config_contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("Failed to read {config_path:?}: {error}"));
    let config: Value = serde_json::from_str(&config_contents)
        .unwrap_or_else(|error| panic!("Failed to parse tauri.conf.json: {error}"));
    let macos_private_api = config
        .get("app")
        .and_then(|app| app.get("macOSPrivateApi"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    if macos_private_api {
        let cargo_path = manifest_dir.join("Cargo.toml");
        let cargo_contents = fs::read_to_string(&cargo_path)
            .unwrap_or_else(|error| panic!("Failed to read {cargo_path:?}: {error}"));
        let mut in_dependencies = false;
        let mut has_feature = false;

        for line in cargo_contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_dependencies = trimmed == "[dependencies]";
                continue;
            }
            if !in_dependencies {
                continue;
            }
            if trimmed.starts_with("tauri") && trimmed.contains("macos-private-api") {
                has_feature = true;
                break;
            }
        }

        assert!(
            has_feature,
            "Cargo.toml [dependencies] must enable macos-private-api when app.macOSPrivateApi is true"
        );
    }
}

#[test]
fn custom_distribution_disables_updater_in_base_and_windows_configs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for file_name in ["tauri.conf.json", "tauri.windows.conf.json"] {
        let path = manifest_dir.join(file_name);
        let config = read_json(&path);
        assert_eq!(
            config.pointer("/bundle/createUpdaterArtifacts"),
            Some(&Value::Bool(false)),
            "{file_name} must not create updater artifacts"
        );
        assert!(
            config.pointer("/plugins/updater").is_none(),
            "{file_name} must not configure an updater endpoint or key"
        );
    }

    let capabilities = read_json(&manifest_dir.join("capabilities/default.json"));
    let serialized = serde_json::to_string(&capabilities).expect("serialize capabilities");
    assert!(
        !serialized.contains("updater:"),
        "desktop capabilities must not expose updater commands"
    );
    assert!(
        !serialized.contains("process:"),
        "the updater-only relaunch capability must not remain exposed"
    );

    let cargo_manifest =
        fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read Cargo.toml");
    let app_entry = fs::read_to_string(manifest_dir.join("src/lib.rs")).expect("read app entry");
    assert!(!cargo_manifest.contains("tauri-plugin-updater"));
    assert!(!cargo_manifest.contains("tauri-plugin-process"));
    assert!(!app_entry.contains("tauri_plugin_updater::init"));
    assert!(!app_entry.contains("tauri_plugin_process::init"));
}

#[test]
fn release_workflow_keeps_installers_and_notes_without_updater_manifest_chain() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workflow_path = manifest_dir.join("../.github/workflows/release.yml");
    let workflow = fs::read_to_string(&workflow_path)
        .unwrap_or_else(|error| panic!("Failed to read {workflow_path:?}: {error}"));

    assert!(workflow.contains("release-artifacts/release-notes.md"));
    assert!(workflow.contains("windows-artifacts"));
    assert!(workflow.contains("linux-bundles-"));
    assert!(workflow.contains("macos-artifacts"));
    assert!(!workflow.contains("latest.json"));
    assert!(!workflow.contains("TAURI_SIGNING_PRIVATE_KEY"));
    assert!(!workflow.contains("tauri signer sign"));
}

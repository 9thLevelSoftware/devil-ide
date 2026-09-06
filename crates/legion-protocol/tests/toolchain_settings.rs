use legion_protocol::{
    CanonicalPath, LanguageToolchainSettingsRecord, TypeScriptToolchainSettings,
    WorkspaceSessionRecord,
};

#[test]
fn language_toolchain_settings_round_trip_metadata_only() {
    let settings = LanguageToolchainSettingsRecord {
        schema_version: 1,
        typescript: Some(TypeScriptToolchainSettings {
            server_archive: CanonicalPath("C:/bundles/typescript-language-server.tgz".into()),
            compiler_archive: CanonicalPath("C:/bundles/typescript.tgz".into()),
            node_executable: CanonicalPath("C:/node/node.exe".into()),
        }),
    };
    let value = serde_json::to_value(&settings).expect("serialize settings");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(
        value["typescript"]["server_archive"],
        "C:/bundles/typescript-language-server.tgz"
    );
    assert!(value.get("cache_path").is_none());
    assert!(value.get("hash").is_none());
    assert!(value.get("grant").is_none());
    assert_eq!(
        serde_json::from_value::<LanguageToolchainSettingsRecord>(value).expect("round trip"),
        settings
    );
}

#[test]
fn legacy_session_without_toolchain_settings_defaults_to_schema_one_none() {
    let legacy = serde_json::json!({
        "session_id": "legacy",
        "last_workspace": null,
        "last_workspace_path": null,
        "open_tabs": [],
        "active_tab": null,
        "active_buffer": null,
        "tab_groups": [],
        "layout_splits": [],
        "explorer_expansion": [],
        "panel_state": {
            "bottom_visible": false,
            "side_visible": true,
            "active_panel": null,
            "bottom_height_px": null,
            "side_width_px": null
        },
        "dirty_indicators": [],
        "saved_at": 0,
        "schema_version": 1
    });
    let session: WorkspaceSessionRecord = serde_json::from_value(legacy).expect("legacy session");
    assert_eq!(
        session.language_toolchain_settings,
        LanguageToolchainSettingsRecord::default()
    );
}

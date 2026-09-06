//! Focused approval-lifecycle tests for the local TypeScript bundle APIs.
//!
//! These tests call the app's real startup authority after configuration and
//! observe the real broker decision without spawning a process.

use std::path::{Path, PathBuf};

use super::*;
use crate::language::LanguageStartupContext;
use legion_lsp::LspServerProcessConfig;
use legion_protocol::{
    CausalityId, CorrelationId, LanguageId, LanguageServerId, PrincipalId, WorkspaceTrustState,
};

fn fixture_files() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("fixture directory");
    let server = dir.path().join("server.tgz");
    let compiler = dir.path().join("compiler.tgz");
    let old_node = dir.path().join(if cfg!(windows) {
        "old-node.exe"
    } else {
        "old-node"
    });
    let new_node = dir.path().join(if cfg!(windows) {
        "new-node.exe"
    } else {
        "new-node"
    });
    for path in [&server, &compiler, &old_node, &new_node] {
        std::fs::write(path, b"fixture").expect("fixture file");
    }
    (dir, server, compiler, old_node, new_node)
}

fn open_trusted_app(root: &Path) -> AppComposition {
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("toolchain-approval-test".to_string()),
    )
    .expect("open trusted workspace");
    app
}

fn startup_context(app: &AppComposition) -> LanguageStartupContext {
    let opened = app
        .active_documents
        .opened_workspace
        .as_ref()
        .expect("workspace opened");
    let root = std::fs::canonicalize(
        app.active_documents
            .workspace_root_path
            .as_ref()
            .expect("workspace root"),
    )
    .expect("canonical workspace root");
    LanguageStartupContext {
        workspace_id: opened.workspace_id,
        root_id: opened.root_id,
        workspace_root: root,
        principal_id: app
            .active_documents
            .active_principal_id
            .clone()
            .expect("principal"),
        trust: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::from_u128(1)),
    }
}

fn broker_allows_command(app: &AppComposition, command: &Path) -> bool {
    let context = startup_context(app);
    let root_uri = crate::canonical_path_to_uri(
        context
            .workspace_root
            .to_str()
            .expect("workspace root UTF-8"),
    );
    app.language_startup_authority
        .prepare_configured(
            &context,
            LanguageServerId(102),
            LanguageId("typescript".to_string()),
            "approval-test",
            LspServerProcessConfig {
                command: std::fs::canonicalize(command)
                    .expect("canonical command")
                    .to_string_lossy()
                    .into_owned(),
                args: Vec::new(),
                cwd: Some(context.workspace_root.clone()),
                env: Vec::new(),
            },
            root_uri,
            None,
            None,
        )
        .is_ok()
}

#[test]
fn manual_bundle_replacement_and_clear_revoke_exact_node_grants() {
    let (dir, server, compiler, old_node, new_node) = fixture_files();
    let mut app = open_trusted_app(dir.path());

    app.configure_typescript_bundle(
        LanguageServerId(102),
        server.clone(),
        compiler.clone(),
        old_node.clone(),
        dir.path().join("cache-old"),
    )
    .expect("configure old manual bundle");
    assert!(broker_allows_command(&app, &old_node));

    std::fs::remove_file(&old_node).expect("remove old Node fixture");
    app.configure_typescript_bundle(
        LanguageServerId(102),
        server,
        compiler,
        new_node.clone(),
        dir.path().join("cache-new"),
    )
    .expect("replace manual bundle");
    std::fs::write(&old_node, b"recreated old fixture").expect("recreate old Node fixture");
    assert!(
        !broker_allows_command(&app, &old_node),
        "recreated path must remain denied after replacement revocation"
    );
    assert!(broker_allows_command(&app, &new_node));

    app.clear_typescript_toolchain();
    assert!(
        !broker_allows_command(&app, &new_node),
        "cleared manual Node grant must be revoked"
    );
}

#[test]
fn unrelated_configured_server_keeps_a_shared_node_grant() {
    let (dir, server, compiler, node, new_node) = fixture_files();
    let mut app = open_trusted_app(dir.path());
    app.configure_language_server_binary(LanguageServerId(101), node.clone())
        .expect("configure unrelated server");
    app.configure_typescript_bundle(
        LanguageServerId(102),
        server,
        compiler,
        node.clone(),
        dir.path().join("cache"),
    )
    .expect("configure shared Node bundle");
    app.clear_typescript_toolchain();
    assert!(
        broker_allows_command(&app, &node),
        "unrelated configured server still owns the shared grant"
    );
    assert!(!broker_allows_command(&app, &new_node));
}

#[test]
fn normal_configuration_is_atomic_and_populates_all_typescript_family_maps() {
    let (dir, server, compiler, node, new_node) = fixture_files();
    let mut app = open_trusted_app(dir.path());
    let node_c = dir.path().join(if cfg!(windows) {
        "replacement-node.exe"
    } else {
        "replacement-node"
    });
    std::fs::write(&node_c, b"replacement fixture").expect("replacement Node fixture");
    let legacy_b = dir.path().join(if cfg!(windows) {
        "legacy-b.exe"
    } else {
        "legacy-b"
    });
    std::fs::write(&legacy_b, b"legacy fixture").expect("legacy Node fixture");
    app.configure_typescript_bundle(
        LanguageServerId(102),
        server.clone(),
        compiler.clone(),
        node.clone(),
        dir.path().join("legacy-cache-a"),
    )
    .expect("configure first legacy bundle");
    app.configure_typescript_bundle(
        LanguageServerId(106),
        server.clone(),
        compiler.clone(),
        legacy_b.clone(),
        dir.path().join("legacy-cache-b"),
    )
    .expect("configure second legacy bundle");
    app.configure_typescript_toolchain(&server, &compiler, &node_c)
        .expect("configure normal toolchain");
    let before = app.language_toolchain_settings();
    assert_eq!(app.typescript_bundles.len(), 4);
    for id in [102_u64, 106, 107, 108] {
        assert!(app.typescript_bundles.contains_key(&LanguageServerId(id)));
    }
    assert!(!broker_allows_command(&app, &node));
    assert!(
        !broker_allows_command(&app, &legacy_b),
        "normal replacement must revoke every replaced legacy Node"
    );
    assert!(broker_allows_command(&app, &node_c));

    let missing = dir.path().join("missing-compiler.tgz");
    assert!(
        app.configure_typescript_toolchain(&server, &missing, &new_node)
            .is_err()
    );
    assert_eq!(app.language_toolchain_settings(), before);
    assert_eq!(app.typescript_bundles.len(), 4);
    assert!(broker_allows_command(&app, &node_c));
    assert!(!broker_allows_command(&app, &node));
    assert!(!broker_allows_command(&app, &legacy_b));
    assert!(!broker_allows_command(&app, &new_node));
}

//! Opt-in product startup coverage for a locally materialized Pyright adapter.
//!
//! Run explicitly with the retained archive and an operator-selected Node:
//! `LEGION_TEST_NODE_RUNTIME=<absolute-node> cargo test -p legion-app
//! --test python_app_startup -- --ignored --nocapture`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use legion_app::AppComposition;
use legion_editor::{TextEdit, TextPosition};
use legion_lsp::LanguageServerAdapterRegistry;
use legion_protocol::{
    LanguageId, LspResultStatus, LspSessionLifecycleKind, PrincipalId, TextCoordinate,
    WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn retained_archive() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.superpowers/sdd/2026-09-04-full-product-completion/pyright-1.1.400.tgz");
    assert!(
        path.is_file(),
        "retained Pyright archive is required: {}",
        path.display()
    );
    path
}

fn selected_node() -> PathBuf {
    let path = PathBuf::from(std::env::var("LEGION_TEST_NODE_RUNTIME").expect(
        "LEGION_TEST_NODE_RUNTIME must select an absolute Node executable for this opt-in test",
    ));
    assert!(path.is_absolute(), "selected Node path must be absolute");
    assert!(path.is_file(), "selected Node executable must be a file");
    std::fs::canonicalize(path).expect("canonicalize selected Node executable")
}

fn wait_for_live(app: &mut AppComposition) {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        app.drain_lsp_session();
        let health = app.lsp_server_health_record();
        if health
            .as_ref()
            .is_some_and(|record| record.init_status == LspResultStatus::Fresh)
        {
            return;
        }
        let lifecycle = app.lsp_session_status_projection().lifecycle;
        assert_ne!(
            lifecycle,
            LspSessionLifecycleKind::Refused,
            "Pyright startup was refused: status={:?}, health={:?}, stderr={:?}",
            app.lsp_session_status_projection(),
            app.lsp_server_health_record(),
            app.lsp_session_log_projection(),
        );
        assert_ne!(
            lifecycle,
            LspSessionLifecycleKind::Failed,
            "Pyright startup failed: status={:?}, health={:?}, stderr={:?}",
            app.lsp_session_status_projection(),
            app.lsp_server_health_record(),
            app.lsp_session_log_projection(),
        );
        assert!(Instant::now() < deadline, "Pyright startup timed out");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
#[ignore = "opt-in native Node + retained Pyright fixture"]
fn explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text() {
    let archive = retained_archive();
    let node = selected_node();
    let root = tempfile::tempdir().expect("temporary Python workspace");
    let source = root.path().join("main.py");
    std::fs::write(&source, "def greet(name):\n    return name\n").expect("seed Python source");
    let cache_root = root.path().join("language-cache");

    let adapter = LanguageServerAdapterRegistry::tier_two()
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .find(|candidate| candidate.is_primary)
        .cloned()
        .expect("tier-two Python adapter");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("python-startup-test".to_string()),
    )
    .expect("open workspace");
    app.configure_downloaded_language_server_local(adapter, archive, node, cache_root)
        .expect("configure local Pyright");

    app.open_file(source.to_string_lossy())
        .expect("open Python file");
    let buffer_id = app
        .active_buffer_id()
        .expect("active Python buffer after open");
    assert_eq!(
        app.lsp_session_status_projection().lifecycle,
        LspSessionLifecycleKind::Idle,
        "opening a Python file must not implicitly start the server"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
        .expect("explicit start dispatch");
    wait_for_live(&mut app);
    let health = app.lsp_server_health_record().expect("live Python health");
    assert_eq!(health.language_id, LanguageId("python".to_string()));
    assert_eq!(health.init_status, LspResultStatus::Fresh);

    let completion_position = TextCoordinate {
        line: 1,
        character: 15,
        byte_offset: None,
        utf16_offset: None,
    };
    app.dispatch_ui_intent(CommandDispatchIntent::RequestCompletion {
        buffer_id,
        position: completion_position,
    })
    .expect("completion request dispatch");
    let completion_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        assert!(
            Instant::now() < completion_deadline,
            "Python completion did not arrive"
        );
        if !app.language_tooling_projection().completions.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    let hover_position = TextCoordinate {
        line: 0,
        character: 5,
        byte_offset: None,
        utf16_offset: None,
    };
    app.dispatch_ui_intent(CommandDispatchIntent::RequestHover {
        buffer_id,
        position: hover_position,
    })
    .expect("hover request dispatch");
    let hover_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.drain_lsp_session();
        assert!(
            Instant::now() < hover_deadline,
            "Python hover did not arrive"
        );
        if app.language_tooling_projection().hover.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(1, 15), " + \"!\""))
        .expect("make Python buffer dirty");
    let dirty_text = app
        .buffer_text_for_input(buffer_id)
        .expect("read dirty buffer text");
    assert!(dirty_text.contains("+ \"!\""));

    app.dispatch_ui_intent(CommandDispatchIntent::LspRestartSession)
        .expect("explicit restart dispatch");
    wait_for_live(&mut app);
    assert_eq!(
        app.buffer_text_for_input(buffer_id)
            .expect("dirty text after restart request"),
        dirty_text,
        "restart must not discard dirty editor text"
    );
}

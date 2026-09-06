use legion_lsp::{
    LanguageServerAdapterRegistry, LspArtifactRuntime, LspDownloadedArtifactMetadata,
    LspDownloadedArtifactResolveError, LspNodeVersion, LspServerBinaryManifest,
    LspServerBinarySource,
};
use legion_protocol::{LanguageId, WorkspaceId};

#[test]
fn registry_rebinds_catalog_to_real_workspace_without_mutating_catalog() {
    let catalog = LanguageServerAdapterRegistry::tier_two();
    let bound = catalog
        .for_workspace(WorkspaceId(77))
        .expect("nonzero workspace identity should bind");

    let python = bound
        .adapters_for_workspace_language(WorkspaceId(77), &LanguageId("python".to_string()))
        .expect("bound Python plans should be selectable");
    assert_eq!(python.len(), 1);
    assert_eq!(python[0].workspace_id, WorkspaceId(77));
    assert!(python[0].is_primary);

    let typescript = bound
        .adapters_for_workspace_language(WorkspaceId(77), &LanguageId("typescript".to_string()))
        .expect("bound TypeScript plans should be selectable");
    assert_eq!(typescript.len(), 2);
    assert!(typescript[0].is_primary);
    assert!(!typescript[1].is_primary);

    let javascript = bound
        .adapters_for_workspace_language(WorkspaceId(77), &LanguageId("javascript".to_string()))
        .expect("bound JavaScript plan should be selectable");
    assert_eq!(javascript.len(), 1);
    assert!(javascript[0].is_primary);
    assert_eq!(javascript[0].server_id.0, 106);
    assert_eq!(
        javascript[0].language_id,
        LanguageId("javascript".to_string())
    );

    let original = catalog
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .next()
        .expect("catalog Python plan should remain available");
    assert_eq!(original.workspace_id, WorkspaceId(1));

    assert!(
        bound
            .adapters_for_workspace_language(WorkspaceId(0), &LanguageId("python".to_string()))
            .is_err()
    );
    assert!(catalog.for_workspace(WorkspaceId(0)).is_err());
}

#[test]
fn tier_two_registry_covers_the_expected_language_smoke_set() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);

    let rust = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("rust".to_string()))
        .expect("system adapter should resolve");
    assert_eq!(rust.len(), 1);
    let expected_rust_command = std::env::var("CARGO_BIN_EXE_mock_lsp_server")
        .unwrap_or_else(|_| "rust-analyzer".to_string());
    assert_eq!(rust[0].command, expected_rust_command);
    assert!(rust[0].args.is_empty());

    let typescript = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("typescript".to_string()))
        .expect("system adapters should resolve");
    assert_eq!(typescript.len(), 2);
    assert_eq!(typescript[0].command, "typescript-language-server");
    assert_eq!(typescript[0].args, vec!["--stdio".to_string()]);
    assert_eq!(typescript[1].command, "tailwindcss-language-server");
    assert_eq!(typescript[1].args, vec!["--stdio".to_string()]);

    let javascript = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("javascript".to_string()))
        .expect("JavaScript should reuse the TypeScript server command");
    assert_eq!(javascript.len(), 1);
    assert_eq!(javascript[0].command, "typescript-language-server");
    assert_eq!(javascript[0].args, vec!["--stdio".to_string()]);
    let javascript_react = registry
        .process_configs_for_workspace_language(
            workspace_id,
            &LanguageId("javascriptreact".to_string()),
        )
        .expect("JSX should reuse the TypeScript server command");
    assert_eq!(javascript_react.len(), 1);
    assert_eq!(javascript_react[0].command, "typescript-language-server");
    let typescript_react = registry
        .process_configs_for_workspace_language(
            workspace_id,
            &LanguageId("typescriptreact".to_string()),
        )
        .expect("TSX should reuse the TypeScript server command");
    assert_eq!(typescript_react.len(), 1);
    assert_eq!(typescript_react[0].command, "typescript-language-server");

    let python = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("python".to_string()));
    assert!(matches!(
        python,
        Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
    ));

    let go = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("go".to_string()))
        .expect("system adapter should resolve");
    assert_eq!(go.len(), 1);
    assert_eq!(go[0].command, "gopls");
    assert!(go[0].args.is_empty());
}

#[test]
fn downloaded_artifact_entries_keep_binary_policy_metadata() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let python_adapter = registry
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .next()
        .expect("python adapter should exist");

    match &python_adapter.binary_source {
        LspServerBinarySource::DownloadedArtifact {
            binary_name,
            artifact_uri,
            checksum_sha256,
            policy_gate,
            metadata,
        } => {
            assert_eq!(binary_name, "pyright-langserver");
            assert_eq!(
                artifact_uri,
                "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz"
            );
            assert_eq!(
                checksum_sha256,
                "2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a"
            );
            assert_eq!(policy_gate, "policy://lsp-download/pyright");
            assert_eq!(metadata.version, "1.1.400");
            assert_eq!(metadata.package_name, "pyright");
            assert_eq!(metadata.archive_format, "tar.gz");
            assert_eq!(metadata.package_root, std::path::Path::new("package"));
            assert_eq!(
                metadata.entrypoint,
                std::path::Path::new("langserver.index.js")
            );
            assert_eq!(
                metadata.runtime,
                LspArtifactRuntime::Node {
                    minimum_version: LspNodeVersion {
                        major: 14,
                        minor: 0,
                        patch: 0,
                    }
                }
            );
        }
        other => panic!("expected downloaded artifact source, got {other:?}"),
    }

    assert!(matches!(
        python_adapter.process_config(),
        Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
    ));
}

#[test]
fn air_gap_manifest_denies_downloads_but_keeps_system_binaries() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);

    let rust = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("rust".to_string()),
        true,
    );
    let expected_rust_command = std::env::var("CARGO_BIN_EXE_mock_lsp_server")
        .unwrap_or_else(|_| "rust-analyzer".to_string());
    assert_manifest_system_path_only(&rust, &expected_rust_command);

    let python = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("python".to_string()),
        true,
    );
    assert!(python.entries.is_empty());
    assert_eq!(python.denied_downloads.len(), 1);
    assert!(python.denied_downloads[0].contains("pyright"));
    assert!(python.denied_downloads[0].contains("policy://lsp-download/pyright"));
}

#[test]
fn manifest_records_workspace_version_pin_for_downloaded_artifacts() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);

    let python = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("python".to_string()),
        false,
    );
    assert_eq!(python.entries.len(), 1);
    let entry = &python.entries[0];
    assert_eq!(entry.workspace_version_pin.as_deref(), Some("workspace/1"));
    match &entry.binary_source {
        LspServerBinarySource::DownloadedArtifact {
            binary_name,
            artifact_uri,
            checksum_sha256,
            policy_gate,
            metadata,
        } => {
            assert_eq!(binary_name, "pyright-langserver");
            assert_eq!(
                artifact_uri,
                "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz"
            );
            assert_eq!(
                checksum_sha256,
                "2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a"
            );
            assert_eq!(policy_gate, "policy://lsp-download/pyright");
            assert_eq!(metadata.version, "1.1.400");
        }
        other => panic!("expected downloaded artifact source, got {other:?}"),
    }
}

#[test]
fn downloaded_pyright_resolves_to_node_and_absolute_entrypoint() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let adapter = registry.adapters_for_language(&LanguageId("python".to_string()))[0];
    let root_path =
        std::env::temp_dir().join(format!("legion-lsp-registry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root_path);
    std::fs::create_dir_all(root_path.join("package")).expect("package");
    std::fs::write(root_path.join("package/langserver.index.js"), b"entry").expect("entrypoint");
    let node = root_path.join("node");
    std::fs::write(&node, b"node").expect("node");
    assert!(matches!(
        adapter.resolve_downloaded_process(&root_path, &node, "13.9.0"),
        Err(LspDownloadedArtifactResolveError::RuntimeTooOld { .. })
    ));
    let config = adapter
        .resolve_downloaded_process(&root_path, &node, "v18.20.0")
        .expect("materialized package should resolve");
    let entrypoint_argument = |path: &std::path::Path| {
        let value = path.canonicalize().unwrap().to_string_lossy().into_owned();
        if cfg!(windows)
            && let Some(rest) = value.strip_prefix("\\\\?\\")
            && rest.len() >= 2
            && rest.as_bytes()[1] == b':'
        {
            return rest.to_string();
        }
        if cfg!(windows)
            && let Some(rest) = value.strip_prefix("\\\\?\\")
            && let Some(unc) = rest.strip_prefix("UNC\\")
        {
            return format!("\\\\{unc}");
        }
        value
    };
    assert_eq!(
        config.command,
        node.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(
        config.args[0],
        entrypoint_argument(&root_path.join("package/langserver.index.js"))
    );
    assert_eq!(config.args[1], "--stdio");
    std::fs::remove_dir_all(root_path).expect("cleanup");
}

#[test]
fn downloaded_resolver_rejects_unmaterialized_and_escaping_paths() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let adapter = registry.adapters_for_language(&LanguageId("python".to_string()))[0];
    let root_path = std::env::temp_dir().join(format!(
        "legion-lsp-registry-missing-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root_path);
    std::fs::create_dir_all(&root_path).expect("artifact root");
    let node = root_path.join("node");
    std::fs::write(&node, b"node").expect("node");
    assert!(matches!(
        adapter.resolve_downloaded_process(&root_path, &node, "18.0.0"),
        Err(LspDownloadedArtifactResolveError::MissingPath {
            field: "package_root",
            ..
        })
    ));
    let registry = LanguageServerAdapterRegistry::tier_two();
    let system = registry.adapters_for_language(&LanguageId("rust".to_string()))[0];
    assert!(matches!(
        system.resolve_downloaded_process(&root_path, &node, "18.0.0"),
        Err(LspDownloadedArtifactResolveError::NotDownloadedArtifact)
    ));
    let escaping = legion_lsp::LanguageServerAdapterPlan::downloaded_package_artifact(
        legion_protocol::LanguageServerId(900),
        WorkspaceId(1),
        LanguageId("python".into()),
        "escaping",
        "pyright-langserver",
        "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz",
        "hash",
        "policy://test",
        LspDownloadedArtifactMetadata {
            package_name: "test".into(),
            version: "test".into(),
            archive_format: "tar.gz".into(),
            package_root: "../escape".into(),
            entrypoint: "entry.js".into(),
            runtime: LspArtifactRuntime::Node {
                minimum_version: LspNodeVersion {
                    major: 14,
                    minor: 0,
                    patch: 0,
                },
            },
        },
        vec!["--stdio".into()],
        true,
    );
    assert!(matches!(
        escaping.resolve_downloaded_process(&root_path, &node, "18.0.0"),
        Err(LspDownloadedArtifactResolveError::UnsafePath {
            field: "package_root"
        })
    ));
    std::fs::remove_dir_all(root_path).expect("cleanup");
}

#[test]
fn node_version_parser_enforces_minimum_and_rejects_malformed_values() {
    assert_eq!(
        LspNodeVersion::parse("v18.20.0\n").expect("LF version"),
        LspNodeVersion {
            major: 18,
            minor: 20,
            patch: 0,
        }
    );
    assert_eq!(
        LspNodeVersion::parse("v18.20.0\r\n").expect("CRLF version"),
        LspNodeVersion {
            major: 18,
            minor: 20,
            patch: 0,
        }
    );
    assert_eq!(
        LspNodeVersion::parse("v18.20.0").expect("valid version"),
        LspNodeVersion {
            major: 18,
            minor: 20,
            patch: 0,
        }
    );
    assert!(matches!(
        LspNodeVersion::parse("18.20"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("18.20.0.1"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("v18.20.0\nextra"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("v18.20.0-pre"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("v18.20.0 trailing"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
}

#[test]
fn resolver_preserves_extra_arguments_after_entrypoint() {
    let adapter = legion_lsp::LanguageServerAdapterPlan::downloaded_package_artifact(
        legion_protocol::LanguageServerId(901),
        WorkspaceId(1),
        LanguageId("python".into()),
        "extra-args",
        "pyright-langserver",
        "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz",
        "hash",
        "policy://test",
        LspDownloadedArtifactMetadata {
            package_name: "pyright".into(),
            version: "1.1.400".into(),
            archive_format: "tar.gz".into(),
            package_root: "package".into(),
            entrypoint: "langserver.index.js".into(),
            runtime: LspArtifactRuntime::Node {
                minimum_version: LspNodeVersion {
                    major: 14,
                    minor: 0,
                    patch: 0,
                },
            },
        },
        vec!["--stdio".into(), "--verbose".into()],
        true,
    );
    let root = std::env::temp_dir().join(format!("legion-lsp-extra-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("package")).expect("package");
    std::fs::write(root.join("package/langserver.index.js"), b"entry").expect("entrypoint");
    let node = root.join("node");
    std::fs::write(&node, b"node").expect("node");
    let config = adapter
        .resolve_downloaded_process(&root, &node, "18.0.0")
        .expect("resolver");
    assert_eq!(config.args[1..], ["--stdio", "--verbose"]);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn resolver_rejects_symlinked_package_escape_and_non_utf8_runtime_path() {
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!("legion-lsp-symlink-{}", std::process::id()));
    let outside = std::env::temp_dir().join(format!("legion-lsp-outside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&root).expect("root");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::write(outside.join("langserver.index.js"), b"entry").expect("outside entrypoint");
    symlink(&outside, root.join("package")).expect("package symlink");
    let node = root.join("node");
    std::fs::write(&node, b"node").expect("node");
    let registry = LanguageServerAdapterRegistry::tier_two();
    let adapter = registry.adapters_for_language(&LanguageId("python".into()))[0];
    assert!(matches!(
        adapter.resolve_downloaded_process(&root, &node, "18.0.0"),
        Err(LspDownloadedArtifactResolveError::WrongPathKind {
            field: "package_root",
            ..
        })
    ));
    std::fs::remove_file(root.join("package")).expect("remove package symlink");
    std::fs::create_dir(root.join("package")).expect("real package");
    std::fs::write(root.join("package/langserver.index.js"), b"entry").expect("entrypoint");
    let non_utf8_node = root.join(std::ffi::OsString::from_vec(vec![
        b'n', b'o', b'd', b'e', 0xff,
    ]));
    match std::fs::write(&non_utf8_node, b"node") {
        Ok(()) => {
            assert!(matches!(
                adapter.resolve_downloaded_process(&root, &non_utf8_node, "18.0.0"),
                Err(LspDownloadedArtifactResolveError::NonUtf8Path {
                    field: "approved_node"
                })
            ));
        }
        Err(error) => {
            // macOS rejects non-UTF-8 filenames at the filesystem, which is a
            // stronger form of the same gate the resolver enforces on Linux.
            assert!(cfg!(target_os = "macos"), "non-utf8 node: {error:?}");
        }
    }
    std::fs::remove_dir_all(root).expect("cleanup root");
    std::fs::remove_dir_all(outside).expect("cleanup outside");
}

fn assert_manifest_system_path_only(manifest: &LspServerBinaryManifest, expected_command: &str) {
    assert_eq!(manifest.entries.len(), 1);
    assert!(manifest.denied_downloads.is_empty());
    let entry = &manifest.entries[0];
    assert_eq!(entry.workspace_version_pin, None);
    match &entry.binary_source {
        LspServerBinarySource::SystemPath { binary_name } => {
            assert_eq!(binary_name, expected_command);
        }
        other => panic!("expected system path source, got {other:?}"),
    }
}

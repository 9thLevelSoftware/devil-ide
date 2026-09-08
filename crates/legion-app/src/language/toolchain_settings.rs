//! Local TypeScript toolchain settings: configure, read, inspect state, clear.
//!
//! Moved verbatim out of `lib.rs` for the chokepoint budget (cross-cutting
//! rule 1). Nothing here changed in the move: the enum, the four inherent
//! `AppComposition` methods and the approval tests are the same bytes that
//! lived in `lib.rs`, and `LanguageToolchainConfigurationState` is re-exported
//! from the crate root so `legion_app::LanguageToolchainConfigurationState`
//! keeps resolving.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{AppComposition, AppCompositionError, TypeScriptBundleStartup};
use legion_protocol::{
    CanonicalPath, LanguageServerId, LanguageToolchainSettingsRecord, LanguageToolingStatusKind,
    ProtocolError, TypeScriptToolchainSettings, WorkspaceTrustState,
};

#[cfg(test)]
#[path = "toolchain_approval_tests.rs"]
mod toolchain_approval_tests;

/// Non-persistent state of the normal TypeScript toolchain controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageToolchainConfigurationState {
    /// No saved operator input exists.
    Unconfigured,
    /// Saved metadata exists but has not been explicitly approved in this app
    /// session (including unsupported schema versions).
    Draft,
    /// Metadata and the app-owned runtime bindings are configured.
    Configured,
}

impl AppComposition {
    /// Configure the operator-selected local TypeScript toolchain for the
    /// active trusted workspace.
    ///
    /// This records only canonical input metadata. It does not materialize
    /// either archive or start a process; each explicit start/restart creates
    /// fresh artifact and Node receipts through the language authority.
    pub fn configure_typescript_toolchain(
        &mut self,
        server_archive: impl AsRef<Path>,
        compiler_archive: impl AsRef<Path>,
        node_executable: impl AsRef<Path>,
    ) -> Result<(), AppCompositionError> {
        if self.active_documents.workspace_id().is_none() {
            return Err(AppCompositionError::WorkspaceNotOpen);
        }
        if self.active_documents.active_workspace_trust != Some(WorkspaceTrustState::Trusted) {
            return Err(AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_workspace_untrusted".to_string(),
                message: "TypeScript toolchains require a trusted workspace".to_string(),
            }));
        }

        fn canonical_regular_file(
            path: &Path,
            label: &str,
        ) -> Result<PathBuf, AppCompositionError> {
            let canonical = std::fs::canonicalize(path).map_err(|error| {
                AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_input_invalid".to_string(),
                    message: format!("{label} is invalid: {error}"),
                })
            })?;
            if !canonical.is_file() {
                return Err(AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_input_invalid".to_string(),
                    message: format!("{label} must be a regular file"),
                }));
            }
            if canonical.to_str().is_none() {
                return Err(AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_input_invalid".to_string(),
                    message: format!("{label} path is not valid UTF-8"),
                }));
            }
            Ok(canonical)
        }

        // Validate every input before changing the policy store or replacing
        // the existing configuration, so an invalid third path is atomic.
        let server_archive = canonical_regular_file(server_archive.as_ref(), "server archive")?;
        let compiler_archive =
            canonical_regular_file(compiler_archive.as_ref(), "compiler archive")?;
        let node_executable = canonical_regular_file(node_executable.as_ref(), "Node executable")?;
        let root = self
            .active_documents
            .workspace_root_path
            .as_deref()
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;
        let root = std::fs::canonicalize(root).map_err(|error| {
            AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_workspace_invalid".to_string(),
                message: error.to_string(),
            })
        })?;
        if !root.is_dir() {
            return Err(AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_workspace_invalid".to_string(),
                message: "workspace root must be a directory".to_string(),
            }));
        }
        let previous_node = self.typescript_node_approval.clone();
        self.language_startup_authority
            .allow_exact_binary(&node_executable)
            .map_err(|error| {
                AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_node_invalid".to_string(),
                    message: error.to_string(),
                })
            })?;
        self.typescript_node_approval = Some(node_executable.clone());
        let mut replaced_nodes = HashSet::new();
        for server_id in [
            LanguageServerId(102),
            LanguageServerId(106),
            LanguageServerId(107),
            LanguageServerId(108),
        ] {
            if let Some(bundle) = self.typescript_bundles.get(&server_id) {
                replaced_nodes.insert(bundle.node_path.clone());
            }
            if let Some(path) = self.language_server_configured_paths.remove(&server_id) {
                replaced_nodes.insert(path);
            }
            if let Some(config) = self.language_server_local_downloads.remove(&server_id) {
                replaced_nodes.insert(config.node_path);
            }
            if let Some(config) = self.language_server_downloaded.remove(&server_id) {
                replaced_nodes.insert(config.approved_node.canonical_path().to_path_buf());
            }
        }
        let cache_root = root.join(".legion").join("language-tools");
        let descriptor = crate::language::TypeScriptBundleDescriptor::pinned();
        let settings = TypeScriptToolchainSettings {
            server_archive: CanonicalPath(server_archive.to_str().unwrap().to_string()),
            compiler_archive: CanonicalPath(compiler_archive.to_str().unwrap().to_string()),
            node_executable: CanonicalPath(node_executable.to_str().unwrap().to_string()),
        };
        self.language_toolchain_settings.schema_version = 1;
        for server_id in [
            LanguageServerId(102),
            LanguageServerId(106),
            LanguageServerId(107),
            LanguageServerId(108),
        ] {
            self.typescript_bundles.insert(
                server_id,
                TypeScriptBundleStartup {
                    descriptor: descriptor.clone(),
                    server_archive: server_archive.clone(),
                    compiler_archive: compiler_archive.clone(),
                    node_path: node_executable.clone(),
                    cache_root: cache_root.clone(),
                },
            );
        }
        if let Some(previous_node) = previous_node {
            replaced_nodes.insert(previous_node);
        }
        for node in replaced_nodes {
            if node != node_executable
                && !self
                    .language_server_configured_paths
                    .values()
                    .any(|path| path == &node)
                && !self
                    .language_server_local_downloads
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path())
                && !self
                    .language_server_downloaded
                    .values()
                    .any(|config| config.approved_node.canonical_path() == node.as_path())
                && !self
                    .typescript_bundles
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path())
            {
                let _ = self.language_startup_authority.revoke_exact_binary(&node);
            }
        }
        self.language_toolchain_settings.typescript = Some(settings);
        Ok(())
    }

    /// Return the metadata-only local language-toolchain configuration.
    pub fn language_toolchain_settings(&self) -> LanguageToolchainSettingsRecord {
        self.language_toolchain_settings.clone()
    }

    /// Return the non-persistent approval/configuration state for UI
    /// projection. A restored record is always a draft until reconfigured.
    pub fn language_toolchain_configuration_state(&self) -> LanguageToolchainConfigurationState {
        if self.language_toolchain_settings.typescript.is_none() {
            LanguageToolchainConfigurationState::Unconfigured
        } else if self.language_toolchain_settings.schema_version != 1
            || ![102_u64, 106, 107, 108]
                .iter()
                .all(|id| self.typescript_bundles.contains_key(&LanguageServerId(*id)))
        {
            LanguageToolchainConfigurationState::Draft
        } else {
            LanguageToolchainConfigurationState::Configured
        }
    }

    /// Clear the configured TypeScript toolchain without deleting its cache
    /// or changing editor buffers and dirty state.
    pub fn clear_typescript_toolchain(&mut self) {
        let selected_server_is_typescript = self
            .lsp_session
            .selected_server_id()
            .is_some_and(|server_id| matches!(server_id.0, 102 | 106 | 107 | 108));
        if selected_server_is_typescript {
            self.terminalize_pending_lsp_writes(
                None,
                LanguageToolingStatusKind::Cancelled,
                "TypeScript language server configuration was cleared",
            );
            self.lsp_session.reset_to_idle();
        }
        let prior_node = self.typescript_node_approval.take();
        let typescript_server_ids = [
            LanguageServerId(102),
            LanguageServerId(106),
            LanguageServerId(107),
            LanguageServerId(108),
        ];
        let mut removed_nodes = HashSet::new();
        if let Some(node) = prior_node.clone() {
            removed_nodes.insert(node);
        }
        for server_id in typescript_server_ids {
            if let Some(path) = self.language_server_configured_paths.get(&server_id) {
                removed_nodes.insert(path.clone());
            }
            if let Some(config) = self.language_server_local_downloads.get(&server_id) {
                removed_nodes.insert(config.node_path.clone());
            }
            if let Some(config) = self.language_server_downloaded.get(&server_id) {
                removed_nodes.insert(config.approved_node.canonical_path().to_path_buf());
            }
            if let Some(config) = self.typescript_bundles.get(&server_id) {
                removed_nodes.insert(config.node_path.clone());
            }
            self.typescript_bundles.remove(&server_id);
            self.language_server_configured_paths.remove(&server_id);
            self.language_server_local_downloads.remove(&server_id);
            self.language_server_downloaded.remove(&server_id);
        }
        self.language_toolchain_settings.typescript = None;
        for node in removed_nodes {
            let retained_elsewhere = self
                .language_server_configured_paths
                .values()
                .any(|path| path == &node)
                || self
                    .language_server_local_downloads
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path())
                || self
                    .language_server_downloaded
                    .values()
                    .any(|config| config.approved_node.canonical_path() == node.as_path())
                || self
                    .typescript_bundles
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path());
            if !retained_elsewhere {
                let _ = self.language_startup_authority.revoke_exact_binary(&node);
            }
        }
    }
}

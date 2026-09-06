//! Index-backed language-edit proposals used when a live server has not
//! answered yet.
//!
//! Format, rename, organize-imports and code-action gestures still have to
//! produce a Previewed workspace-edit proposal immediately. The LSP write
//! path can replace that preview later; without this fallback the surface
//! records a failure and the proposal ledger stays empty.

use legion_protocol::{
    BufferId, ByteRange, CancellationTokenId, CapabilityId, EditBatch, LanguageToolingProjection,
    LspEditProposalConversionInput, LspRequestCorrelation, PreviewSummary, ProposalAffectedTarget,
    ProposalLifecycleState, ProposalPort, ProposalRequest, ProposalResponse,
    ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalTargetKind,
    ProposalVersionPreconditions, ProtocolDiagnostic, ProtocolDiagnosticSeverity, RedactionHint,
    SemanticPrivacyScope, TextCoordinate, TextEdit, TextRange, TimestampMillis,
    WorkspaceEditProposalPayload, WorkspaceEditSourceKind, WorkspaceTextEdit,
};

use crate::{AppComposition, AppCompositionError, LanguageProposalKind, bounded_label};

fn identifier_byte_range_at(text: &str, requested_byte: u64) -> Option<ByteRange> {
    if text.is_empty() {
        return None;
    }

    let mut index = usize::try_from(requested_byte).unwrap_or(usize::MAX);
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    let bytes = text.as_bytes();
    if index == bytes.len() && index > 0 {
        index -= 1;
    }
    if index < bytes.len()
        && !is_identifier_byte(bytes[index])
        && index > 0
        && is_identifier_byte(bytes[index - 1])
    {
        index -= 1;
    }
    if index >= bytes.len() || !is_identifier_byte(bytes[index]) {
        return None;
    }

    let mut start = index;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = index + 1;
    while end < bytes.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }
    Some(ByteRange::new(start as u64, end as u64))
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

impl AppComposition {
    /// Create a Previewed workspace-edit proposal for a language gesture.
    ///
    /// A live rename request is issued instead of a local identifier rewrite
    /// when the session can carry it. Format / organize-imports / code-action
    /// still return a safe no-op preview so the ledger has a proposal id even
    /// before a language server is ready.
    pub(crate) fn run_language_proposal(
        &mut self,
        buffer_id: BufferId,
        kind: LanguageProposalKind,
        position: TextCoordinate,
        label: String,
    ) -> Result<LanguageToolingProjection, AppCompositionError> {
        let event_context = self.next_event_context();
        let input = self.language_request_input(buffer_id, event_context)?;
        let proposal_id = self.proposal_coordinator.next_id();
        let capability = CapabilityId("fs.write".to_string());
        let preconditions = ProposalVersionPreconditions {
            file_version: Some(input.metadata.file_content_version),
            buffer_version: Some(input.buffer_version),
            snapshot_id: Some(input.snapshot_id),
            generation: Some(input.metadata.workspace_generation),
            file_content_version: Some(input.metadata.file_content_version),
            workspace_generation: Some(input.metadata.workspace_generation),
            expected_fingerprint: Some(input.metadata.fingerprint.clone()),
            expected_file_length: input.metadata.file_length,
            expected_modified_at: input.metadata.modified_at,
        };
        let source = match kind {
            LanguageProposalKind::Formatting => WorkspaceEditSourceKind::LspFormatting,
            LanguageProposalKind::Rename => WorkspaceEditSourceKind::LspRename,
            LanguageProposalKind::OrganizeImports | LanguageProposalKind::CodeAction => {
                WorkspaceEditSourceKind::LspCodeAction
            }
        };
        let title = match kind {
            LanguageProposalKind::Formatting => "Format active buffer".to_string(),
            LanguageProposalKind::Rename => {
                format!("Rename symbol to {}", bounded_label(&label, 64))
            }
            LanguageProposalKind::OrganizeImports => "Organize imports".to_string(),
            LanguageProposalKind::CodeAction => {
                format!("Apply code action {}", bounded_label(&label, 64))
            }
        };
        let (workspace_edit, diagnostics) = match kind {
            LanguageProposalKind::Rename => {
                // When the LSP session is live, route through `textDocument/rename`
                // for multi-file rename coverage (PKT-LSP-C I-2).  The result
                // arrives asynchronously via `ingest_lsp_rename_result` and is
                // projected into `language_tooling` on the next drain call.
                if self.lsp_session.is_live()
                    && self.issue_lsp_rename_request_inner(buffer_id, position, label.clone())
                {
                    return Ok(self.language_tooling.projection());
                }
                // --- local (non-LSP) rename path ---
                let replacement = bounded_label(&label, 128);
                if replacement.trim().is_empty() {
                    return Ok(self.language_tooling.record_proposal_failure(
                        &input,
                        kind,
                        "Rename proposal requires a non-empty replacement label".to_string(),
                    ));
                }
                let Some(range) =
                    identifier_byte_range_at(&input.text, position.byte_offset.unwrap_or(0))
                else {
                    return Ok(self.language_tooling.record_proposal_failure(
                        &input,
                        kind,
                        "Rename proposal requires an identifier at the requested position"
                            .to_string(),
                    ));
                };
                let target = ProposalAffectedTarget {
                    target_id: format!("file:{}", input.metadata.identity.file_id.0),
                    kind: ProposalTargetKind::OpenBuffer,
                    workspace_id: Some(input.workspace_id),
                    file_id: Some(input.metadata.identity.file_id),
                    buffer_id: Some(buffer_id),
                    path: Some(input.metadata.identity.canonical_path.clone()),
                    terminal_session_id: None,
                    plugin_id: None,
                    remote_authority: None,
                    collaboration_session_id: None,
                    byte_ranges: vec![range],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                };
                let workspace_edit = WorkspaceEditProposalPayload {
                    workspace_id: input.workspace_id,
                    edit_id: uuid::Uuid::now_v7(),
                    title: title.clone(),
                    source,
                    target_coverage: ProposalTargetCoverage {
                        coverage_kind: ProposalTargetCoverageKind::Complete,
                        targets: vec![target],
                        omitted_target_count: 0,
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    },
                    file_edits: vec![WorkspaceTextEdit {
                        file: input.metadata.identity.clone(),
                        buffer_id: Some(buffer_id),
                        edits: EditBatch {
                            edits: vec![TextEdit {
                                range: TextRange::byte(range.start, range.end),
                                replacement,
                            }],
                        },
                        preconditions: preconditions.clone(),
                    }],
                    file_operations: Vec::new(),
                    required_capability: capability.clone(),
                    diagnostics: Vec::new(),
                    schema_version: 1,
                };
                (workspace_edit, Vec::new())
            }
            LanguageProposalKind::Formatting
            | LanguageProposalKind::OrganizeImports
            | LanguageProposalKind::CodeAction => {
                let diagnostics = vec![ProtocolDiagnostic {
                    code: "language_tooling.runtime_edit_unavailable".to_string(),
                    message: format!(
                        "{title} is represented as a safe no-op preview until live LSP edits are wired"
                    ),
                    severity: ProtocolDiagnosticSeverity::Warning,
                    path: Some(input.metadata.identity.canonical_path.clone()),
                    range: None,
                }];
                let target = ProposalAffectedTarget {
                    target_id: format!("file:{}", input.metadata.identity.file_id.0),
                    kind: ProposalTargetKind::OpenBuffer,
                    workspace_id: Some(input.workspace_id),
                    file_id: Some(input.metadata.identity.file_id),
                    buffer_id: Some(buffer_id),
                    path: Some(input.metadata.identity.canonical_path.clone()),
                    terminal_session_id: None,
                    plugin_id: None,
                    remote_authority: None,
                    collaboration_session_id: None,
                    byte_ranges: vec![ByteRange::new(0, input.text.len() as u64)],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                };
                let workspace_edit = WorkspaceEditProposalPayload {
                    workspace_id: input.workspace_id,
                    edit_id: uuid::Uuid::now_v7(),
                    title: title.clone(),
                    source,
                    target_coverage: ProposalTargetCoverage {
                        coverage_kind: ProposalTargetCoverageKind::Complete,
                        targets: vec![target],
                        omitted_target_count: 0,
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    },
                    file_edits: vec![WorkspaceTextEdit {
                        file: input.metadata.identity.clone(),
                        buffer_id: Some(buffer_id),
                        edits: EditBatch {
                            edits: vec![TextEdit {
                                range: TextRange::byte(0, input.text.len() as u64),
                                replacement: input.text.clone(),
                            }],
                        },
                        preconditions: preconditions.clone(),
                    }],
                    file_operations: Vec::new(),
                    required_capability: capability.clone(),
                    diagnostics: diagnostics.clone(),
                    schema_version: 1,
                };
                (workspace_edit, diagnostics)
            }
        };
        let request = LspRequestCorrelation {
            request_id: legion_protocol::LspRequestId(uuid::Uuid::now_v7()),
            server_id: legion_protocol::LanguageServerId(1),
            workspace_id: input.workspace_id,
            file_id: Some(input.metadata.identity.file_id),
            snapshot_id: Some(input.snapshot_id),
            buffer_version: Some(input.buffer_version),
            correlation_id: input.event_context.correlation_id,
            causality_id: input.event_context.causality_id,
            cancellation_token: Some(CancellationTokenId(uuid::Uuid::now_v7())),
            privacy_scope: SemanticPrivacyScope::Workspace,
            issued_at: TimestampMillis::now(),
            schema_version: 1,
        };
        let proposal = legion_protocol::convert_lsp_edit_to_workspace_proposal(
            LspEditProposalConversionInput {
                proposal_id,
                principal: input.principal.clone(),
                capability,
                request,
                workspace_edit,
                preconditions,
                lifecycle_state: ProposalLifecycleState::Created,
                privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                preview: PreviewSummary {
                    summary: title.clone(),
                    details: vec![
                        "language_tooling.proposal_preview".to_string(),
                        format!("buffer_version={}", input.buffer_version.0),
                        format!("snapshot_id={}", input.snapshot_id.0),
                    ],
                },
                expires_at: None,
                created_at: TimestampMillis::now(),
                diagnostics,
                schema_version: 1,
            },
        )
        .map_err(|error| AppCompositionError::LanguageTooling(format!("{error:?}")))?;
        self.proposal_coordinator
            .register_lifecycle_context(proposal.proposal_id, input.event_context);
        let created = self.proposal_coordinator.created_response(&proposal);
        if !matches!(created, ProposalResponse::Created(_)) {
            return Ok(self.language_tooling.record_proposal_failure(
                &input,
                kind,
                format!("{title} proposal creation failed: {created:?}"),
            ));
        }
        let validated = self
            .proposal_coordinator
            .handle(ProposalRequest::Validate(proposal.clone()));
        if !matches!(validated, Ok(ProposalResponse::Validated(_))) {
            return Ok(self.language_tooling.record_proposal_failure(
                &input,
                kind,
                format!("{title} proposal validation failed: {validated:?}"),
            ));
        }
        let previewed = self
            .proposal_coordinator
            .handle(ProposalRequest::Preview(proposal.clone()));
        if !matches!(previewed, Ok(ProposalResponse::Previewed { .. })) {
            return Ok(self.language_tooling.record_proposal_failure(
                &input,
                kind,
                format!("{title} proposal preview failed: {previewed:?}"),
            ));
        }
        Ok(self.language_tooling.record_proposal(
            &input,
            kind,
            proposal.proposal_id,
            if matches!(kind, LanguageProposalKind::CodeAction) {
                Some(label.as_str())
            } else {
                None
            },
            format!("{title} proposal preview created"),
            None,
        ))
    }
}

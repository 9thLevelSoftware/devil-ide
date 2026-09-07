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
    ProposalVersionPreconditions, ProtocolDiagnostic, ProtocolDiagnosticSeverity,
    ProtocolTextRange, RedactionHint, SemanticPrivacyScope, TextCoordinate, TextEdit, TextRange,
    TimestampMillis, WorkspaceEditProposalPayload, WorkspaceEditSourceKind, WorkspaceTextEdit,
};

use crate::{AppComposition, AppCompositionError, LanguageProposalKind, bounded_label};

fn identifier_byte_range_at(text: &str, requested_byte: u64) -> Option<ByteRange> {
    if text.is_empty() {
        return None;
    }

    let mut byte = usize::try_from(requested_byte).ok()?.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    if byte == text.len() {
        byte = text.char_indices().next_back()?.0;
    }
    let current = text[byte..].chars().next()?;
    if !is_identifier_char(current) {
        byte = text[..byte].char_indices().next_back()?.0;
        let previous = text[byte..].chars().next()?;
        if !is_identifier_char(previous) {
            return None;
        }
    }

    let mut start = byte;
    for (index, ch) in text[..byte].char_indices().rev() {
        if !is_identifier_char(ch) {
            break;
        }
        start = index;
    }
    let mut end = byte;
    for (index, ch) in text[byte..].char_indices() {
        if !is_identifier_char(ch) {
            break;
        }
        end = byte + index + ch.len_utf8();
    }
    Some(ByteRange::new(start as u64, end as u64))
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn coordinate_byte_offset(text: &str, position: &TextCoordinate) -> Option<u64> {
    if let Some(offset) = position.byte_offset {
        return (offset as usize <= text.len()).then_some(offset);
    }
    let mut remaining_lines = position.line;
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if remaining_lines == 0 {
            let body = line.trim_end_matches(['\n', '\r']);
            let utf16: Vec<u16> = body.encode_utf16().collect();
            let character = usize::try_from(position.character).ok()?;
            if character > utf16.len() {
                return None;
            }
            let prefix = String::from_utf16(&utf16[..character]).ok()?;
            return Some((offset + prefix.len()) as u64);
        }
        remaining_lines = remaining_lines.checked_sub(1)?;
        offset = offset.checked_add(line.len())?;
    }
    // A file that ends with `\n` has a trailing empty line. LSP's
    // `{ line: last+1, character: 0 }` is that line's only valid position.
    if remaining_lines == 0 && position.character == 0 && text.ends_with('\n') {
        return Some(offset as u64);
    }
    None
}

impl AppComposition {
    /// Organize-imports: ask a live server first. A placeholder preview is
    /// only minted when nothing went out, so a multi-action LSP answer can
    /// still require an opaque selection before any proposal exists.
    pub(crate) fn run_organize_imports_proposal(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<LanguageToolingProjection, AppCompositionError> {
        let issued = self
            .whole_document_utf16_range(buffer_id)
            .is_some_and(|range| {
                self.request_code_actions_scoped(
                    buffer_id,
                    ProtocolTextRange {
                        start: TextCoordinate {
                            line: range.start.line,
                            character: range.start.character,
                            byte_offset: None,
                            utf16_offset: None,
                        },
                        end: TextCoordinate {
                            line: range.end.line,
                            character: range.end.character,
                            byte_offset: None,
                            utf16_offset: None,
                        },
                    },
                    true,
                )
            });
        if issued {
            return Ok(self.language_tooling.projection());
        }
        self.run_language_proposal(
            buffer_id,
            LanguageProposalKind::OrganizeImports,
            TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: Some(0),
                utf16_offset: Some(0),
            },
            "organize-imports".to_string(),
        )
    }

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
                // The command arm owns the live `textDocument/rename` issue
                // (including capability gating and deferral). This fallback
                // only builds an index-backed preview so the ledger has an id
                // immediately; it must not send a second rename request.
                let replacement = bounded_label(&label, 128);
                if replacement.trim().is_empty() {
                    return Ok(self.language_tooling.record_proposal_failure(
                        &input,
                        kind,
                        "Rename proposal requires a non-empty replacement label".to_string(),
                    ));
                }
                let Some(offset) = coordinate_byte_offset(&input.text, &position) else {
                    return Ok(self.language_tooling.record_proposal_failure(
                        &input,
                        kind,
                        "rename requires a resolved byte offset".to_string(),
                    ));
                };
                let Some(range) = identifier_byte_range_at(&input.text, offset) else {
                    return Ok(self.language_tooling.record_proposal_failure(
                        &input,
                        kind,
                        if self.lsp_session.is_live() {
                            self.lsp_rename_unavailable_message(buffer_id).to_string()
                        } else {
                            "Rename proposal requires an identifier at the requested position"
                                .to_string()
                        },
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
                    byte_ranges: Vec::new(),
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
                        edits: EditBatch { edits: Vec::new() },
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

#[cfg(test)]
mod tests {
    use super::{coordinate_byte_offset, identifier_byte_range_at};
    use legion_protocol::TextCoordinate;

    #[test]
    fn identifier_range_covers_whole_unicode_symbol() {
        let range = identifier_byte_range_at("rename café now", 8).expect("café");
        assert_eq!(
            &"rename café now"[range.start as usize..range.end as usize],
            "café"
        );
    }

    #[test]
    fn coordinate_without_byte_offset_uses_line_and_character() {
        let text = "fn café()\n";
        let position = TextCoordinate {
            line: 0,
            character: 3,
            byte_offset: None,
            utf16_offset: None,
        };
        let offset = coordinate_byte_offset(text, &position).expect("offset");
        let range = identifier_byte_range_at(text, offset).expect("ident");
        assert_eq!(&text[range.start as usize..range.end as usize], "café");
    }

    #[test]
    fn missing_byte_offset_past_line_fails_closed() {
        let position = TextCoordinate {
            line: 4,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        };
        assert_eq!(coordinate_byte_offset("fn x()\n", &position), None);
    }

    #[test]
    fn trailing_empty_line_resolves_to_eof_byte() {
        let text = "fn x()\n";
        let position = TextCoordinate {
            line: 1,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        };
        assert_eq!(
            coordinate_byte_offset(text, &position),
            Some(text.len() as u64)
        );
        let without_newline = TextCoordinate {
            line: 1,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        };
        assert_eq!(coordinate_byte_offset("fn x()", &without_newline), None);
    }
}

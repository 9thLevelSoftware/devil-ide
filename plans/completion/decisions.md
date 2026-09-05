# S0-01 inventory decisions

## Authority and status

The approved product completion design (`docs/superpowers/specs/2026-09-04-product-completion-design.md`) governs scope and acceptance. The kanban and historical ledgers remain source history and implementation leads. `acceptance` is `unassessed` for every inventory row; kanban `done` values and retained evidence do not promote acceptance.

The inventory contains 197 rows: 165 current kanban task outcome rows from 163 task IDs plus 32 additional source-scope outcomes (22 approved feature families and GAP-01..GAP-10). Legacy task IDs are retained in `legacy_ids`; source references carry path and stable task/scope identity. S0-02 owns population of scenario/configuration IDs, so those arrays are intentionally empty here.

## Conflict resolutions

- **LSP startup wording:** the user guide's implicit Rust-LSP startup wording conflicts with current source tests and the language evidence showing capability-gated startup. The current source behavior and approved completion design govern implementation facts; the ordinary user workflow must be qualified explicitly before acceptance.
- **Fixture defaults:** historical fixture/default claims are treated as test setup evidence only. They do not establish a product default or acceptance result; a native scenario must verify the default.
- **Canvas scope:** existing canvas arrangement evidence is not the full `docs/ui/canvas-workspace-direction.md` promise. The full direction is retained as a separate scope requirement and remains unassessed.
- **Release capability:** historical release/readiness claims do not establish signed, update-safe product capability. GAP-01 through GAP-10 and the release/escrow sources remain required outcomes. The current dry-run/unsigned limits are implementation facts until qualifying evidence exists.
- **VSIX metadata restriction:** the full approved completion specification supersedes the historical metadata-only VSIX restriction. The promised extension surface includes the required webview, notebook, custom-editor and storage capabilities. A future authority amendment is still required before Node activation; metadata parsing or installation alone cannot satisfy that promise.
- **Stale surface-freeze citations:** ADR-0046 freeze citations are retained as historical/deferred-source references. The current project instructions state that the freeze is retired; deferred rows remain requirements only where their own source/evidence requires them.

## Unavailable sources

The backlog documents that `.hermes/plans/2026-06-13_173122-legion-current-to-ga-kanban-plan.md` was removed; it is unavailable and was not inferred. `ENGINEERING_STATUS.md`, `ENGINEERING_AUDIT.yaml`, and `ENGINEERING_PLAN.yaml` are also unavailable after cleanup. Historical `plans/legion-production-master-plan-v0.1.md` and the e2e source package remain supporting leads only; their historical implementation claims were not treated as current truth. The accessible replacement/source trail is the current kanban, completion traceability register, approved completion design, v0.2 master plan, current roadmap/ledgers, installed-product sequence, canvas direction, ADRs, and retained evidence.

## Inventory method

Implementation classifications are conservatively `partial` pending direct current code/evidence review. No classification was copied from kanban status or inferred from filename existence; no row is marked `implemented` or `absent` without a verified trace. This is an inventory signal, not acceptance, and the remaining classification slice is explicitly open for follow-on review.

## S0-01a lossless acceptance extraction (2026-09-04)

This bounded repair used Python 3.12.14 with stdlib `tomllib` to parse `plans/kanban/legion-ga-backlog.toml`. It found 163 task IDs and 165 acceptance strings. Task rows in `requirements.json` now retain their existing fields and stable first IDs, use the exact acceptance string as `title`, and carry an additional `source_refs` entry identifying the exact task and one-based acceptance ordinal (`<task-id> acceptance[<ordinal>]`). The two multi-outcome tasks, `P1.F3.T2` and `P6.F5.T1`, retain their original first requirement ID and receive an additional `-02` row ID. The 32 non-kanban rows were preserved unchanged.

Validation confirmed 197 total rows, 165 task outcome rows, 32 non-kanban rows, exact `(legacy id, ordinal, text)` mapping, unique requirement IDs, zero missing task IDs, and `acceptance = "unassessed"` for all rows. This increment does not mark S0-01 complete.

The other independent S0-01 review items remain explicitly open for separate increments: concrete expansion of family/GAP placeholder rows, implementation classification, stable package definitions/routing, internal-to-product `protected_product_ids`, and the existing internal self-link concern. Duplicating rows did not repair those links; that is outside S0-01a.

## S0-01b package and internal-outcome mapping repair (2026-09-04)

This bounded mapping repair assigns each of the 197 existing rows to a package defined by the Stage 0, Manual/language, AI/team, or production-qualification plan headings. Invented aggregate owners (`S0-BASELINE`, `S1-MANUAL`, `S2-LANGUAGE`, `S3-AI`, `S4-ORCHESTRATION`, and `S5-EXTENSIONS-TEAM`) were replaced with concrete package IDs such as `S1-04`, `S2-02`, `S3-05`, `S4-02`, `S5-09`, and `XQ-01`-family IDs where the qualification plan owns the outcome. Stage values now follow the package's declared implementation stage, including the XQ exceptions (`XQ-02` stage 1, `XQ-03`/`XQ-07`/`XQ-08` stage 0, and `XQ-04`/`XQ-05`/`XQ-06` stage 1).

Routing rationale by legacy family: P0 documentation, governance, and licensing rows route to S0-06; Kanban inventory rows route to S0-01; baseline/build and gate observations route to S0-05. P1 routes by Manual authority, workbench, editing, and evidence outcomes to S1-02/03/04/08. P2 routes language lifecycle/refactoring/debug/test outcomes to S2-02/03/04/05, while terminal, search, and Git outcomes route to S1-06/05/07. P3-P5 route proposal, provider, context, Assist, Delegate, sandbox, and agent-loop outcomes to S3-01/02/04/05/06. P6 routes workflow orchestration to S4-02/03/04/05, with the Canvas outcome owned by S1-03C. P7 routes extension outcomes to S5-01/02/03. P8 routes directly to XQ qualification packages. P9 routes evaluation to S3-07, security audit to XQ-08, collaboration to S5-09/10/11, and training/telemetry to S5-12/13. Family and GAP placeholders receive dominant implementation owners using the same semantic routing; their concrete expansion remains open for a later increment.

Internal rows now reference an existing relevant product requirement through `protected_product_ids`; product rows have empty protected lists. No internal row self-links, no source-extracted title/ID/source-ref fields, and no `acceptance` or `implementation` values were changed. This remains provisional routing metadata and is not implementation or acceptance evidence; no S0-01 completion claim is made.

### S0-01b review-round-1 fixes (2026-09-05)

The review correction keeps `owner_role = "luna_worker"` because the current user instruction explicitly selects Luna execution; this is execution ownership for this bounded inventory repair and does not override the selected package's future implementation/reviewer model.

`P9.F2.T2` now routes to `S3-04`/stage S3 for secret-rule implementation and negative coverage, while `P9.F2.T3` routes to `S5-11`/stage S5 for signed policy bundles, ceilings, retention, and export enforcement. `P9.F2.T1` is an internal security-model requirement owned by `S5-11`; `P9.F2.T4` remains an internal external-audit requirement owned by `XQ-08`. Neither audit nor qualification is treated as construction of the product outcomes.

Internal protection relationships were re-evaluated against the final `kind` assignments. P0.F4.T1/T2 protect documentation-truth GAP-08/GAP-01; P0.F4.T3/T4 protect Manual distribution/package outcomes (FAMILY-22 and GAP-02); P0.F4.T5 protects the GP-1/2/3 product journeys; P0.F4.T6 protects the Rust diagnostics product outcome; P0.F5 protects the recovered SmallCode corpus outcome; P8.F1 protects release/install outcomes (GAP-02 and FAMILY-22); P9.F2 internal rows protect enterprise security policy (FAMILY-19). No internal row points to an internal row or to itself.

`COMP-SCOPE-GAP-06` remains a provisional dominant owner for the Manual/no-egress family only. Its source expansion remains open across S0-05, XQ-01, and later qualification; `XQ-06` alone is not claimed to construct the full artifact or no-egress controls.

### S0-01b review-round-2 fix (2026-09-05)

`P9.F2.T2` is classified as internal because its exact acceptance is fixture/test proof for secret-detection rules. It remains owned by `S3-04` at stage S3 and protects the existing product redaction outcome `COMP-P4-F2-T3-1` (“The bytes sent equal the manifest minus redacted items, with no other delta.”).

## S0-01c legacy implementation classification (2026-09-05)

The current-source implementation inventory is recorded in [implementation-audit.md](implementation-audit.md). It covers all 165 legacy acceptance outcomes exactly once, including separate rows for the two outcome pairs `P1.F3.T2` and `P6.F5.T1`; all `acceptance` values remain `unassessed`. Classifications are implementation facts only: native, hosted, renderer, external-tool, and product acceptance remain separate follow-on evidence.

The audit promotes only behavior established by current source and test bodies. `P1.F4.T5` is `implemented` because the standard performance harness wires the renderer workload, while measurement remains unassessed. `P2.F2.T5` remains `partial` because its ConPTY evidence is metadata parity rather than runtime equivalence. `P2.F4.T4` remains `partial` pending proof of the exact workload semantics and measurement. Missing run evidence alone does not lower an otherwise concrete internal implementation.

The native exploratory lead for `P1.F2.T3` exposed a current-source gap: the desktop keyboard path has no Home/End editor mapping. That row therefore remains `partial`; the observation does not promote or invalidate unrelated native acceptance rows, and the Ctrl+S ingress hypothesis remains separate from the proven command-palette save path.

The independent S0-01c review corrected three classifications against the exact legacy caveats: `P7.F2.T4` is implemented metadata-only VSIX reporting (`NodeSidecar` is classification data, not execution), `P8.F1.T2` is implemented through explicit unsigned-beta/dry-run release descriptors and tests, and `P8.F3.T2` is implemented for the amended v1 consented Rust-panic capture contract. `P8.F1.T3` and `P9.F2.T4` remain absent because the fresh-VM result and archived external audit report are missing.

Round-two source review keeps `P0.F1.T1` and `P0.F1.T2` partial. The canonical mode table and labels exist, but no gate proves every mode-mentioning page links `docs/MODES.md`, and stale labels remain in `mockups/design.md`, `mockups/src/imports/design.md`, and `docs/LEGION_PIVOT.md`; current docs-hygiene checks only exact stale headings.

## S0-01d text-editing scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-01` row is replaced by thirteen atomic rows
`COMP-EDIT-001..013` in package `S1-04`. Each new
row retains the approved `family-01` source identity and remains
`acceptance = unassessed` with empty scenario/configuration coverage. Exact
existing outcomes are mapped in `plans/completion/text-editing-scope-audit.md`
instead of duplicated: multi-cursor is `COMP-P1-F3-T2-1-02`, Vim is
`COMP-P1-F3-T5-1` for generic key-feed safety while named motions, operators,
register, and insert-entry capabilities are explicit `COMP-EDIT-010..013`
companions, and large-file behavior is covered by
`COMP-P1-F4-T2-1` through `COMP-P1-F4-T5-1`.

The approved S1-04 deliverable is the governing text-editing expansion. The
full-vision rectangular editor selection is retained as `COMP-EDIT-009` and
is distinguished from spatial Canvas selection. Historical implementation/status
claims do not promote any new row; the audit records baseline `1895fda2` code
traces and explicit evidence limits. New source paths are literal files with
section locators in identities. This increment does not claim full S0-01 or
full S0 completion.

The large-file mapping preserves `P1.F4.T5` as the committed
`implementation = implemented` classification: the standard harness wiring is
implemented, while renderer measurement and product acceptance remain
unassessed. The audit does not weaken that existing legacy row.

## S0-01e terminal scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-07` row is replaced by fifteen atomic rows `COMP-TERM-001`, `COMP-TERM-002`, and `COMP-TERM-004..016` (with stable TERM-003 removed as an exact duplicate) in package `S1-06`. The approved family and S1-06 terminal deliverable are expanded into explicit shell execution, shell profiles, PTY/input/resize, scrollback/search/rendering, TUI/control keys, OSC cwd/boundaries, task/output references, trust/redaction/env, lifecycle recovery, agent proposal, platform parity, and independent process-evidence outcomes, and multi-session terminal tabs. Exact legacy outcomes P2.F2.T1 through T5 are mapped in `plans/completion/terminal-scope-audit.md` rather than duplicated. The retained P2.F2.T2 row owns the exact trust-denial outcome; all remaining new and retained product rows remain `acceptance = unassessed`; implementation values are source traces only. Source paths are literal files with section locators, and committed baseline `8d151d5` was used for implementation evidence. This increment does not claim full S0-01 or S0 completion.

## S0-01f Workbench scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-02` row is replaced by thirteen atomic rows `COMP-WB-001..013`. Outcomes are routed to their actual packages: S1-03 for tabs/splits/dock/focus/windows/tree/open/reveal/commands, S1-05 for Explorer create/rename/move/delete, S1-04 for settings profiles and keymaps, and S1-08 for native window/focus/DPI/accessibility qualification. Exact broad legacy outcomes remain mapped rather than duplicated (`COMP-P1-F2-T2-1`, `COMP-P1-F2-T3-1`, `COMP-P1-F2-T4-1`, and retained P6.F5 canvas rows). The approved S1-05 create/rename/move/delete/open/reveal promise is current required scope: WB006 explicitly covers both file/folder open/reveal, while WB010-WB013 explicitly cover both file/folder create/rename/move/delete; historical deferrals do not omit it. No inspected source promises multi-root switching, so it is not invented. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01g Canvas scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-03` row is replaced by ten atomic Canvas outcomes `COMP-CAN-001..010` in package `S1-03C`. Existing exact P6.F5 arrangement and person-edge outcomes remain canonical and are mapped rather than duplicated. New rows cover spatial pan/zoom, accessible movement, connection lifecycle, full promised node kinds, groups/scope/minimap, navigation/accessibility, workflow-node execution, provenance-labeled derived edges, and native per-configuration EvidenceRun/recovery/performance evidence. Editor text controls remain in `COMP-EDIT-*`; no editor mutation is claimed for Canvas. Current ADR-0051 restrictions and unavailable `docs/superpowers/specs/2026-09-04-canvas-full-vision-spec.md` are recorded as limits/design gates, not as omissions of approved Canvas scope. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01h Navigation/search scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-04` row is replaced by thirteen atomic outcomes `COMP-NAV-001..013` in package `S1-05`. Exact retained P2.F4 broad outcomes remain canonical for repo-search speed, proposal-only replacement, and large-fixture bounds; Workbench command/CRUD rows and broad P2.F1 language rows remain mapped rather than duplicated. New rows cover quick file/symbol/definition/references navigation, recent buffers and local history, literal/regex search options and ignore/binary rules, cancellation/stale/degraded/error handling, reviewable replacement preview/cancel/apply, stale/conflict/path denial, and the required real-tree native workflow/evidence. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01i Language tooling scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-05` row is replaced by thirteen atomic outcomes `COMP-LANG-001..013` routed to S2-01 through S2-06. The rows preserve the approved Rust, TypeScript/JavaScript, and Python matrix, provisioning/version/hash/prerequisite configuration, real-server LSP projections, proposal-mediated edits, build/test, debug, lifecycle/recovery, and native EvidenceRun promises. Exact retained P2.F1 registry/lifecycle/LSP/call-hierarchy rows and P2.F3 test/debug rows are mapped in `plans/completion/language-tooling-scope-audit.md`, not duplicated. Committed evidence is Rust-focused: TS/JS registry declarations do not prove a desktop live route, and the Python registry URL is an invalid example URL with no materializer; these are recorded limits, never native acceptance claims. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

S0-01i fix1 preserves the explicitly named S2-01 fixture/project identity and fixture hashes/provenance in LANG-001, names organize-imports in LANG-007, and names targeted/grouped build-test execution and output in LANG-011; all remain acceptance-unassessed.

## S0-01j Build/test/debug scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-06` row is replaced by nine atomic outcomes `COMP-BTD-001..009` routed to S2-04, S2-05, and S2-06. The rows preserve approved four-language configuration/prerequisite, build/task/test execution, targeted/group result output, failure/stop/rerun/recovery, real DAP launch/attach/inspection/terminate, and native EvidenceRun promises. Exact retained P2.F3 rows and broad LANG-011/LANG-012 outcomes are mapped in `plans/completion/build-test-debug-scope-audit.md` rather than duplicated. Protocol, fixture, mock, and projection traces remain implementation evidence only; all acceptance remains `unassessed`. This increment does not claim S0 completion.

S0-01j fix1 removes redundant `COMP-BTD-010`: the broad four-language real-runner/adapter promise is already owned by `COMP-LANG-011` and `COMP-LANG-012`. Stable `COMP-BTD-001..009` remain for narrower configuration, execution/result, recovery, inspection, and native-journey outcomes.

## S0-01k Source control scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-08` row is replaced by ten atomic outcomes `COMP-SCM-001..010` in S1-07. The rows preserve status/diff, file and partial-hunk staging, commit, branch safety, conflict resolution, policy-visible remote verbs, local history/checkpoint restoration, worktree management, dirty/external/restart/storage recovery, and host-observed native Git effects. Exact retained P2.F5 outcomes are mapped in `plans/completion/source-control-scope-audit.md`, not duplicated. Read/projection contracts are not treated as actual Git operation proof; no remote/auth mutation was performed. All acceptance remains `unassessed`; this increment does not claim S0 completion.

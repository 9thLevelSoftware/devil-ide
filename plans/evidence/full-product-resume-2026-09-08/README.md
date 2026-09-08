# Full product resume evidence — 2026-09-08

## What this directory is

This directory holds one raw log: the workspace test suite run for round r00 of
the resumed full-product completion work.

**Classification: workspace suite result. Not product acceptance.**

A green workspace suite says the repository's own tests compile and pass on this
host. It does not assess any acceptance row, does not qualify a packaged
product, and does not establish native GUI, cross-OS, release, dependency, or
phase readiness. No `acceptance` value in `plans/completion/requirements.json`
may be changed on the strength of this run. The 59-package /
419-requirement scope remains unchanged and unassessed as a complete product
matrix.

## The run

- Packet: `round-r00-full-suite`
- Kind: `cargo-full-suite`
- Command: `cargo test --workspace --all-targets -j 1 --no-fail-fast`
- Worktree: `D:/legion-ide-completion`, branch `codex/full-product-resume`
- Environment: `LEGION_TEST_NODE_RUNTIME` exported
- Log: [round-r00-full-suite.log](round-r00-full-suite.log) (6983 lines, ends `EXIT=0` at line 6983)
- Source of the copy: `.superpowers/sdd/2026-09-04-full-product-completion/round-r00-full-suite-full-suite.log`
  (local gitignored ledger); copied byte-identical,
  SHA-256 `06a09d89d2ef7db434eb64ff2120f5fe9306621adc265a85bdf73d2e7e09d3ed`.

## Outcome exactly as logged

Build finished (``Finished `test` profile [unoptimized + debuginfo] target(s) in
3m 30s``, log line 37) with zero compiler errors.

356 test binaries were launched and 356 `test result:` lines were emitted; every
started target reported a result and none was cut short. Aggregate across all
356 targets:

| | passed | failed | ignored | measured | filtered out |
|---|---|---|---|---|---|
| 49 unittest binaries | 2140 | 0 | 2 | 0 | 0 |
| 307 integration-test binaries | 2521 | 0 | 35 | 0 | 0 |
| **total** | **4661** | **0** | **37** | **0** | **0** |

Every `test result:` line is `ok.`; there are zero non-ok result lines. The log
contains 0 lines matching `^error[` or `^error:`, 0 lines containing `FAILED`, 0
`failures:` sections, and 0 `panicked at` occurrences. The list of failing test
names is empty. Doc-tests were not run, which is expected under `--all-targets`.

All 30 workspace members named in `Cargo.toml` produced a unittest binary
(`legion_app`, `legion_desktop`, `xtask`, `legion_ui`, `legion_editor`,
`legion_text`, `legion_project`, `legion_index`, `legion_lsp`, `legion_ai`,
`legion_ai_providers`, `legion_agent`, `legion_tracker`, `legion_memory`,
`legion_security`, `legion_platform`, `legion_protocol`, `legion_storage`,
`legion_observability`, `legion_plugin`, `legion_vscode_compat`,
`legion_collaboration`, `legion_remote`, `legion_remote_transport`,
`legion_terminal`, `legion_debug`, `legion_telemetry`, `legion_retention`,
`legion_sandbox`, `legion_cli`). `legion_app`, `legion_desktop` and `xtask` each
contributed two unittest binaries (lib + bin). The remaining unittest binaries
are test fixtures and examples.

A second ``Finished `test` profile ... in 0.18s`` at log line 1235 follows
`Compiling bench-test-fixture v0.1.0` in a temp directory. That is a
test-spawned cargo build of a temp fixture crate from inside the suite
(`legion_bench_live`), not a second workspace build. Its interleaved output is
the only ordering anomaly in the Running-to-result alternation (lines 1236 and
1245); the counts still pair 1:1 and the parent target reports 13 passed at line
1240.

Warnings: 22 warning lines. Only 2 are compiler warnings, both from the excluded
vendored crate — ``warning: falling back to `f32` as the trait bound `f32:
From<f64>` is not satisfied`` at `vendor/epaint/src/tessellator.rs:2326:34`
(lint `float_literal_f32_fallback`, future-incompatible, rust-lang issue
#154024) and its `epaint (lib) generated 1 warning` roll-up. The other 20 are
`LF will be replaced by CRLF` notices from git subprocesses that tests spawn
against temp fixtures. No warning came from any `legion-*` crate.

Sum of per-target reported runtimes is 114.5s; wall clock was dominated by the
3m30s compile plus serialized `-j 1` execution.

## What this run does NOT cover — 37 ignored tests

37 tests were reported ignored (37 occurrences, 37 unique names). **These tests
did not run.** `#[ignore]` is a compile-time attribute, so exporting
`LEGION_TEST_NODE_RUNTIME` opted none of them in under a plain `cargo test`
invocation — several print an opt-in Node/Pyright/TypeScript-fixture reason
string and were skipped all the same. Running them requires a separate
invocation with `-- --ignored` plus the named fixtures and servers, which was
not part of this step.

Live-model hostile eval (needs a local model server; `cargo run -p xtask --
hostile-eval-live`):

1. `a_live_model_cannot_exfiltrate_a_secret`
2. `a_live_model_reading_injected_text_still_cannot_act_on_it`
3. `tests::anthropic_messages_client_live_smoke_round_trip`

Native LSP / language runtime (opt-in Node, retained Pyright and TypeScript
fixtures, rust-analyzer):

4. `explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text`
5. `explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text`
6. `native_node_runtime_probe_uses_selected_executable_and_revalidates`
7. `native_python_diagnostic_clears_after_editor_replace_and_save`
8. `native_python_rename_external_overwrite_rejects_without_partial_mutation`
9. `native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit`
10. `native_typescript_formatting_is_reviewable_before_disk_mutation`
11. `native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval`
12. `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports`
13. `native_typescript_rename_conflict_does_not_partially_apply`
14. `native_typescript_rename_is_reviewable_and_applies_only_after_approval`
15. `retained_pyright_fixture_materializes_with_catalog_identity`
16. `rust_analyzer_full_workflow`
17. `rust_analyzer_initializes_against_legion_repo_when_opted_in`
18. `rust_analyzer_initializes_and_emits_diagnostics`
19. `rust_analyzer_product_composition_smoke`
20. `two_ra_same_workspace_didchange_stress`

100MB / scale budgets:

21. `large_file_100mb_degraded_mode_measurement` (report-only)
22. `scale_100mb_buffer_creation_under_budget`
23. `scale_100mb_memory_ceiling`
24. `scale_100mb_single_keystroke_edit_under_budget`
25. `scale_100mb_snapshot_creation_and_chunk_iteration`
26. `scale_100mb_streaming_from_reader`
27. `scale_100mb_viewport_slice_under_budget`

Perf / timing diagnostics and other opt-ins:

28. `completion_request_cost_against_position_depth` (does not assert)
29. `indexed_workspace_search_benchmark_large_fixture`
30. `keystroke_cost_amortized_versus_compaction`
31. `keystroke_cost_by_position_in_the_file`
32. `keystroke_cost_tracks_line_count_not_byte_count`
33. `snapshot_retention_and_release`
34. `training::tests::regenerate_training_candidate_fixtures`
35. `undo_redo_latency_under_edit_burst`
36. `viewport_projection_cost_against_scroll_depth`
37. `workspace_open_1000_files_completes_within_budget`

Any acceptance row that depends on native LSP behaviour (rust-analyzer, Pyright,
TypeScript), on the live-model hostile eval, on the 100MB scale budgets, or on
the keystroke/viewport perf budgets has **no evidence from this run** and must
not be assessed from it.

## Unverifiable from the log alone

The log has no invocation header; it begins directly with compiler output. The
branch name `codex/full-product-resume` and the `LEGION_TEST_NODE_RUNTIME`
export are therefore recorded from the invoking context, not readable from the
log bytes. Neither affects the pass/fail outcome above.

## Standing position

The full-product goal remains unfinished, not blocked and not complete. Native
GUI automation is resumed on this Windows host by owner instruction; macOS and
Linux hosts remain unavailable and their rows stay blocked with their recorded
prerequisite strings.

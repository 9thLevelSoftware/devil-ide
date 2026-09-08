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

---

# Round r01 — packet round evidence (appended 2026-09-08)

## What round r01 was

One bounded packet round on branch `codex/full-product-resume`. One packet
passed review: `s2-03b-language-extract`, a pure-move extraction of the language
toolchain settings and the proposal-kind dispatch out of
`crates/legion-app/src/lib.rs` into `crates/legion-app/src/language/`. Two
packets were rejected at review and reverted before any test ran
(`s2-03a-bounded-stdin`, `s0-03e-register-verify`); no packet failed its tests.

**Classification for every log below: component or integrated evidence.
Not packaged evidence, not native GUI evidence, not product acceptance.**

No `acceptance` value in `plans/completion/requirements.json` was changed on the
strength of any of these runs, and none may be. These are crate-level compile
checks, crate-level unit and integration tests, a static lint pass, and
repository hygiene gates. They exercise no packaged artifact, no real window, no
real language-server process, and no cross-OS host.

## Commands

The raw logs carry no invocation header — each begins directly with tool output
and ends with a trailing `EXIT=<code>` line written by the runner. The command
column below is the command the round driver
(`.superpowers/sdd/2026-09-04-full-product-completion/legion-completion-round.js`,
steps Check and Test) issues for that step, and it is consistent with the tool
output present in each log. Every cargo command in this round took `-j 1` under
the single-cargo-lane rule.

## Logs

| log | command | exit | classification |
|---|---|---|---|
| `round-r01-check0-legion-platform.log` | `cargo check -p legion-platform --all-targets -j 1` | 0 | component evidence — compile check only, no test executed |
| `round-r01-check0-legion-app.log` | `cargo check -p legion-app --all-targets -j 1` | 0 | component evidence — compile check only, no test executed |
| `round-r01-check0-xtask.log` | `cargo check -p xtask --all-targets -j 1` | 0 | component evidence — compile check only, no test executed |
| `round-r01-test-step1.log` | `git checkout --` of the rejected packets' tracked paths (no cargo) | 0 | process record, not product evidence — records the revert of the two rejected packets |
| `round-r01-test-step2a.log` | `cargo test -p legion-app --test typescript_toolchain_settings -j 1 --no-fail-fast` | 0 | integrated evidence, crate level — in-crate integration target, no real language server, no packaged product |
| `round-r01-test-step2b.log` | `cargo test -p legion-app --lib -j 1 --no-fail-fast language::formatting_dispatch_tests` | 0 | component evidence — in-crate unit tests |
| `round-r01-test-step2c.log` | `cargo test -p legion-app --lib -j 1 --no-fail-fast language::toolchain_settings::toolchain_approval_tests` | 0 | component evidence — in-crate unit tests |
| `round-r01-test-step4a.log` | `cargo test -p legion-app --lib -j 1` | 0 | component evidence — crate unit suite, regression guard |
| `round-r01-test-step4b.log` | `cargo test -p legion-desktop --lib -j 1` | 0 | component evidence — crate unit suite, regression guard |
| `round-r01-test-step5.log` | `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | 0 | component evidence — static lint, no test executed |
| `round-r01-test-step6a.log` | `cargo fmt --all --check` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6b.log` | `cargo run -p xtask -- check-deps` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6c.log` | `cargo run -p xtask -- docs-hygiene` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6d.log` | `cargo run -p xtask -- claim-audit` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6e.log` | `cargo run -p xtask -- extract-before-modify` | 0 | component evidence — chokepoint growth gate |

Step 3 of the driver (the `--ignored` real-server startup suites) produced no log
because no packet in this round was marked `real_server`; the passed packet is a
pure move. That absence is not a skipped pass — the real-server rows simply have
no evidence from round r01.

## Copy integrity

Each log was copied byte-identical from the gitignored ledger
`.superpowers/sdd/2026-09-04-full-product-completion/` into this directory and
re-hashed after the copy; every pair matched.

| log | lines | SHA-256 |
|---|---|---|
| `round-r01-check0-legion-platform.log` | 4 | `2c546ca14139c3828f449558cd0c747e81cba2db751fa19780726be463b66fb3` |
| `round-r01-check0-legion-app.log` | 25 | `60e6339b68d365ad8a95fb0a6f346b482f7f20f845a6fb96db241fae11ba1a46` |
| `round-r01-check0-xtask.log` | 10 | `007f9de0f447ab3e323138f1aff8de69b79fb3864847f313eb08e739e1b366d8` |
| `round-r01-test-step1.log` | 25 | `4b0c8cdfe2567d25c642d813b0a58ec6f930e2126df5b882eb71c5e2c1f328bd` |
| `round-r01-test-step2a.log` | 18 | `ea15b1708a508c82af9d4febd0dacda27be2249c01be4de73c136618767ab55d` |
| `round-r01-test-step2b.log` | 12 | `e0e1a55589c3f2f370f7c82c3356d8db0e727c503e8f3450548e3492ba3170d2` |
| `round-r01-test-step2c.log` | 11 | `372db2a3864684d4fa3de24b82b23d07a199c69c6b8c3a3159a8edf341954bce` |
| `round-r01-test-step4a.log` | 447 | `755d52241e6765c9cbcb8810058246c83238e0b74beaa5ece4e106922cf7b8ea` |
| `round-r01-test-step4b.log` | 270 | `768700fde9254d8c8e5a3cfb679610ba70046c65f89cac5aa7c78b32dbbdff91` |
| `round-r01-test-step5.log` | 22 | `b344d85a276c4d281245ccea62a83dc58ee476279bc5dd6391b48492646b8446` |
| `round-r01-test-step6a.log` | 1 | `418a5c17f33c70e99b0cc0a07fce69191489cfedc94164bfa903785777c5bd4b` |
| `round-r01-test-step6b.log` | 7 | `718f6d2e2a47a0de3efd52d3acee85a51746f0d18e3bd1e183c538e7648ff88f` |
| `round-r01-test-step6c.log` | 4 | `31a4c7b8571b88142024770317c27cec193fa0cb884af5c9dacd78909066505b` |
| `round-r01-test-step6d.log` | 4 | `61a4c8e60570d2bb6d4c97cc28156db6791917256960e01635a7d985b500b0a3` |
| `round-r01-test-step6e.log` | 4 | `e56ade19083b2c5e960a68ff8dd507c8befa2e633b08647a8da3795dce4f6767` |

The hashes above are of the LF bytes as the runner wrote them, which is
also what git stores: this repository sets `core.autocrlf=true` and
`.gitattributes` carries no rule for `plans/evidence/**`, so a Windows checkout
expands these logs to CRLF and re-hashing them there will not reproduce these
values. Compare against the stored blob (`git show <rev>:<path> | sha256sum`),
not against a Windows working-tree copy.

## Outcomes exactly as logged

- `check0` (three logs): all three crates checked with `--all-targets`; the `dev`
  profile finished in 17.73s, 52.56s and 29.57s respectively; zero compiler
  errors; `EXIT=0`.
- Step 1: the two rejected packets' tracked files were restored with
  `git checkout --` (`checkout rc=0`), and the one untracked file the rejected
  bounded-stdin packet would have added
  (`crates/legion-platform/src/bounded_stdin.rs`) was recorded `ABSENT (never
  created; nothing to delete)`. The pre-revert diffstat it discarded was 5 files,
  1021 insertions, 38 deletions. Post-revert status for those paths is empty; the
  only remaining dirty paths are the four owned by the passed packet.
- Step 2a: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0
  filtered out; finished in 0.02s`.
- Step 2b: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 436
  filtered out; finished in 0.01s`.
- Step 2c: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 436
  filtered out; finished in 0.01s`.
- Step 4a regression guard: `test result: ok. 439 passed; 0 failed; 0 ignored; 0
  measured; 0 filtered out; finished in 0.65s`. The driver expects 439 or more;
  439 observed, no regression.
- Step 4b regression guard: `test result: ok. 243 passed; 0 failed; 0 ignored; 0
  measured; 0 filtered out; finished in 11.09s`. The driver expects 243 or more;
  243 observed, no regression.
- Step 5: clippy finished in 1m 15s with `EXIT=0`. The only warning in the log is
  the vendored-crate `float_literal_f32_fallback` future-incompatibility at
  `vendor/epaint/src/tessellator.rs:2326:34` and its `epaint (lib) generated 1
  warning` roll-up; `epaint` is excluded from the workspace lint failure. No
  `legion-*` crate warned.
- Step 6a: no output at all apart from the trailing `EXIT=0` — the formatting
  gate found nothing to report.
- Step 6b: `dependency policy checks passed`.
- Step 6c: `documentation hygiene checks passed`.
- Step 6d: `claim audit passed`.
- Step 6e: `extract-before-modify: no chokepoint file grew past its slack`.

Every one of the fifteen logs ends `EXIT=0`. No log contains a `FAILED` marker, a
`failures:` section, a `panicked at` line, or a non-`ok.` `test result:` line.

## Register effect of round r01: none

The reviewer-proposed implementation transition for the passed packet was
`COMP-LANG-007` to `partial`. That row was already `partial`, so applying the
transition changed nothing: `plans/completion/requirements.json` still holds 419
rows, the implementation tally is unchanged at 143 implemented / 233 partial / 43
absent, and all 419 `acceptance` values remain `unassessed`. The file is
byte-identical to its state before the round. A pure move is expected to be
register-neutral; it relocates code without adding product behaviour.

## Standing position

The full-product goal remains unfinished, not blocked and not complete. Native
GUI automation is resumed on this Windows host by owner instruction; macOS and
Linux hosts remain unavailable and their rows stay blocked with the exact
prerequisite strings recorded in `plans/completion/decisions.md` and in the local
`blockers.json`.

## Round r02 — bounded stdin, register validator, canonical registers

Three packets passed independent review with no blocking findings. This is
component evidence. It does not qualify packaged completion, native GUI
behaviour, or any of the 419 requirements; every row remains `unassessed`.

Packet checks:

- `round-r02-test-packet-tests-s2-03a.log` and
  `round-r02-test-packet-tests-s2-03a-rerun.log` record the first bounded-stdin
  attempt, in which one test failed (`test result: FAILED. 0 passed; 1 failed`,
  `EXIT=101`). They are retained because a failed attempt is part of the record.
- `round-r02-retest-packet-retests-s2-03a-test.log` records the repaired run:
  `cargo test -p legion-platform --test bounded_process -j 1 --no-fail-fast`,
  18 passed, 0 failed, `EXIT=0`.
- `round-r02-test-packet-tests-s0-03e.log` records the register-validator tests,
  33 passed and 18 passed, 0 failed, `EXIT=0`.

Round gates, re-run by the coordinator with full command provenance after the
first attempt was refuted for producing logs that did not show a command had
run:

| Log | Result |
| --- | --- |
| `round-r02-gates-fmt.log` | `cargo fmt --all --check`, EXIT=0 |
| `round-r02-gates-check-deps.log` | dependency policy, EXIT=0 |
| `round-r02-gates-docs-hygiene.log` | documentation hygiene, EXIT=0 |
| `round-r02-gates-claim-audit.log` | claim audit, EXIT=0 |
| `round-r02-gates-extract-before-modify.log` | no chokepoint growth, EXIT=0 |
| `round-r02-gates-regress-app.log` | `legion-app` lib, 439 passed, 0 failed |
| `round-r02-gates-regress-desktop.log` | `legion-desktop` lib, 243 passed, 0 failed |
| `round-r02-gates-clippy.log` | workspace all-targets `-D warnings`, EXIT=0 |
| `round-r02-gates-register-verify.log` | `verify-completion-register`, **EXIT=1**, 1579 structural issues |

The register validator exiting 1 is the truthful current state, not a
regression. It now runs for the first time, and it reports that the 419
requirement rows still carry empty `scenario_ids` and `configuration_ids`, so
required rows have no scenario or configuration coverage. Binding those rows is
the next register task. No row was accepted, and no exemption was added to make
the validator pass.

The first gate attempt of this round was refuted and superseded. Its
narration-only logs are retained in the local ledger and were deliberately not
copied here, because they are not evidence of execution.

## Round r03 — register binding, Python toolchain settings, formatter approval

Three packets passed independent review. Component evidence only; every one of
the 419 rows remains `acceptance: unassessed`.

| Log | Result |
| --- | --- |
| `round-r03-gates-fmt.log` | `cargo fmt --all --check`, EXIT=0 |
| `round-r03-gates-check-deps.log` | dependency policy, EXIT=0 |
| `round-r03-gates-docs-hygiene.log` | documentation hygiene, EXIT=0 |
| `round-r03-gates-claim-audit.log` | claim audit, EXIT=0 |
| `round-r03-gates-extract-before-modify.log` | no chokepoint growth, EXIT=0 |
| `round-r03-gates-regress-app.log` | `legion-app` lib, 446 passed, 0 failed |
| `round-r03-gates-regress-desktop.log` | `legion-desktop` lib, 243 passed, 0 failed |
| `round-r03-gates-clippy.log` | workspace all-targets `-D warnings`, EXIT=0 |
| `round-r03-gates-register-verify.log` | `verify-completion-register`, **EXIT=1**, 144 structural issues |

The register validator improves from 1579 issues to 144 and still exits 1,
which is the honest state. The remainder is 77 rows the binding generator
deliberately left unbound, each named with a reason code in its packet report,
plus 67 pre-existing register defects that the first real validator run has now
surfaced: product rows carrying `protected_product_ids`, `source_refs` pointing
at directories rather than files, a product requirement owned by S6, and two
non-bidirectional links. No row was accepted and no exemption was added.

The round's first formatting gate failed with 13 mechanical rustfmt hunks in
two files of the formatter-approval packet. `round-r03-fix-fmt.log` records the
`cargo fmt --all` that repaired it, and every gate above is the re-run after
that fix. The failed gate attempt is part of the record.

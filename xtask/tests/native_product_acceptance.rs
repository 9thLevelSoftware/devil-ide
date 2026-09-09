//! ADR-0056 / `COMP-PLAT-002`: the native input acceptance harness must fail
//! *loudly and distinguishably* on a host that cannot answer the question.
//!
//! Every test here runs headless: no display session, no input driver, no
//! packaged binary. Driver discovery and child-process launching are injected
//! boundaries, so these assertions test the code and not the host.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use xtask::native_product_acceptance::{
    self as npa, AcceptanceReport, DriverAvailability, EXIT_BLOCKED, EXIT_CONFORMANCE_FAILED,
    EXIT_OPERATIONAL_ERROR, EXIT_PASSED, InputDriverProbe, NativeProductAcceptanceOptions,
    REPORT_FILE_NAME, SubprocessLauncher,
};

/// A driver-discovery fixture. The real probe touches the filesystem; this one
/// answers whatever the test needs, so no test depends on the host.
struct FixtureProbe {
    availability: DriverAvailability,
}

impl InputDriverProbe for FixtureProbe {
    fn probe(&self) -> DriverAvailability {
        self.availability.clone()
    }
}

/// A launcher that records every child process it is asked to start and starts
/// none of them.
#[derive(Default)]
struct RecordingLauncher {
    launches: Mutex<Vec<String>>,
}

impl RecordingLauncher {
    fn launch_count(&self) -> usize {
        self.launches.lock().expect("launch log").len()
    }
}

impl SubprocessLauncher for RecordingLauncher {
    fn launch(&self, program: &Path, args: &[String], _working_dir: &Path) -> io::Result<i32> {
        self.launches.lock().expect("launch log").push(format!(
            "{} {}",
            program.display(),
            args.join(" ")
        ));
        Ok(0)
    }
}

fn temp_workspace(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "legion-native-product-acceptance-{tag}-{}-{nanos}",
        process::id()
    ));
    fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("create {}: {err}", dir.display()));
    dir
}

fn options() -> NativeProductAcceptanceOptions {
    NativeProductAcceptanceOptions {
        out_dir: "out".to_string(),
        package_dir: "package".to_string(),
        driver_path: Some("tools/native-input-driver/fixture-driver".to_string()),
    }
}

fn unavailable_probe() -> FixtureProbe {
    FixtureProbe {
        availability: DriverAvailability::Unavailable {
            prerequisite: npa::PREREQUISITE_DRIVER_MISSING.to_string(),
        },
    }
}

fn available_probe() -> FixtureProbe {
    FixtureProbe {
        availability: DriverAvailability::Available {
            driver_path: "tools/native-input-driver/fixture-driver".to_string(),
        },
    }
}

/// Create an empty file at `path`, parents included. Used to stand in for a
/// built or staged driver binary without building one.
fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("create {}: {err}", parent.display()));
    }
    fs::write(path, b"").unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

fn report_text(workspace: &Path) -> String {
    let path = workspace.join("out").join(REPORT_FILE_NAME);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// Value of a `key = "value"` line, unescaped only for the escapes this
/// harness emits.
fn toml_string_field(text: &str, key: &str) -> String {
    let prefix = format!("{key} = \"");
    let line = text
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("report has no `{key}` field:\n{text}"));
    let body = &line[prefix.len()..];
    let body = body
        .strip_suffix('"')
        .unwrap_or_else(|| panic!("unterminated `{key}` field: {line}"));
    body.replace("\\\\", "\\").replace("\\\"", "\"")
}

#[test]
fn driver_discovery_prefers_the_built_in_repo_driver_over_the_staged_path() {
    // The driver is a workspace crate now, so discovery looks first where cargo
    // puts it. The staged `tools/` path is retained, last, so a binary an owner
    // put there by hand still works — but a freshly built one must win over a
    // stale staged one, or a rebuild would silently not be what ran.
    let candidates = npa::default_driver_candidates();
    assert_eq!(
        candidates.len(),
        3,
        "discovery probes release, then debug, then the retained staged path: {candidates:?}"
    );
    assert!(
        candidates[0].starts_with("target/release/"),
        "the release build is probed first: {candidates:?}"
    );
    assert!(
        candidates[1].starts_with("target/debug/"),
        "the debug build is probed second: {candidates:?}"
    );
    assert!(
        candidates[2].starts_with("tools/native-input-driver/"),
        "the owner-staged path is retained, last: {candidates:?}"
    );
    for candidate in &candidates {
        assert!(
            candidate.ends_with(npa::driver_executable_name()),
            "every candidate names this host's driver binary: {candidate}"
        );
    }

    let workspace = temp_workspace("discovery");
    let resolved = candidates
        .iter()
        .map(|candidate| workspace.join(candidate))
        .collect::<Vec<_>>();

    assert_eq!(
        npa::first_existing_driver(&resolved),
        None,
        "with nothing built and nothing staged, discovery must find nothing rather than \
         invent a path"
    );

    touch(&resolved[2]);
    assert_eq!(
        npa::first_existing_driver(&resolved).as_deref(),
        Some(resolved[2].as_path()),
        "a staged binary is still discovered when nothing has been built"
    );

    touch(&resolved[1]);
    assert_eq!(
        npa::first_existing_driver(&resolved).as_deref(),
        Some(resolved[1].as_path()),
        "a debug build outranks the staged path"
    );

    touch(&resolved[0]);
    assert_eq!(
        npa::first_existing_driver(&resolved).as_deref(),
        Some(resolved[0].as_path()),
        "the release build outranks both"
    );

    // The host probe answers from that same ordered list, and never builds
    // anything to make an answer come out.
    match npa::HostInputDriverProbe::new(resolved.clone()).probe() {
        DriverAvailability::Available { driver_path } => {
            assert_eq!(driver_path, resolved[0].display().to_string());
        }
        DriverAvailability::Unavailable { prerequisite } => {
            // A driver file exists at `resolved[0]`, so the only build that may
            // still answer unavailable is one ADR-0056 does not enable at all.
            assert_ne!(
                std::env::consts::OS,
                "windows",
                "on Windows a driver that exists on disk must be discovered; got {prerequisite:?}"
            );
            assert_eq!(
                prerequisite,
                npa::PREREQUISITE_UNSUPPORTED_HOST,
                "macOS and Linux stay blocked on BLK-2026-09-08-02 whatever is on disk"
            );
        }
    }

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn driver_missing_prerequisite_names_the_in_repo_build_command() {
    let prerequisite = npa::PREREQUISITE_DRIVER_MISSING;
    assert!(
        prerequisite.contains("cargo build -p legion-input-driver --release"),
        "the driver is built from this repository, so the prerequisite must name the build \
         command, not an installation the owner has to source: {prerequisite:?}"
    );
    assert!(
        prerequisite.contains("target/release/legion-input-driver.exe"),
        "the prerequisite must say where that build lands: {prerequisite:?}"
    );

    let build_at = prerequisite
        .find("cargo build -p legion-input-driver --release")
        .expect("build command present");
    let staged_at = prerequisite
        .find("tools/native-input-driver/")
        .expect("the retained staged path stays documented, because discovery still probes it");
    assert!(
        build_at < staged_at,
        "the in-repo build is the action to take; the staged path is the retained fallback \
         mentioned after it: {prerequisite:?}"
    );

    // And the blocked report carries that exact string, not a paraphrase.
    let workspace = temp_workspace("build-command");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "prerequisite"), prerequisite);
    assert_eq!(
        launcher.launch_count(),
        0,
        "naming a build command must not make the harness run one"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn unavailable_driver_returns_blocked_with_a_nonzero_exit() {
    let workspace = temp_workspace("unavailable");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);

    assert_ne!(
        code, 0,
        "a host with no input driver must never exit 0; that is how a blocked run gets read \
         as a pass"
    );
    assert_eq!(
        code, EXIT_BLOCKED,
        "an unavailable driver is blocked, not a conformance failure"
    );
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "blocked");
    assert!(
        text.contains("driver_available = false"),
        "report must record that discovery found no driver:\n{text}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn blocked_result_carries_an_exact_actionable_prerequisite_string() {
    let workspace = temp_workspace("prerequisite");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    let text = report_text(&workspace);
    let prerequisite = toml_string_field(&text, "prerequisite");

    assert!(
        prerequisite.len() > 80,
        "a prerequisite must be a sentence naming what the owner supplies, not a label; got \
         {prerequisite:?}"
    );
    let lowered = prerequisite.to_lowercase();
    for needle in ["host", "session", "driver", "install"] {
        assert!(
            lowered.contains(needle),
            "prerequisite must name the {needle} the owner has to supply; got {prerequisite:?}"
        );
    }
    assert!(
        lowered.contains("keyboard") && lowered.contains("clipboard") && lowered.contains("ime"),
        "prerequisite must say what the driver has to be able to do; got {prerequisite:?}"
    );
    assert_ne!(
        lowered.trim(),
        "driver unavailable",
        "a prerequisite that cannot be acted on is a dead end"
    );
    assert_eq!(
        prerequisite,
        npa::PREREQUISITE_DRIVER_MISSING,
        "the report must carry the published prerequisite verbatim"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn blocked_exit_code_is_distinct_from_the_conformance_failure_exit_code() {
    // Two concrete integers. If a later change ever makes them equal, CI can no
    // longer tell "the product is wrong" from "this machine cannot answer the
    // question", and this test fails.
    assert_eq!(EXIT_BLOCKED, 3);
    assert_eq!(EXIT_CONFORMANCE_FAILED, 1);
    assert_ne!(
        EXIT_BLOCKED, EXIT_CONFORMANCE_FAILED,
        "blocked and conformance-failed must never collapse into one nonzero code"
    );

    assert_eq!(EXIT_PASSED, 0);
    assert_eq!(EXIT_OPERATIONAL_ERROR, 2);
    let codes = [
        EXIT_PASSED,
        EXIT_CONFORMANCE_FAILED,
        EXIT_OPERATIONAL_ERROR,
        EXIT_BLOCKED,
    ];
    for (index, first) in codes.iter().enumerate() {
        for second in codes.iter().skip(index + 1) {
            assert_ne!(first, second, "exit codes must be pairwise distinct");
        }
    }
    assert!(
        !codes.iter().skip(1).any(|code| *code == 0),
        "no non-pass outcome may exit 0"
    );
}

#[test]
fn blocked_report_records_neither_passed_nor_skipped() {
    let workspace = temp_workspace("status");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    let text = report_text(&workspace);
    let status = toml_string_field(&text, "status");
    assert_eq!(status, "blocked");
    assert!(
        !status.contains("passed"),
        "a blocked run must not report a pass; status was {status:?}"
    );
    assert!(
        !status.contains("skipped"),
        "a blocked run must not report a skip; status was {status:?}"
    );
    assert!(
        !text.contains("status = \"passed\"") && !text.contains("status = \"skipped\""),
        "no status line in a blocked report may read passed or skipped:\n{text}"
    );
    assert!(
        text.contains("window_created = false") && text.contains("input_classes_observed = []"),
        "a blocked run records no positive evidence:\n{text}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn no_subprocess_is_spawned_when_the_driver_is_unavailable() {
    let workspace = temp_workspace("no-spawn");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    assert_eq!(
        launcher.launch_count(),
        0,
        "an unavailable driver must start no child process; spawning a windowed binary on a \
         headless machine hangs CI. Recorded: {:?}",
        launcher.launches.lock().expect("launch log")
    );
    let text = report_text(&workspace);
    assert!(
        text.contains("subprocess_launched = false"),
        "the report must state that nothing was launched:\n{text}"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn missing_packaged_binary_is_reported_blocked_rather_than_failed() {
    let workspace = temp_workspace("no-package");
    // Driver present, package absent: the host cannot answer the question and
    // the product is not implicated.
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &available_probe(), &launcher);

    assert_eq!(
        code, EXIT_BLOCKED,
        "an absent packaged product is a missing prerequisite, not a product defect"
    );
    assert_ne!(
        code, EXIT_CONFORMANCE_FAILED,
        "blaming the product for a missing package would file a false defect"
    );
    let text = report_text(&workspace);
    assert_eq!(toml_string_field(&text, "status"), "blocked");
    assert!(
        text.contains("driver_available = true")
            && text.contains("packaged_product_present = false"),
        "the report must distinguish which prerequisite was missing:\n{text}"
    );
    let prerequisite = toml_string_field(&text, "prerequisite").to_lowercase();
    assert!(
        prerequisite.contains("packaged") && prerequisite.contains("product"),
        "the prerequisite must name the packaged product; got {prerequisite:?}"
    );
    assert_eq!(
        launcher.launch_count(),
        0,
        "nothing may be launched when there is no packaged product to launch"
    );

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn report_is_written_and_readable_even_when_the_run_is_blocked() {
    let workspace = temp_workspace("report");
    let launcher = RecordingLauncher::default();
    let code =
        npa::run_native_product_acceptance(&workspace, &options(), &unavailable_probe(), &launcher);
    assert_eq!(code, EXIT_BLOCKED);

    let path = workspace.join("out").join(REPORT_FILE_NAME);
    assert!(
        path.is_file(),
        "a blocked run that leaves no artifact is unauditable: {}",
        path.display()
    );
    let text = fs::read_to_string(&path).expect("blocked report must be readable");
    assert!(!text.trim().is_empty(), "blocked report must not be empty");
    assert!(
        text.contains("requirement_id = \"COMP-PLAT-002\""),
        "the report must name the requirement it is about:\n{text}"
    );
    assert!(
        text.contains("exit_code = 3"),
        "the report must record the exit code it returned:\n{text}"
    );

    // Rendering is pure, so the same report text is reproducible.
    let rendered = npa::render_report(&AcceptanceReport {
        status: "blocked".to_string(),
        exit_code: EXIT_BLOCKED,
        prerequisite: Some(npa::PREREQUISITE_DRIVER_MISSING.to_string()),
        driver_available: false,
        interactive_session: false,
        packaged_product_present: false,
        subprocess_launched: false,
        window_created: false,
        input_classes_observed: Vec::new(),
        driver_path: "tools/native-input-driver/fixture-driver".to_string(),
        package_dir: "package".to_string(),
        notes: Vec::new(),
    });
    assert!(rendered.contains("status = \"blocked\""));

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn command_is_not_referenced_by_any_pr_gate_workflow() {
    let workflows = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows");
    let entries = fs::read_dir(&workflows)
        .unwrap_or_else(|err| panic!("read {}: {err}", workflows.display()));

    let mut inspected = 0;
    for entry in entries {
        let entry = entry.expect("workflow dir entry");
        let path = entry.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "yml" || ext == "yaml");
        if !is_yaml {
            continue;
        }
        inspected += 1;
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        assert!(
            !text.contains("native-product-acceptance"),
            "{} references native-product-acceptance; this harness has never produced a real \
             result and must not gate merges. Promoting it is a deliberate change that has to \
             edit this test.",
            path.display()
        );
    }
    assert!(
        inspected > 0,
        "expected workflow files under {}",
        workflows.display()
    );
}

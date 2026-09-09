//! `COMP-PLAT-002`: native input acceptance harness for the **packaged**
//! product (ADR-0056).
//!
//! `xtask` may not depend on `legion-desktop`. The product is reached only as a
//! subprocess, and every OS-level observation is made by an external input
//! driver process, never by an in-process assertion: an assertion inside the
//! product cannot tell a working native input path from a test harness talking
//! to itself.
//!
//! # Exit codes
//!
//! A CI job must be able to tell "the product is wrong" from "this machine
//! cannot answer the question". These four codes are the contract; they are
//! deliberately distinct and must stay distinct.
//!
//! | Code | Meaning |
//! | ---- | ------- |
//! | [`EXIT_PASSED`] (`0`) | Every one of the six input classes was observed to conform, from outside the product process, on a real window. Exit `0` requires those positive facts to be recorded; it is never the mere absence of a failure. |
//! | [`EXIT_CONFORMANCE_FAILED`] (`1`) | The harness reached the product and the product's native input behaviour deviated. **The product is wrong.** |
//! | [`EXIT_OPERATIONAL_ERROR`] (`2`) | The harness itself could not operate: the output directory could not be created, the report could not be written, the driver was launched but produced no result. |
//! | [`EXIT_BLOCKED`] (`3`) | This host cannot answer the question: no input driver, no interactive desktop session, no packaged product, or an OS whose rows are blocked on `BLK-2026-09-08-02`. **Never a pass, never a skip, never `0`.** |
//!
//! # Not a gate
//!
//! Like `windowed-gui-e2e`, this command is not a standing gate and is not
//! merge-blocking. It is referenced by no workflow under `.github/workflows/`,
//! and `xtask/tests/native_product_acceptance.rs` asserts exactly that, so
//! promoting it to a gate has to be a deliberate act.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Every input class was observed to conform from outside the product process.
pub const EXIT_PASSED: i32 = 0;
/// The harness reached the product and the product's behaviour deviated.
pub const EXIT_CONFORMANCE_FAILED: i32 = 1;
/// The harness could not operate (output directory, report, or driver result).
pub const EXIT_OPERATIONAL_ERROR: i32 = 2;
/// This host cannot answer the question. Never a pass and never a skip.
pub const EXIT_BLOCKED: i32 = 3;

/// File name of the report this command always writes, including when the run
/// is blocked. A blocked run that leaves no readable artifact is unauditable.
pub const REPORT_FILE_NAME: &str = "native_product_acceptance.toml";
/// File the driver writes during the interactive-session handshake.
pub const SESSION_HANDSHAKE_FILE_NAME: &str = "driver_session.toml";
/// File the driver writes with its per-class observations.
pub const DRIVER_RESULT_FILE_NAME: &str = "driver_result.toml";

/// The six input classes named by `COMP-PLAT-002`.
pub const INPUT_CLASSES: [&str; 6] = [
    "keyboard",
    "pointer",
    "text",
    "clipboard",
    "ime-cjk",
    "command",
];

/// Status string written into the report's `status` field.
pub const STATUS_PASSED: &str = "passed";
/// Status string for an observed product deviation.
pub const STATUS_CONFORMANCE_FAILED: &str = "conformance-failed";
/// Status string for a harness malfunction.
pub const STATUS_OPERATIONAL_ERROR: &str = "operational-error";
/// Status string for a host that cannot answer the question.
pub const STATUS_BLOCKED: &str = "blocked";

/// Exact, actionable prerequisite for a Windows host on which no native input
/// driver has been built. Shaped like the prerequisite strings in
/// `plans/completion/decisions.md`: it names what the owner must supply.
///
/// The driver is a workspace crate (`crates/legion-input-driver`), so the
/// action is a build, not an install. The staged path stays in the sentence
/// because [`default_driver_candidates`] still probes it last, and an
/// owner-staged binary therefore still works.
pub const PREREQUISITE_DRIVER_MISSING: &str = concat!(
    "A Windows 11 x64 host with an interactive logged-in desktop session and ",
    "the in-repo native input driver built with ",
    "`cargo build -p legion-input-driver --release`, present at ",
    "`target/release/legion-input-driver.exe` (a debug build at ",
    "`target/debug/legion-input-driver.exe`, or a binary installed at ",
    "`tools/native-input-driver/legion-input-driver.exe`, is also accepted), ",
    "able to inject OS-level keyboard, pointer, text, clipboard and IME/CJK ",
    "input into another process and to read that process's UI Automation text ",
    "and clipboard state from outside it."
);

/// Exact, actionable prerequisite when the driver exists but the host has no
/// interactive desktop session (a headless Windows CI runner reaches this).
pub const PREREQUISITE_SESSION_MISSING: &str = concat!(
    "A Windows 11 x64 host running this command inside an interactive ",
    "logged-in desktop session (an attended session or an autologon runner ",
    "with a real window station), not a service or headless session, so the ",
    "installed native input driver can attach to the input desktop."
);

/// Exact, actionable prerequisite when no packaged product is present.
pub const PREREQUISITE_PACKAGE_MISSING: &str = concat!(
    "A Windows 11 x64 host with the packaged native Legion product installed ",
    "into the package directory this command was given, containing the ",
    "product executable, so the harness can launch the packaged product as a ",
    "subprocess rather than a development build."
);

/// Exact, actionable prerequisite for macOS and Linux. These rows stay blocked
/// on `BLK-2026-09-08-02` and a Windows result never stands in for them.
pub const PREREQUISITE_UNSUPPORTED_HOST: &str = concat!(
    "A macOS host and a Linux host with the packaged native product installed ",
    "and a real display session, able to run the windowed GUI e2e suite ",
    "(BLK-2026-09-08-02). A Windows result never substitutes for a macOS or ",
    "Linux row."
);

/// Options for `xtask native-product-acceptance`.
#[derive(Debug, Clone)]
pub struct NativeProductAcceptanceOptions {
    /// Output directory, relative to the workspace root, for the report.
    pub out_dir: String,
    /// Directory holding the packaged product, relative to the workspace root.
    pub package_dir: String,
    /// Path to the input driver, relative to the workspace root. `None` probes
    /// [`default_driver_candidates`] in order.
    pub driver_path: Option<String>,
}

impl Default for NativeProductAcceptanceOptions {
    fn default() -> Self {
        Self {
            out_dir: "target/native-input-acceptance".to_string(),
            package_dir: "target/native-input-acceptance/package".to_string(),
            driver_path: None,
        }
    }
}

impl NativeProductAcceptanceOptions {
    /// The driver paths this run will probe, in order.
    pub fn driver_candidates(&self) -> Vec<String> {
        match &self.driver_path {
            Some(path) => vec![path.clone()],
            None => default_driver_candidates(),
        }
    }

    /// The driver path this run names in its report before discovery has run.
    /// Discovery reports the candidate it actually found.
    pub fn resolved_driver_path(&self) -> String {
        self.driver_candidates()
            .first()
            .cloned()
            .unwrap_or_else(|| default_driver_candidates()[0].clone())
    }
}

/// File name of the driver binary cargo produces for this host.
pub fn driver_executable_name() -> &'static str {
    if cfg!(windows) {
        "legion-input-driver.exe"
    } else {
        "legion-input-driver"
    }
}

/// Where the harness looks for the native input driver, in order, first
/// existing file wins.
///
/// The driver is a workspace crate, so the first two entries are simply where
/// cargo puts it. The third is retained so a binary an owner staged by hand
/// still works. Discovery probes the **filesystem** and nothing else: `xtask`
/// must never build the driver mid-run, because a harness that builds its own
/// instrument cannot report a clean blocked result.
pub fn default_driver_candidates() -> Vec<String> {
    let name = driver_executable_name();
    vec![
        format!("target/release/{name}"),
        format!("target/debug/{name}"),
        format!("tools/native-input-driver/{name}"),
    ]
}

/// The first candidate that exists as a file on disk.
///
/// Pure and injectable: `candidates` are already resolved against a root, so
/// this is testable without a host driver and without a display session.
pub fn first_existing_driver(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
}

/// Name of the packaged product executable inside the package directory.
pub fn packaged_executable_name() -> &'static str {
    if cfg!(windows) {
        "legion-desktop.exe"
    } else {
        "legion-desktop"
    }
}

/// What the driver-discovery boundary reports.
///
/// `Unavailable` always carries an exact prerequisite string; a blocked result
/// whose reason cannot be acted on is a dead end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverAvailability {
    /// A usable driver was found on disk at `driver_path`.
    Available {
        /// Absolute or workspace-relative path the driver was found at.
        driver_path: String,
    },
    /// No usable driver. `prerequisite` names what the owner must supply.
    Unavailable {
        /// Exact, actionable prerequisite string.
        prerequisite: String,
    },
}

/// Driver discovery, as an injectable boundary.
///
/// Tests point this at a fixture so every test in this module runs headless,
/// with no display session, no driver and no packaged binary present.
pub trait InputDriverProbe {
    /// Probe this host for a usable external native input driver.
    fn probe(&self) -> DriverAvailability;
}

/// Real host discovery.
///
/// Availability is **probed**, not inferred. macOS and Linux are refused by
/// decision (`BLK-2026-09-08-02`), and on Windows a driver must actually exist
/// as a file on disk at one of the ordered candidates — a `cfg!(windows)`
/// build or an environment variable that happens to be set is not evidence
/// that a driver is present, and neither is the driver crate being a workspace
/// member. This probe reads the filesystem and does nothing else: it cannot
/// build the driver, and it starts no process, so an unavailable driver can
/// never spawn a child.
///
/// The interactive-session question is answered separately, by the driver
/// itself, in [`run_native_product_acceptance`].
#[derive(Debug, Clone)]
pub struct HostInputDriverProbe {
    candidates: Vec<PathBuf>,
}

impl HostInputDriverProbe {
    /// Probe `candidates` in order (each already resolved against the
    /// workspace root); the first existing file wins.
    pub fn new(candidates: Vec<PathBuf>) -> Self {
        Self { candidates }
    }
}

impl InputDriverProbe for HostInputDriverProbe {
    fn probe(&self) -> DriverAvailability {
        if !cfg!(windows) {
            return DriverAvailability::Unavailable {
                prerequisite: PREREQUISITE_UNSUPPORTED_HOST.to_string(),
            };
        }
        match first_existing_driver(&self.candidates) {
            Some(driver_path) => DriverAvailability::Available {
                driver_path: driver_path.display().to_string(),
            },
            None => DriverAvailability::Unavailable {
                prerequisite: PREREQUISITE_DRIVER_MISSING.to_string(),
            },
        }
    }
}

/// Child-process launching, as an injectable boundary.
///
/// Tests substitute a recorder and assert that an unavailable driver launches
/// nothing. That is what stops a future change from spawning a windowed binary
/// on a headless machine and hanging CI.
pub trait SubprocessLauncher {
    /// Run `program` with `args` from `working_dir`; return its exit code.
    fn launch(&self, program: &Path, args: &[String], working_dir: &Path) -> io::Result<i32>;
}

/// Real child-process launching.
#[derive(Debug, Clone, Default)]
pub struct HostSubprocessLauncher;

impl SubprocessLauncher for HostSubprocessLauncher {
    fn launch(&self, program: &Path, args: &[String], working_dir: &Path) -> io::Result<i32> {
        let status = Command::new(program)
            .args(args)
            .current_dir(working_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        Ok(status.code().unwrap_or(1))
    }
}

/// The report this command always writes.
#[derive(Debug, Clone)]
pub struct AcceptanceReport {
    /// One of [`STATUS_PASSED`], [`STATUS_CONFORMANCE_FAILED`],
    /// [`STATUS_OPERATIONAL_ERROR`], [`STATUS_BLOCKED`].
    pub status: String,
    /// The exit code this run returns.
    pub exit_code: i32,
    /// Exact prerequisite string; present exactly when `status` is blocked.
    pub prerequisite: Option<String>,
    /// Whether driver discovery found a driver.
    pub driver_available: bool,
    /// Whether the driver reported an interactive desktop session.
    pub interactive_session: bool,
    /// Whether the packaged product executable was present.
    pub packaged_product_present: bool,
    /// Whether any child process was started by this run.
    pub subprocess_launched: bool,
    /// Whether a native window was observed from outside the product process.
    pub window_created: bool,
    /// Input classes observed to conform, from outside the product process.
    pub input_classes_observed: Vec<String>,
    /// The driver path this run used.
    pub driver_path: String,
    /// The package directory this run used.
    pub package_dir: String,
    /// Free-form notes; never a substitute for the fields above.
    pub notes: Vec<String>,
}

impl AcceptanceReport {
    fn new(driver_path: String, package_dir: String) -> Self {
        Self {
            status: STATUS_BLOCKED.to_string(),
            exit_code: EXIT_BLOCKED,
            prerequisite: None,
            driver_available: false,
            interactive_session: false,
            packaged_product_present: false,
            subprocess_launched: false,
            window_created: false,
            input_classes_observed: Vec::new(),
            driver_path,
            package_dir,
            notes: Vec::new(),
        }
    }

    fn blocked(&mut self, prerequisite: &str) {
        self.status = STATUS_BLOCKED.to_string();
        self.exit_code = EXIT_BLOCKED;
        self.prerequisite = Some(prerequisite.to_string());
    }
}

/// Render the report as TOML. The `status` field is rendered verbatim so a
/// blocked run can never read as a pass or a skip after a reword.
pub fn render_report(report: &AcceptanceReport) -> String {
    let mut text = String::new();
    text.push_str("# xtask native-product-acceptance (ADR-0056)\n");
    text.push_str("# COMP-PLAT-002 native input acceptance harness.\n");
    text.push_str("schema_version = 1\n");
    text.push_str("command = \"native-product-acceptance\"\n");
    text.push_str("requirement_id = \"COMP-PLAT-002\"\n");
    text.push_str(&format!("status = \"{}\"\n", escape_toml(&report.status)));
    text.push_str(&format!("exit_code = {}\n", report.exit_code));
    text.push_str(&format!("host_os = \"{}\"\n", host_os_label()));
    text.push_str(&format!(
        "driver_available = {}\n",
        bool_literal(report.driver_available)
    ));
    text.push_str(&format!(
        "interactive_session = {}\n",
        bool_literal(report.interactive_session)
    ));
    text.push_str(&format!(
        "packaged_product_present = {}\n",
        bool_literal(report.packaged_product_present)
    ));
    text.push_str(&format!(
        "subprocess_launched = {}\n",
        bool_literal(report.subprocess_launched)
    ));
    text.push_str(&format!(
        "window_created = {}\n",
        bool_literal(report.window_created)
    ));
    text.push_str(&format!(
        "driver_path = \"{}\"\n",
        escape_toml(&report.driver_path)
    ));
    text.push_str(&format!(
        "package_dir = \"{}\"\n",
        escape_toml(&report.package_dir)
    ));
    text.push_str(&format!(
        "required_input_classes = [{}]\n",
        INPUT_CLASSES
            .iter()
            .map(|class| format!("\"{}\"", escape_toml(class)))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    text.push_str(&format!(
        "input_classes_observed = [{}]\n",
        report
            .input_classes_observed
            .iter()
            .map(|class| format!("\"{}\"", escape_toml(class)))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    match &report.prerequisite {
        Some(prerequisite) => text.push_str(&format!(
            "prerequisite = \"{}\"\n",
            escape_toml(prerequisite)
        )),
        None => text.push_str("prerequisite = \"\"\n"),
    }
    text.push_str(&format!(
        "notes = [{}]\n",
        report
            .notes
            .iter()
            .map(|note| format!("\"{}\"", escape_toml(note)))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    text
}

fn bool_literal(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn host_os_label() -> &'static str {
    std::env::consts::OS
}

fn escape_toml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Run the harness with the host's real boundaries.
pub fn run_native_product_acceptance_command(opts: &NativeProductAcceptanceOptions) -> i32 {
    let workspace_root = match std::env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!(
                "native-product-acceptance: unable to resolve current directory: {err} \
                 (operational error, exit {EXIT_OPERATIONAL_ERROR})"
            );
            return EXIT_OPERATIONAL_ERROR;
        }
    };
    let candidates = opts
        .driver_candidates()
        .iter()
        .map(|candidate| workspace_root.join(candidate))
        .collect::<Vec<_>>();
    let probe = HostInputDriverProbe::new(candidates);
    let launcher = HostSubprocessLauncher;
    run_native_product_acceptance(&workspace_root, opts, &probe, &launcher)
}

/// Run the harness against injected boundaries.
///
/// Return points, exhaustively: an unwritable output directory or report is
/// [`EXIT_OPERATIONAL_ERROR`]; an unavailable driver, an absent packaged
/// product, an OS blocked on `BLK-2026-09-08-02` and a host with no
/// interactive desktop session are [`EXIT_BLOCKED`]; a driver that ran but
/// produced no readable result is [`EXIT_OPERATIONAL_ERROR`]; an observed
/// product deviation is [`EXIT_CONFORMANCE_FAILED`]. [`EXIT_PASSED`] is
/// returned only when the driver exited zero **and** the result records a
/// created window **and** every one of the six [`INPUT_CLASSES`] was observed
/// to conform.
pub fn run_native_product_acceptance(
    workspace_root: &Path,
    opts: &NativeProductAcceptanceOptions,
    probe: &dyn InputDriverProbe,
    launcher: &dyn SubprocessLauncher,
) -> i32 {
    let out_dir = workspace_root.join(&opts.out_dir);
    let package_dir = workspace_root.join(&opts.package_dir);
    let driver_path = opts.resolved_driver_path();
    let report_path = out_dir.join(REPORT_FILE_NAME);

    if let Err(err) = fs::create_dir_all(&out_dir) {
        eprintln!(
            "native-product-acceptance: cannot create output directory {}: {err} \
             (operational error, exit {EXIT_OPERATIONAL_ERROR})",
            out_dir.display()
        );
        return EXIT_OPERATIONAL_ERROR;
    }

    let mut report = AcceptanceReport::new(driver_path, opts.package_dir.clone());

    match probe.probe() {
        DriverAvailability::Unavailable { prerequisite } => {
            report.blocked(&prerequisite);
            report.notes.push(
                "driver discovery found no usable external native input driver; no child \
                 process was started"
                    .to_string(),
            );
            return finish(&report_path, &report);
        }
        DriverAvailability::Available { driver_path: found } => {
            report.driver_available = true;
            report.driver_path = found;
        }
    }

    let packaged = package_dir.join(packaged_executable_name());
    if !packaged.is_file() {
        report.blocked(PREREQUISITE_PACKAGE_MISSING);
        report.notes.push(format!(
            "no packaged product executable at {}; this host cannot answer the question and \
             the product is not implicated",
            packaged.display()
        ));
        return finish(&report_path, &report);
    }
    report.packaged_product_present = true;

    // Only now, with a driver on disk and a packaged product present, may a
    // child process start. The interactive-session question is answered by the
    // driver on the real host, never by an environment variable read here.
    // `Path::join` with an already-absolute discovery result yields that
    // absolute path, so this works for both a workspace-relative fixture and
    // the host probe's resolved path.
    let driver_program = workspace_root.join(&report.driver_path);
    let session_path = out_dir.join(SESSION_HANDSHAKE_FILE_NAME);
    let _ = fs::remove_file(&session_path);
    let session_args = vec![
        "--probe-session".to_string(),
        "--report".to_string(),
        session_path.display().to_string(),
    ];
    report.subprocess_launched = true;
    match launcher.launch(&driver_program, &session_args, workspace_root) {
        Ok(0) => {}
        Ok(code) => {
            report.blocked(PREREQUISITE_SESSION_MISSING);
            report.notes.push(format!(
                "driver session handshake exited {code}; the host did not present an \
                 interactive desktop session"
            ));
            return finish(&report_path, &report);
        }
        Err(err) => {
            report.blocked(PREREQUISITE_SESSION_MISSING);
            report
                .notes
                .push(format!("driver session handshake could not start: {err}"));
            return finish(&report_path, &report);
        }
    }
    let session_text = fs::read_to_string(&session_path).unwrap_or_default();
    if !session_text.contains("interactive_session = true") {
        report.blocked(PREREQUISITE_SESSION_MISSING);
        report.notes.push(
            "driver session handshake did not record `interactive_session = true`".to_string(),
        );
        return finish(&report_path, &report);
    }
    report.interactive_session = true;

    let result_path = out_dir.join(DRIVER_RESULT_FILE_NAME);
    let _ = fs::remove_file(&result_path);
    let run_args = vec![
        "--conformance-run".to_string(),
        "--product".to_string(),
        packaged.display().to_string(),
        "--report".to_string(),
        result_path.display().to_string(),
    ];
    let driver_code = match launcher.launch(&driver_program, &run_args, workspace_root) {
        Ok(code) => code,
        Err(err) => {
            report.status = STATUS_OPERATIONAL_ERROR.to_string();
            report.exit_code = EXIT_OPERATIONAL_ERROR;
            report
                .notes
                .push(format!("driver conformance run could not start: {err}"));
            return finish(&report_path, &report);
        }
    };

    let result_text = match fs::read_to_string(&result_path) {
        Ok(text) => text,
        Err(err) => {
            report.status = STATUS_OPERATIONAL_ERROR.to_string();
            report.exit_code = EXIT_OPERATIONAL_ERROR;
            report.notes.push(format!(
                "driver exited {driver_code} but wrote no readable result at {}: {err}",
                result_path.display()
            ));
            return finish(&report_path, &report);
        }
    };

    report.window_created = result_text.contains("window_created = true");
    report.input_classes_observed = INPUT_CLASSES
        .iter()
        .copied()
        .filter(|class| result_text.contains(&format!("input_class_{class} = \"conforms\"")))
        .map(|class| class.to_string())
        .collect();

    let every_class_observed = report.input_classes_observed.len() == INPUT_CLASSES.len();
    if driver_code == 0 && report.window_created && every_class_observed {
        report.status = STATUS_PASSED.to_string();
        report.exit_code = EXIT_PASSED;
    } else {
        report.status = STATUS_CONFORMANCE_FAILED.to_string();
        report.exit_code = EXIT_CONFORMANCE_FAILED;
        report.notes.push(format!(
            "driver exited {driver_code}; window_created={}, {} of {} input classes observed",
            report.window_created,
            report.input_classes_observed.len(),
            INPUT_CLASSES.len()
        ));
    }
    finish(&report_path, &report)
}

/// Write the report and return its exit code. A report that cannot be written
/// downgrades the run to an operational error rather than reporting a result
/// nobody can read.
fn finish(report_path: &Path, report: &AcceptanceReport) -> i32 {
    let text = render_report(report);
    if let Err(err) = fs::write(report_path, &text) {
        eprintln!(
            "native-product-acceptance: cannot write report {}: {err} \
             (operational error, exit {EXIT_OPERATIONAL_ERROR})",
            report_path.display()
        );
        return EXIT_OPERATIONAL_ERROR;
    }
    if report.exit_code == EXIT_BLOCKED {
        eprintln!(
            "native-product-acceptance: blocked (exit {EXIT_BLOCKED}); this host cannot answer \
             the question. Prerequisite: {}",
            report.prerequisite.as_deref().unwrap_or("")
        );
    } else {
        eprintln!(
            "native-product-acceptance: {} (exit {}); report written to {}",
            report.status,
            report.exit_code,
            report_path.display()
        );
    }
    report.exit_code
}

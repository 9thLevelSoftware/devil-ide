//! ADR-0056 / `COMP-PLAT-002`: the contract `legion-input-driver` owes the
//! harness.
//!
//! Every test here runs headless: no display session, no packaged product and
//! no IME. The driver's host-independent halves — argument parsing, report
//! rendering, the outcome vocabulary and the on-disk digest — are included by
//! path so they can be asserted without a lib target and without a window.
//!
//! The one test that runs the real binary points it at a product path that
//! does not exist, so it starts nothing.

use std::{
    fs,
    path::PathBuf,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

// The driver is a binary crate. Including its host-independent modules by path
// is what lets these assertions run against the same source the binary uses,
// rather than against a copy that could drift away from it.
#[path = "../src/cli.rs"]
#[allow(dead_code)]
mod cli;
#[path = "../src/observe.rs"]
#[allow(dead_code)]
mod observe;
#[path = "../src/report.rs"]
#[allow(dead_code)]
mod report;
#[path = "../src/session.rs"]
#[allow(dead_code)]
mod session;

use report::{ClassObservation, ClassOutcome, ConformanceReport};
use session::DesktopAttachment;

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "legion-input-driver-{tag}-{}-{nanos}",
        process::id()
    ));
    fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("create {}: {err}", dir.display()));
    dir
}

fn not_attached() -> DesktopAttachment {
    DesktopAttachment::NotAttached {
        detail: "OpenInputDesktop failed: the operation completed unsuccessfully".to_string(),
    }
}

fn attached() -> DesktopAttachment {
    DesktopAttachment::Attached {
        desktop: "Default".to_string(),
    }
}

fn conforming_report() -> ConformanceReport {
    ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: true,
        observations: report::INPUT_CLASSES
            .iter()
            .copied()
            .map(|class| ClassObservation::new(class, ClassOutcome::Conforms, "observed"))
            .collect(),
        prerequisite: None,
        notes: Vec::new(),
    }
}

#[test]
fn probe_session_writes_a_report_even_when_no_interactive_desktop_is_available() {
    let dir = temp_dir("probe-report");
    let path = dir.join("driver_session.toml");
    let text = report::render_session_report(&not_attached());
    fs::write(&path, text.as_bytes()).expect("write session report");

    let written = fs::read_to_string(&path).expect("a blocked handshake must leave an artifact");
    assert!(
        !written.trim().is_empty(),
        "a blocked session report that is empty is unauditable"
    );
    assert!(
        written.contains("status = \"blocked\""),
        "the handshake must say it was blocked:\n{written}"
    );
    assert!(
        written.contains("mode = \"probe-session\""),
        "the report must name the mode that produced it:\n{written}"
    );
    let prerequisite_line = written
        .lines()
        .find(|line| line.starts_with("prerequisite = \""))
        .unwrap_or_else(|| panic!("no prerequisite line:\n{written}"));
    assert!(
        prerequisite_line.len() > 100,
        "a prerequisite must be a sentence the owner can act on, not a label: {prerequisite_line}"
    );
    let lowered = prerequisite_line.to_lowercase();
    for needle in ["windows", "session", "desktop"] {
        assert!(
            lowered.contains(needle),
            "the prerequisite must name the {needle} the owner has to supply: {prerequisite_line}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn probe_session_exits_nonzero_when_it_cannot_attach_to_the_input_desktop() {
    let blocked = report::session_exit_code(&not_attached());
    assert_ne!(
        blocked, 0,
        "a driver that could not attach must never exit 0; that is how a blocked host gets read \
         as a pass"
    );
    assert_eq!(
        blocked, 3,
        "failing to attach is blocked (3), not a conformance failure"
    );

    let unsupported = report::session_exit_code(&DesktopAttachment::UnsupportedHost {
        detail: "linux".to_string(),
    });
    assert_eq!(unsupported, 3, "an unsupported host is blocked, not a pass");

    assert_eq!(
        report::session_exit_code(&attached()),
        0,
        "attaching to the input desktop is the only outcome that exits 0"
    );
}

#[test]
fn probe_session_never_writes_interactive_session_true_without_attaching() {
    // A driver that pasted an OS string straight into its report could forge
    // the harness's success marker inside a blocked report. Try exactly that.
    let adversarial = DesktopAttachment::NotAttached {
        detail: "OpenInputDesktop failed; interactive_session = true was not established"
            .to_string(),
    };

    for attachment in [not_attached(), adversarial] {
        let text = report::render_session_report(&attachment);
        assert!(
            !text.contains("interactive_session = true"),
            "a report written without attaching must never carry the harness's success marker \
             anywhere in it:\n{text}"
        );
        assert!(
            text.contains("interactive_session = false"),
            "a blocked handshake must record the negative explicitly:\n{text}"
        );
    }

    let attached_text = report::render_session_report(&attached());
    assert!(
        attached_text.contains("interactive_session = true"),
        "attaching really must write the marker, or the harness can never pass:\n{attached_text}"
    );
}

#[test]
fn conformance_run_without_a_product_executable_exits_nonzero_and_records_no_class() {
    let dir = temp_dir("no-product");
    let report_path = dir.join("driver_result.toml");
    let absent_product = dir.join("there-is-no-packaged-product-here.exe");
    assert!(
        !absent_product.exists(),
        "this test depends on the product path being absent"
    );

    let status = process::Command::new(env!("CARGO_BIN_EXE_legion-input-driver"))
        .arg("--conformance-run")
        .arg("--product")
        .arg(&absent_product)
        .arg("--report")
        .arg(&report_path)
        .status()
        .expect("the driver binary must be runnable");

    assert_ne!(
        status.code(),
        Some(0),
        "a run with no product to drive must never exit 0"
    );
    assert_eq!(
        status.code(),
        Some(3),
        "an absent packaged product is blocked, not a conformance failure: the product is not \
         implicated by its own absence"
    );

    let text = fs::read_to_string(&report_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", report_path.display()));
    assert!(
        text.contains("status = \"blocked\""),
        "the result must say blocked:\n{text}"
    );
    assert!(
        !text.contains("window_created = true"),
        "no window can have been observed:\n{text}"
    );
    for class in report::INPUT_CLASSES {
        assert!(
            !text.contains(&format!("input_class_{class} =")),
            "class `{class}` was never observed, so no line may claim any outcome for it:\n{text}"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn driver_report_markers_match_the_harness_wire_protocol_verbatim() {
    // These literals are written out here on purpose. If this test imported the
    // strings it asserts, a rename on both sides would keep it green while the
    // harness stopped recognising the driver's reports.
    let session = report::render_session_report(&attached());
    assert!(
        session.contains("interactive_session = true"),
        "session handshake marker drifted:\n{session}"
    );

    let run = report::render_conformance_report(&conforming_report());
    assert!(
        run.contains("window_created = true"),
        "window marker drifted:\n{run}"
    );
    for marker in [
        "input_class_keyboard = \"conforms\"",
        "input_class_pointer = \"conforms\"",
        "input_class_text = \"conforms\"",
        "input_class_clipboard = \"conforms\"",
        "input_class_ime-cjk = \"conforms\"",
        "input_class_command = \"conforms\"",
    ] {
        assert!(
            run.contains(marker),
            "per-class marker `{marker}` drifted:\n{run}"
        );
    }

    assert_eq!(
        report::INPUT_CLASSES,
        [
            "keyboard",
            "pointer",
            "text",
            "clipboard",
            "ime-cjk",
            "command"
        ],
        "the six class names are the harness's, not this crate's, to rename"
    );
    assert_eq!(
        report::conformance_exit_code(&conforming_report()),
        0,
        "a windowed six-of-six run is the only thing that exits 0"
    );
}

#[test]
fn an_unobserved_input_class_is_never_rendered_as_conforms() {
    let partial = ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: true,
        observations: vec![
            ClassObservation::new("keyboard", ClassOutcome::Conforms, "QAB observed"),
            ClassObservation::new("pointer", ClassOutcome::Deviates, "focus missed the point"),
            ClassObservation::new("text", ClassOutcome::Blocked, "no TextPattern"),
        ],
        prerequisite: None,
        notes: Vec::new(),
    };

    let text = report::render_conformance_report(&partial);
    assert!(text.contains("input_class_keyboard = \"conforms\""));
    assert!(text.contains("input_class_pointer = \"deviates\""));
    assert!(text.contains("input_class_text = \"blocked\""));
    for unobserved in ["clipboard", "ime-cjk", "command"] {
        assert!(
            !text.contains(&format!("input_class_{unobserved} =")),
            "class `{unobserved}` was never observed, so it must produce no line at all:\n{text}"
        );
    }
    assert_eq!(
        text.matches("= \"conforms\"").count(),
        1,
        "exactly one class was observed to conform:\n{text}"
    );

    // A detail string that spells the marker cannot forge one either.
    let forged = ConformanceReport {
        product: "package/legion-desktop.exe".to_string(),
        window_created: false,
        observations: vec![ClassObservation::new(
            "keyboard",
            ClassOutcome::Blocked,
            "the oracle wanted input_class_clipboard = \"conforms\" and window_created = true",
        )],
        prerequisite: None,
        notes: vec!["window_created = true".to_string()],
    };
    let forged_text = report::render_conformance_report(&forged);
    assert!(
        !forged_text.contains("input_class_clipboard = \"conforms\""),
        "free-form text must not be able to forge a class marker:\n{forged_text}"
    );
    assert!(
        !forged_text.contains("window_created = true"),
        "free-form text must not be able to forge the window marker:\n{forged_text}"
    );
    assert_ne!(
        report::conformance_exit_code(&partial),
        0,
        "a run missing three classes is not a pass"
    );
}

#[test]
fn unsupported_host_build_reports_blocked_and_exits_nonzero() {
    // The macOS and Linux builds route both subcommands through these two
    // reports. They stay blocked on BLK-2026-09-08-02, and a Windows result
    // never substitutes for either row.
    let session = report::render_session_report(&DesktopAttachment::UnsupportedHost {
        detail: "`macos` is not a native input host enabled by ADR-0056".to_string(),
    });
    assert!(session.contains("status = \"blocked\""));
    assert!(!session.contains("interactive_session = true"));
    assert!(
        session.contains("BLK-2026-09-08-02"),
        "the unsupported-host prerequisite must name the blocker it belongs to:\n{session}"
    );

    let run = report::unsupported_host_conformance_report("package/legion-desktop");
    assert!(
        run.observations.is_empty(),
        "no class can have been observed"
    );
    assert!(!run.window_created, "no window can have been observed");
    let code = report::conformance_exit_code(&run);
    assert_ne!(code, 0, "an unsupported host must never exit 0");
    assert_eq!(code, 3, "an unsupported host is blocked");
    assert_eq!(report::conformance_status(&run), "blocked");

    let text = report::render_conformance_report(&run);
    assert!(text.contains("BLK-2026-09-08-02"), "{text}");
    assert!(
        text.contains("never substitutes for a macOS or Linux row"),
        "the report must carry the substitution rule, not just the blocker id:\n{text}"
    );
}

#[test]
fn sha256_matches_published_known_answer_vectors() {
    // FIPS 180-4 / NIST CAVP known answers. This crate carries its own SHA-256
    // so its dependency admission stays limited to the `windows` crate; an
    // unverified digest would make the text and command oracles worthless.
    assert_eq!(
        observe::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        observe::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        observe::sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

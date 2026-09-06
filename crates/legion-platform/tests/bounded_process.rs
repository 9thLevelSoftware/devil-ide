use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use legion_platform::{
    BoundedProcessRequest, NativeProcessService, PlatformError, ProcessRequest, ProcessService,
};

fn bounded_with(
    command: String,
    args: Vec<String>,
    cancellation: Arc<AtomicBool>,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> BoundedProcessRequest {
    BoundedProcessRequest::new(
        ProcessRequest {
            command,
            args,
            cwd: None,
            env: Vec::new(),
            stdin: None,
            timeout: None,
            cancelled: false,
        },
        stdout_limit,
        stderr_limit,
        timeout,
        cancellation,
    )
}

fn bounded(
    command: String,
    args: Vec<String>,
    cancellation: Arc<AtomicBool>,
) -> BoundedProcessRequest {
    bounded_with(command, args, cancellation, 64, 64, Duration::from_secs(2))
}

fn fixture_request(mode: &str) -> BoundedProcessRequest {
    let mut request = ProcessRequest::new(
        std::env::current_exe()
            .expect("test executable")
            .to_string_lossy()
            .to_string(),
    );
    request.args = vec![
        "--exact".to_string(),
        "bounded_child_fixture".to_string(),
        "--nocapture".to_string(),
    ];
    request.env = vec![("LEGION_BOUNDED_FIXTURE".to_string(), mode.to_string())];
    BoundedProcessRequest::new(
        request,
        64,
        64,
        Duration::from_millis(500),
        Arc::new(AtomicBool::new(false)),
    )
}

fn fixture_request_with_marker(mode: &str, marker: &std::path::Path) -> BoundedProcessRequest {
    let mut request = fixture_request(mode);
    request.process.env.push((
        "LEGION_BOUNDED_READY_MARKER".to_string(),
        marker.to_string_lossy().into_owned(),
    ));
    request.max_stdout_bytes = 4096;
    request.max_stderr_bytes = 4096;
    request
}

#[test]
// This fixture intentionally leaves the pipe-holding descendant alive so the
// outer bounded runner must clean it up through its process group/job.
#[allow(clippy::zombie_processes)]
fn bounded_child_fixture() {
    let Ok(mode) = std::env::var("LEGION_BOUNDED_FIXTURE") else {
        return;
    };
    if mode == "hold" {
        if let Ok(marker) = std::env::var("LEGION_BOUNDED_READY_MARKER") {
            let _ = std::fs::write(marker, b"ready");
        }
        thread::sleep(Duration::from_secs(10));
    } else if mode == "descendant" {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command.args(["--exact", "bounded_child_fixture", "--nocapture"]);
        command.env("LEGION_BOUNDED_FIXTURE", "hold");
        let _child = command.spawn().expect("spawn pipe holder");
        if let Ok(marker) = std::env::var("LEGION_BOUNDED_READY_MARKER") {
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline && !std::path::Path::new(&marker).exists() {
                thread::sleep(Duration::from_millis(5));
            }
            assert!(
                std::path::Path::new(&marker).exists(),
                "descendant did not signal readiness"
            );
            println!("descendant-ready");
        }
    }
}

fn emit_command() -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "[Console]::Out.Write('ok')".to_string(),
            ],
        )
    }
    #[cfg(not(windows))]
    {
        (
            "sh".to_string(),
            vec!["-c".to_string(), "printf ok".to_string()],
        )
    }
}

#[test]
fn bounded_process_returns_normal_output() {
    let (command, args) = emit_command();
    let service = NativeProcessService;
    let result = service
        .execute_bounded(&bounded(command, args, Arc::new(AtomicBool::new(false))))
        .expect("bounded process");
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "ok");
}

#[test]
fn bounded_process_terminates_on_stdout_overflow() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::Out.Write('x' * 4096)".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "head -c 4096 /dev/zero".to_string()],
            )
        }
    };
    let service = NativeProcessService;
    let result = service.execute_bounded(&bounded(command, args, Arc::new(AtomicBool::new(false))));
    assert!(matches!(
        result,
        Err(PlatformError::ProcessOutputLimit {
            stream: "stdout",
            ..
        })
    ));
}

#[test]
fn bounded_process_terminates_on_stderr_overflow() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::Error.Write('x' * 4096)".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "head -c 4096 /dev/zero >&2".to_string()],
            )
        }
    };
    let service = NativeProcessService;
    let result = service.execute_bounded(&bounded(command, args, Arc::new(AtomicBool::new(false))));
    assert!(matches!(
        result,
        Err(PlatformError::ProcessOutputLimit {
            stream: "stderr",
            ..
        })
    ));
}

#[test]
fn bounded_process_enforces_timeout_and_kills_descendants_holding_pipes() {
    let marker = std::env::temp_dir().join(format!(
        "legion-bounded-ready-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_file(&marker);
    let request = fixture_request_with_marker("descendant", &marker);
    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    let _ = std::fs::remove_file(&marker);
    assert!(
        result.is_ok(),
        "parent exit should allow bounded drain: {result:?}"
    );
    assert!(result.unwrap().stdout.contains("descendant-ready"));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "pipe-holding child was not reaped"
    );
}

#[test]
fn bounded_process_enforces_actual_timeout() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "Start-Sleep -Seconds 10".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "sleep 10".to_string()],
            )
        }
    };
    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&bounded_with(
        command,
        args,
        Arc::new(AtomicBool::new(false)),
        64,
        64,
        Duration::from_millis(100),
    ));
    assert!(matches!(result, Err(PlatformError::Timeout { .. })));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn bounded_process_rejects_stdin_before_spawn() {
    let (command, args) = emit_command();
    let mut request = bounded(command, args, Arc::new(AtomicBool::new(false)));
    request.process.stdin = Some(b"must not be written".to_vec());
    let result = NativeProcessService.execute_bounded(&request);
    assert!(matches!(
        result,
        Err(PlatformError::UnsupportedOperation { .. })
    ));
}

#[test]
fn bounded_process_honors_prelaunch_cancellation() {
    let (command, args) = emit_command();
    let cancellation = Arc::new(AtomicBool::new(true));
    let result = NativeProcessService.execute_bounded(&bounded(command, args, cancellation));
    assert!(matches!(result, Err(PlatformError::Cancelled { .. })));
}

#[test]
fn bounded_process_rejects_invalid_utf8() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::OpenStandardOutput().WriteByte(255)".to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "printf '\\377'".to_string()],
            )
        }
    };
    let result = NativeProcessService.execute_bounded(&bounded(
        command,
        args,
        Arc::new(AtomicBool::new(false)),
    ));
    assert!(matches!(result, Err(PlatformError::Encoding { .. })));
}

#[test]
fn bounded_process_caps_both_streams_in_one_child() {
    let (command, args) = {
        #[cfg(windows)]
        {
            (
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "[Console]::Out.Write('x' * 4096); [Console]::Error.Write('y' * 4096)"
                        .to_string(),
                ],
            )
        }
        #[cfg(not(windows))]
        {
            (
                "sh".to_string(),
                vec![
                    "-c".to_string(),
                    "head -c 4096 /dev/zero & head -c 4096 /dev/zero >&2; wait".to_string(),
                ],
            )
        }
    };
    let result = NativeProcessService.execute_bounded(&bounded(
        command,
        args,
        Arc::new(AtomicBool::new(false)),
    ));
    assert!(matches!(
        result,
        Err(PlatformError::ProcessOutputLimit { .. })
    ));
}

#[test]
fn bounded_process_honors_live_cancellation() {
    let cancellation = Arc::new(AtomicBool::new(false));
    let request = {
        #[cfg(windows)]
        {
            bounded(
                "powershell".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "Start-Sleep -Seconds 10".to_string(),
                ],
                cancellation.clone(),
            )
        }
        #[cfg(not(windows))]
        {
            bounded(
                "sh".to_string(),
                vec!["-c".to_string(), "sleep 10".to_string()],
                cancellation.clone(),
            )
        }
    };
    let flag = cancellation.clone();
    let join = thread::spawn(move || NativeProcessService.execute_bounded(&request));
    thread::sleep(Duration::from_millis(50));
    flag.store(true, Ordering::Release);
    let result = join.join().expect("bounded worker");
    assert!(matches!(result, Err(PlatformError::Cancelled { .. })));
}

#[test]
fn bounded_process_requires_finite_timeout() {
    let (command, args) = emit_command();
    let mut request = bounded(command, args, Arc::new(AtomicBool::new(false)));
    request.timeout = Duration::ZERO;
    let started = Instant::now();
    let result = NativeProcessService.execute_bounded(&request);
    assert!(matches!(
        result,
        Err(PlatformError::UnsupportedOperation { .. })
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
}

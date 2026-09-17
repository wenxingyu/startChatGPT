//! Fault injection with real child processes and pipes; no account or network access.
use super::*;
use std::io::BufRead;
use std::sync::atomic::{AtomicUsize, Ordering};

fn mock_command(mode: &str, release: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "usage::recovery_tests::mock_server",
            "--ignored",
            "--nocapture",
        ])
        .env("QUOTA_TEST_MODE", mode)
        .env("QUOTA_TEST_RELEASE", release)
        .creation_flags(0x0800_0000);
    command
}

#[test]
#[ignore = "subprocess fixture, invoked by recovery tests"]
fn mock_server() {
    let Ok(mode) = std::env::var("QUOTA_TEST_MODE") else {
        return;
    };
    let release = PathBuf::from(std::env::var_os("QUOTA_TEST_RELEASE").unwrap());
    if mode == "frames" {
        let payload = format!(
            "{}\r\n{}",
            json!({"text": "额度".repeat(3000)}),
            json!({"last": true})
        );
        let mut output = std::io::stdout().lock();
        for chunk in payload.as_bytes().chunks(127) {
            output.write_all(chunk).unwrap();
            output.flush().unwrap();
        }
        // Do not let the test harness append text after our unterminated frame.
        std::process::exit(0);
    }
    if mode == "hold-pipe" {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !release.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        return;
    }
    let mut requests = 0;
    let mut holder = None;
    for line in std::io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line.unwrap()).unwrap();
        requests += 1;
        if requests == 2 && mode == "timeout" {
            thread::sleep(Duration::from_secs(60));
            return;
        }
        let response = if requests == 2 && mode == "held-pipe" {
            // The descendant inherits stdout. Killing this server does not close
            // the pipe while the descendant remains alive.
            holder = Some(
                mock_command("hold-pipe", &release)
                    .stdin(Stdio::null())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            json!({"id": request["id"], "error": {"code": -1}})
        } else {
            let used = if mode == "healthy" { 37 } else { 0 };
            json!({"id": request["id"], "result": {"rateLimits": {
                "primary": {"usedPercent": used, "windowDurationMins": 300}
            }}})
        };
        println!("{response}");
        std::io::stdout().flush().unwrap();
    }
    if let Some(mut holder) = holder {
        let _ = holder.wait();
    }
}

fn mock_bridge(mode: &str, release: &Path) -> Result<Bridge, String> {
    let mut child = mock_command(mode, release)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (reader, messages) = MessageReader::spawn(output);
    Ok(Bridge {
        child,
        input,
        messages,
        reader,
        next_id: 1,
    })
}

fn wait_state(
    state: &Mutex<State>,
    timeout: Duration,
    predicate: impl Fn(&State) -> bool,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        // The UI uses this same mutex for details and redraws. A blocked lock
        // must fail the test instead of hanging the test runner.
        match state.try_lock() {
            Ok(snapshot) if predicate(&snapshot) => return Ok(()),
            Err(std::sync::TryLockError::Poisoned(_)) => return Err("state poisoned".into()),
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err("state unavailable or not updated before deadline".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn verify_recovery(mode: &'static str, manual: bool) {
    let release = std::env::temp_dir().join(format!(
        "quota-recovery-{}-{mode}-{manual}.release",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&release);
    let state = Arc::new(Mutex::new(State::default()));
    let connections = Arc::new(AtomicUsize::new(0));
    let attempts = connections.clone();
    let child_release = release.clone();
    let (action, worker) = start_with_connector(
        move || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            mock_bridge(if attempt == 0 { mode } else { "healthy" }, &child_release)
        },
        state.clone(),
        None,
    );
    let result = (|| -> Result<(), String> {
        wait_state(&state, Duration::from_secs(5), |s| {
            !s.stale() && s.windows[0].as_ref().is_some_and(|w| w.remaining == 100.0)
        })?;
        action.send(Action::Refresh).unwrap();
        if mode == "timeout" {
            // While the real 15-second request timeout is pending, the state
            // must remain accessible to the UI.
            thread::sleep(Duration::from_millis(200));
            wait_state(&state, Duration::from_secs(1), |_| true)?;
        }
        wait_state(&state, Duration::from_secs(20), |s| {
            s.error.is_some()
                && s.stale()
                && s.windows[0].as_ref().is_some_and(|w| w.remaining == 100.0)
        })?;
        if manual {
            action.send(Action::Refresh).unwrap();
        }
        wait_state(
            &state,
            Duration::from_secs(if manual { 5 } else { 55 }),
            |s| {
                !s.stale()
                    && s.error.is_none()
                    && s.windows[0].as_ref().is_some_and(|w| w.remaining == 63.0)
            },
        )?;
        if connections.load(Ordering::SeqCst) != 2 {
            return Err("expected exactly one reconnect".into());
        }
        Ok(())
    })();
    // Release inherited stdout even on a regression, so an old blocking join
    // can finish and all test-owned processes are allowed to exit.
    std::fs::write(&release, b"release").unwrap();
    let _ = action.send(Action::Stop);
    worker.join().unwrap();
    // Keep the release marker: a detached holder may not have observed it yet.
    assert!(result.is_ok(), "{mode}: {result:?}");
}

#[test]
fn timeout_preserves_ui_access_and_manual_refresh_recovers() {
    verify_recovery("timeout", true);
}

#[test]
fn inherited_stdout_does_not_freeze_state_or_reconnect() {
    verify_recovery("held-pipe", true);
}

#[test]
fn failed_connection_automatically_recovers_without_clicking_refresh() {
    verify_recovery("held-pipe", false);
}

#[test]
fn reader_recovers_split_unicode_frames_and_final_line_without_newline() {
    let bridge = mock_bridge("frames", Path::new("unused")).unwrap();
    assert_eq!(
        bridge
            .messages
            .recv_timeout(Duration::from_secs(5))
            .unwrap(),
        json!({"text": "额度".repeat(3000)})
    );
    assert_eq!(
        bridge
            .messages
            .recv_timeout(Duration::from_secs(5))
            .unwrap(),
        json!({"last": true})
    );
}

#[test]
fn reader_can_be_cancelled_immediately_after_spawn() {
    for _ in 0..10 {
        let mut bridge = mock_bridge("healthy", Path::new("unused")).unwrap();
        assert!(bridge.reader.shutdown());
        assert!(bridge.reader.handle.is_none());
    }
}

#[test]
fn reader_is_reclaimed_while_descendant_still_holds_stdout() {
    let release = std::env::temp_dir().join(format!(
        "quota-reader-cancel-{}.release",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&release);
    let mut bridge = mock_bridge("held-pipe", &release).unwrap();
    let result = (|| {
        bridge.request("account/rateLimits/read", None)?;
        if bridge.request("account/rateLimits/read", None).is_ok() {
            return Err("fixture did not inject the error".to_string());
        }
        if !bridge.reader.shutdown() || bridge.reader.handle.is_some() {
            return Err("reader survived cancellation while stdout remained open".into());
        }
        Ok(())
    })();
    std::fs::write(&release, b"release").unwrap();
    drop(bridge);
    assert!(result.is_ok(), "{result:?}");
}

//! Integration tests for auriga-pty public API

use auriga_pty::PtyHandle;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn tmp_dir() -> PathBuf {
    std::env::temp_dir()
}

fn read_all(handle: &PtyHandle, timeout: Duration) -> Vec<u8> {
    let start = Instant::now();
    let mut collected = Vec::new();
    let settle = Duration::from_millis(100);
    let mut last_read = Instant::now();

    loop {
        if let Some(chunk) = handle.try_read() {
            collected.extend_from_slice(&chunk);
            last_read = Instant::now();
        } else if !collected.is_empty() && last_read.elapsed() >= settle {
            break;
        }

        if start.elapsed() >= timeout {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    collected
}

#[test]
fn spawn_command_and_read_output() {
    let handle =
        PtyHandle::spawn_with_args("echo", &["hello_pty"], &tmp_dir(), 80, 24, &[]).unwrap();

    let output = read_all(&handle, Duration::from_secs(3));
    let text = String::from_utf8_lossy(&output);

    assert!(text.contains("hello_pty"));
}

#[test]
fn spawn_returns_valid_pid() {
    let handle = PtyHandle::spawn_with_args("echo", &["test"], &tmp_dir(), 80, 24, &[]).unwrap();

    let pid = handle.child_pid();
    assert!(pid.is_some());
    assert!(pid.unwrap() > 0);
}

#[test]
fn write_input_to_pty() {
    let mut handle = PtyHandle::spawn("cat", &tmp_dir(), 80, 24, &[]).unwrap();

    thread::sleep(Duration::from_millis(200));

    handle.write_input(b"test_input\n").unwrap();

    let output = read_all(&handle, Duration::from_secs(3));
    let text = String::from_utf8_lossy(&output);

    assert!(text.contains("test_input"));
}

#[test]
fn spawn_with_env_vars() {
    let handle = PtyHandle::spawn_with_args(
        "sh",
        &["-c", "echo $TEST_VAR"],
        &tmp_dir(),
        80,
        24,
        &[("TEST_VAR", "env_test_value")],
    )
    .unwrap();

    let output = read_all(&handle, Duration::from_secs(3));
    let text = String::from_utf8_lossy(&output);

    assert!(text.contains("env_test_value"));
}

#[test]
fn spawn_invalid_command_fails() {
    let result = PtyHandle::spawn("nonexistent_command_xyz_12345", &tmp_dir(), 80, 24, &[]);

    assert!(result.is_err());
}

#[test]
fn resize_pty() {
    let handle = PtyHandle::spawn_with_args("sleep", &["1"], &tmp_dir(), 80, 24, &[]).unwrap();

    // Should not panic
    let result = handle.resize(120, 40);
    assert!(result.is_ok());
}

#[test]
fn try_read_returns_none_when_no_output() {
    let handle = PtyHandle::spawn_with_args("sleep", &["10"], &tmp_dir(), 80, 24, &[]).unwrap();

    thread::sleep(Duration::from_millis(50));

    // sleep produces no output
    let data = handle.try_read();
    assert!(data.is_none() || data.as_ref().map_or(false, |d| d.is_empty()));
}

#[test]
fn multiple_writes_and_reads() {
    let mut handle = PtyHandle::spawn("cat", &tmp_dir(), 80, 24, &[]).unwrap();
    thread::sleep(Duration::from_millis(200));

    for i in 0..3 {
        let msg = format!("line_{}\n", i);
        handle.write_input(msg.as_bytes()).unwrap();
    }

    let output = read_all(&handle, Duration::from_secs(3));
    let text = String::from_utf8_lossy(&output);

    assert!(text.contains("line_0"));
    assert!(text.contains("line_1"));
    assert!(text.contains("line_2"));
}

//! Integration tests for auriga CLI
//!
//! These tests verify the CLI behavior as a subprocess.

use std::process::Command;

fn auriga_binary() -> std::path::PathBuf {
    // In test mode, the binary is in target/debug/
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps/
    path.push("auriga");
    path
}

#[test]
#[ignore = "requires built binary"]
fn cli_version_exits_zero() {
    let output = Command::new(auriga_binary())
        .arg("--version")
        .output()
        .expect("failed to run auriga");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("auriga v"));
}

#[test]
#[ignore = "requires built binary"]
fn cli_help_exits_zero() {
    let output = Command::new(auriga_binary())
        .arg("--help")
        .output()
        .expect("failed to run auriga");

    assert!(output.status.success() || output.status.code() == Some(0));
}

#[test]
#[ignore = "requires built binary"]
fn cli_unknown_command_exits_nonzero() {
    let output = Command::new(auriga_binary())
        .arg("unknown_command_xyz")
        .output()
        .expect("failed to run auriga");

    assert!(!output.status.success());
}

// Unit-style tests that don't require the binary

#[test]
fn version_format() {
    // VERSION comes from Cargo.toml
    let version = env!("CARGO_PKG_VERSION");
    let parts: Vec<&str> = version.split('.').collect();

    // Should be semver format (major.minor.patch)
    assert_eq!(parts.len(), 3);
    for part in parts {
        assert!(part.parse::<u32>().is_ok());
    }
}

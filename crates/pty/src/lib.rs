use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::thread;

pub struct PtyHandle {
    writer: Box<dyn Write + Send>,
    pub reader_rx: mpsc::Receiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    child_pid: Option<u32>,
}

impl PtyHandle {
    pub fn spawn(
        command: &str,
        working_dir: &Path,
        cols: u16,
        rows: u16,
        env: &[(&str, &str)],
    ) -> Result<Self> {
        Self::spawn_with_args(command, &[], working_dir, cols, rows, env)
    }

    pub fn spawn_with_args(
        command: &str,
        args: &[&str],
        working_dir: &Path,
        cols: u16,
        rows: u16,
        env: &[(&str, &str)],
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(command);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.cwd(working_dir);
        cmd.env("TERM", "xterm-256color");
        for (key, value) in env {
            cmd.env(key, value);
        }

        let child = pair.slave.spawn_command(cmd)?;
        let child_pid = child.process_id();

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            writer,
            reader_rx: rx,
            master: pair.master,
            child_pid,
        })
    }

    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn try_read(&self) -> Option<Vec<u8>> {
        self.reader_rx.try_recv().ok()
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Helper: read all available output from a PtyHandle, retrying for up to `timeout`.
    /// Returns the concatenated bytes once at least one byte has been received and
    /// no new data arrives for a short settling period, or when the timeout expires.
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
                // We got data and nothing new for a while -- done.
                break;
            }

            if start.elapsed() >= timeout {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        collected
    }

    fn tmp_dir() -> std::path::PathBuf {
        std::env::temp_dir()
    }

    #[test]
    fn spawn_echo_returns_valid_pid() {
        let handle = PtyHandle::spawn_with_args("echo", &["hello"], &tmp_dir(), 80, 24, &[])
            .expect("spawn echo should succeed");

        // child_pid should be a real pid (Some with a nonzero value)
        let pid = handle.child_pid();
        assert!(pid.is_some(), "child_pid should be Some");
        assert!(pid.unwrap() > 0, "pid should be positive");

        // We should also be able to read the echoed output.
        let output = read_all(&handle, Duration::from_secs(3));
        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("hello"),
            "expected 'hello' in output, got: {text:?}"
        );
    }

    #[test]
    fn write_input_and_read_output_via_cat() {
        let mut handle =
            PtyHandle::spawn("cat", &tmp_dir(), 80, 24, &[]).expect("spawn cat should succeed");

        // Give cat a moment to start.
        thread::sleep(Duration::from_millis(200));

        let payload = b"pty_test_data\n";
        handle
            .write_input(payload)
            .expect("write_input should succeed");

        let output = read_all(&handle, Duration::from_secs(3));
        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("pty_test_data"),
            "expected 'pty_test_data' in output, got: {text:?}"
        );
    }

    #[test]
    fn spawn_invalid_command_returns_err() {
        let result = PtyHandle::spawn("this_command_does_not_exist_9999", &tmp_dir(), 80, 24, &[]);
        assert!(
            result.is_err(),
            "spawning a nonexistent command should return Err"
        );
    }

    #[test]
    fn try_read_on_fresh_handle_returns_none_initially() {
        // Spawn a command that produces no immediate output and doesn't exit
        // quickly. `sleep` is ideal: it just waits.
        let handle = PtyHandle::spawn_with_args("sleep", &["10"], &tmp_dir(), 80, 24, &[])
            .expect("spawn sleep should succeed");

        // Immediately try to read -- there should be nothing yet.
        // Give the reader thread a tiny moment to start, but sleep itself
        // should not produce output.
        thread::sleep(Duration::from_millis(50));
        let data = handle.try_read();
        assert!(
            data.is_none() || data.as_ref().map_or(false, |d| d.is_empty()),
            "expected no output from sleep, got: {:?}",
            data
        );
    }
}

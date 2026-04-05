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

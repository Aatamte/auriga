use crate::helpers::git_diff_stat;
use crate::types::{DiffResult, FileEvent};
use crossterm::event::{self, Event};
use notify::{RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn start_input_thread() -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || loop {
        // Block up to 50ms waiting for input, then loop so thread can exit
        // when channel disconnects
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(evt) = event::read() {
                if tx.send(evt).is_err() {
                    break; // Main thread dropped receiver, exit
                }
            }
        }
    });
    rx
}

pub fn start_diff_thread() -> (mpsc::Sender<PathBuf>, mpsc::Receiver<DiffResult>) {
    let (path_tx, path_rx) = mpsc::channel::<PathBuf>();
    let (result_tx, result_rx) = mpsc::channel::<DiffResult>();

    thread::spawn(move || {
        while let Ok(path) = path_rx.recv() {
            if let Some((added, removed)) = git_diff_stat(&path) {
                if result_tx
                    .send(DiffResult {
                        path,
                        added,
                        removed,
                    })
                    .is_err()
                {
                    tracing::debug!("diff result channel closed");
                    break;
                }
            }
        }
    });

    (path_tx, result_rx)
}

fn should_ignore_path(path: &Path) -> bool {
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        match s.as_ref() {
            ".git" | "target" | "node_modules" | ".DS_Store" | "__pycache__" => return true,
            _ => {}
        }
    }
    // Ignore common temp/lock files
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.ends_with(".swp")
            || name.ends_with(".swo")
            || name.ends_with('~')
            || name.ends_with(".tmp")
            || name.starts_with(".#")
        {
            return true;
        }
    }
    false
}

pub fn start_file_watcher(tx: mpsc::Sender<FileEvent>) -> notify::Result<impl Watcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        match event.kind {
            notify::EventKind::Create(_) => {
                for path in event.paths {
                    if !should_ignore_path(&path) && tx.send(FileEvent::Created(path)).is_err() {
                        tracing::debug!("file event channel closed");
                    }
                }
            }
            notify::EventKind::Modify(_) => {
                for path in event.paths {
                    if !should_ignore_path(&path) && tx.send(FileEvent::Modified(path)).is_err() {
                        tracing::debug!("file event channel closed");
                    }
                }
            }
            notify::EventKind::Remove(_) => {
                for path in event.paths {
                    if !should_ignore_path(&path) && tx.send(FileEvent::Removed(path)).is_err() {
                        tracing::debug!("file event channel closed");
                    }
                }
            }
            _ => {
                if event.paths.len() == 2
                    && !should_ignore_path(&event.paths[1])
                    && tx
                        .send(FileEvent::Renamed(
                            event.paths[0].clone(),
                            event.paths[1].clone(),
                        ))
                        .is_err()
                {
                    tracing::debug!("file event channel closed");
                }
            }
        }
    })?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    watcher.watch(&cwd, RecursiveMode::Recursive)?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_git_directory() {
        assert!(should_ignore_path(Path::new("/project/.git/objects/abc")));
    }

    #[test]
    fn ignores_target_directory() {
        assert!(should_ignore_path(Path::new("/project/target/debug/app")));
    }

    #[test]
    fn ignores_node_modules() {
        assert!(should_ignore_path(Path::new(
            "/project/node_modules/pkg/index.js"
        )));
    }

    #[test]
    fn ignores_swap_files() {
        assert!(should_ignore_path(Path::new("/project/src/main.rs.swp")));
        assert!(should_ignore_path(Path::new("/project/src/main.rs.swo")));
    }

    #[test]
    fn ignores_tilde_backup_files() {
        assert!(should_ignore_path(Path::new("/project/src/main.rs~")));
    }

    #[test]
    fn ignores_tmp_files() {
        assert!(should_ignore_path(Path::new("/project/data.tmp")));
    }

    #[test]
    fn ignores_emacs_lock_files() {
        assert!(should_ignore_path(Path::new("/project/src/.#main.rs")));
    }

    #[test]
    fn allows_normal_source_files() {
        assert!(!should_ignore_path(Path::new("/project/src/main.rs")));
        assert!(!should_ignore_path(Path::new("/project/Cargo.toml")));
    }

    #[test]
    fn ignores_pycache() {
        assert!(should_ignore_path(Path::new(
            "/project/__pycache__/module.pyc"
        )));
    }

    #[test]
    fn ignores_ds_store() {
        assert!(should_ignore_path(Path::new("/project/.DS_Store")));
    }
}

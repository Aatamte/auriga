use ignore::WalkBuilder;
use auriga_core::FileEntry;
use std::path::Path;

pub fn git_diff_stat(path: &Path) -> Option<(usize, usize)> {
    let output = std::process::Command::new("git")
        .args(["diff", "--numstat", "--"])
        .arg(path)
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?;
    let mut parts = line.split('\t');
    let added: usize = parts.next()?.parse().ok()?;
    let removed: usize = parts.next()?.parse().ok()?;
    Some((added, removed))
}

pub fn walk_directory(root: &Path) -> Vec<FileEntry> {
    let mut entries = Vec::new();

    for result in WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .sort_by_file_path(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.cmp(b),
            }
        })
        .build()
    {
        let Ok(dir_entry) = result else { continue };
        let path = dir_entry.path().to_path_buf();

        if path == *root {
            continue;
        }

        let depth = path
            .strip_prefix(root)
            .map(|p| p.components().count())
            .unwrap_or(0);

        if depth == 0 {
            continue;
        }

        let depth = depth - 1;

        if path.is_dir() {
            entries.push(FileEntry::dir(path, depth));
        } else {
            entries.push(FileEntry::file(path, depth));
        }
    }

    entries
}

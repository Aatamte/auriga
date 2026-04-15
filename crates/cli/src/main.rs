use std::fs;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    eprintln!("auriga v{VERSION}");
    eprintln!();
    eprintln!("Usage: auriga [command]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  (none)     Launch the Auriga");
    eprintln!("  update     Update to the latest version");
    eprintln!("  version    Show version");
    eprintln!("  help       Show this help message");
}

fn target_arch() -> &'static str {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else {
        "unknown"
    }
}

fn update() -> anyhow::Result<()> {
    println!("Checking for updates...");

    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner("Aatamte")
        .repo_name("auriga")
        .build()?
        .fetch()?;

    let latest = releases
        .first()
        .ok_or_else(|| anyhow::anyhow!("No releases found"))?;
    let latest_version = latest.version.trim_start_matches('v');

    if latest_version == VERSION {
        println!("Already on latest version (v{VERSION}).");
        return Ok(());
    }

    println!("New version available: v{VERSION} -> v{latest_version}");

    let arch = target_arch();
    let asset_name = format!("auriga-{arch}.tar.gz");
    let asset = latest
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| anyhow::anyhow!("No asset found for {arch}"))?;

    println!("Downloading {asset_name}...");

    let tmp_dir = tempfile::tempdir()?;
    let tmp_tarball = tmp_dir.path().join(&asset_name);

    let mut tmp_file = fs::File::create(&tmp_tarball)?;
    self_update::Download::from_url(&asset.download_url)
        .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
        .download_to(&mut tmp_file)?;
    drop(tmp_file);

    println!("Extracting...");

    self_update::Extract::from_source(&tmp_tarball)
        .archive(self_update::ArchiveKind::Tar(Some(
            self_update::Compression::Gz,
        )))
        .extract_into(tmp_dir.path())?;

    let bin_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine binary directory"))?
        .to_path_buf();

    for bin in ["auriga", "auriga-app"] {
        let src = tmp_dir.path().join(bin);
        let dst = bin_dir.join(bin);
        if src.exists() {
            println!("Installing {bin}...");
            let tmp_dst = bin_dir.join(format!("{bin}.new"));
            fs::copy(&src, &tmp_dst)?;
            self_update::Move::from_source(&tmp_dst)
                .replace_using_temp(&dst)
                .to_dest(&dst)?;
        }
    }

    println!("Updated to v{latest_version}");
    Ok(())
}

fn launch() -> anyhow::Result<()> {
    let bin = app_binary_path()?;

    if !bin.exists() {
        anyhow::bail!(
            "auriga-app not found at {}. Run `cargo build` first.",
            bin.display()
        );
    }

    let status = Command::new(&bin).status()?;
    std::process::exit(status.code().unwrap_or(1));
}

#[derive(Debug, PartialEq)]
enum Cmd<'a> {
    Launch,
    Update,
    Version,
    Help,
    Unknown(&'a str),
}

fn resolve_command(args: &[String]) -> Cmd<'_> {
    match args.get(1).map(|s| s.as_str()) {
        None => Cmd::Launch,
        Some("update") => Cmd::Update,
        Some("version" | "--version" | "-v") => Cmd::Version,
        Some("help" | "--help" | "-h") => Cmd::Help,
        Some(other) => Cmd::Unknown(other),
    }
}

fn app_binary_path() -> anyhow::Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine binary directory"))?;
    Ok(dir.join("auriga-app"))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match resolve_command(&args) {
        Cmd::Launch => launch(),
        Cmd::Update => update(),
        Cmd::Version => {
            println!("auriga v{VERSION}");
            Ok(())
        }
        Cmd::Help => {
            usage();
            Ok(())
        }
        Cmd::Unknown(other) => {
            eprintln!("Unknown command: {other}");
            usage();
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(vals: &[&str]) -> Vec<String> {
        vals.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_returns_launch() {
        assert_eq!(resolve_command(&args(&["auriga"])), Cmd::Launch);
    }

    #[test]
    fn update_arg_recognized() {
        assert_eq!(resolve_command(&args(&["auriga", "update"])), Cmd::Update);
    }

    #[test]
    fn version_variants() {
        assert_eq!(resolve_command(&args(&["auriga", "version"])), Cmd::Version);
        assert_eq!(
            resolve_command(&args(&["auriga", "--version"])),
            Cmd::Version
        );
        assert_eq!(resolve_command(&args(&["auriga", "-v"])), Cmd::Version);
    }

    #[test]
    fn help_variants() {
        assert_eq!(resolve_command(&args(&["auriga", "help"])), Cmd::Help);
        assert_eq!(resolve_command(&args(&["auriga", "--help"])), Cmd::Help);
        assert_eq!(resolve_command(&args(&["auriga", "-h"])), Cmd::Help);
    }

    #[test]
    fn unknown_command() {
        assert_eq!(
            resolve_command(&args(&["auriga", "foo"])),
            Cmd::Unknown("foo")
        );
    }

    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION should be major.minor.patch");
        for part in &parts {
            assert!(
                part.parse::<u32>().is_ok(),
                "each version component should be numeric, got '{part}'"
            );
        }
    }

    #[test]
    fn app_binary_is_sibling_of_exe() {
        let path = app_binary_path().expect("should resolve binary path");
        assert_eq!(path.file_name().unwrap(), "auriga-app");
        let exe_dir = std::env::current_exe().unwrap();
        assert_eq!(path.parent().unwrap(), exe_dir.parent().unwrap());
    }

    #[test]
    fn target_arch_matches_release_naming() {
        let arch = target_arch();
        // Must match one of the release workflow targets
        let valid = [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux-gnu",
        ];
        assert!(
            valid.contains(&arch) || arch == "unknown",
            "target_arch() returned '{arch}' which doesn't match release targets"
        );
    }
}

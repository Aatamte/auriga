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

fn update() -> anyhow::Result<()> {
    println!("Checking for updates...");

    let status = self_update::backends::github::Update::configure()
        .repo_owner("Aatamte")
        .repo_name("auriga")
        .bin_name("auriga")
        .current_version(VERSION)
        .show_download_progress(true)
        .build()?
        .update()?;

    if status.updated() {
        println!("Updated to v{}", status.version());
    } else {
        println!("Already on latest version (v{VERSION}).");
    }

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
}

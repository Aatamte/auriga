use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    eprintln!("aorch v{VERSION}");
    eprintln!();
    eprintln!("Usage: aorch [command]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  (none)     Launch the orchestrator TUI");
    eprintln!("  update     Update to the latest version");
    eprintln!("  version    Show version");
    eprintln!("  help       Show this help message");
}

fn update() -> anyhow::Result<()> {
    println!("Checking for updates...");

    let status = self_update::backends::github::Update::configure()
        .repo_owner("Aatamte")
        .repo_name("agent-orchestrator")
        .bin_name("aorch")
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
    let bin = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine binary directory"))?
        .join("orchestrator-app");

    if !bin.exists() {
        anyhow::bail!(
            "orchestrator-app not found at {}. Run `cargo build` first.",
            bin.display()
        );
    }

    let status = Command::new(&bin).status()?;
    std::process::exit(status.code().unwrap_or(1));
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str());

    match cmd {
        None => launch(),
        Some("update") => update(),
        Some("version" | "--version" | "-v") => {
            println!("aorch v{VERSION}");
            Ok(())
        }
        Some("help" | "--help" | "-h") => {
            usage();
            Ok(())
        }
        Some(other) => {
            eprintln!("Unknown command: {other}");
            usage();
            std::process::exit(1);
        }
    }
}

use std::path::PathBuf;

use blaksync::security_scan::{scan_repo, self_test, write_dummy_key};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "blaksync-secret-scan",
    about = "Scan BlakSync git history for Syncthing identity material"
)]
struct Args {
    #[arg(long, default_value = ".")]
    source: PathBuf,
    #[arg(long)]
    self_test: bool,
    #[arg(long, value_name = "PATH")]
    write_dummy_key: Option<PathBuf>,
}

fn run() -> blaksync::Result<()> {
    let args = Args::parse();
    if args.self_test {
        return self_test();
    }
    if let Some(path) = args.write_dummy_key {
        return write_dummy_key(path);
    }

    let source = args.source.canonicalize()?;
    let findings = scan_repo(&source)?;
    if findings.is_empty() {
        println!("secret scan clean: {}", source.display());
        return Ok(());
    }

    eprintln!("secret scan failed (values redacted):");
    for finding in findings {
        eprintln!("  {finding}");
    }
    std::process::exit(1);
}

fn main() {
    if let Err(error) = run() {
        eprintln!("secret-scan: {error}");
        std::process::exit(2);
    }
}

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "wiretrail", version, about = "HAR analyzer CLI")]
struct Cli {
    /// Path to the HAR file.
    file: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    eprintln!("wiretrail: {} (not yet implemented)", cli.file.display());
    std::process::exit(0);
}

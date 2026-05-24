use clap::{Parser, Subcommand};
use har::analysis::summary::{compute_summary, render_summary_text};
use har::assemble::assemble;
use har::filter::Filter;
use har::loader::load;
use har::render::{Envelope, ExitCode};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "wiretrail", version, about = "Fast HAR analyzer CLI")]
struct Cli {
    /// Path to the HAR file.
    file: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,

    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    /// Max items per list (top-N).
    #[arg(long, global = true, default_value_t = 10)]
    top: usize,

    /// Filter clauses, e.g. --filter "host:api.foo.com status:>=400".
    #[arg(long, global = true)]
    filter: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Executive summary of the capture (default).
    Summary,
}

fn main() {
    let cli = Cli::parse();

    let filter = match Filter::parse(&cli.filter) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("wiretrail: invalid filter: {e}");
            std::process::exit(ExitCode::InvalidHar as i32);
        }
    };

    let doc = match load(&cli.file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("wiretrail: {e}");
            std::process::exit(ExitCode::InvalidHar as i32);
        }
    };
    let cap = assemble(doc);

    match cli.command.unwrap_or(Command::Summary) {
        Command::Summary => {
            let result = compute_summary(&cap, &filter, cli.top);
            let has_findings = result.error_count > 0 || !result.top_duplicates.is_empty();
            if cli.json {
                let env = Envelope::new("summary", cap.meta.clone(), &result)
                    .with_next_commands(vec![
                        "duplicates".to_string(),
                        "errors".to_string(),
                        "slowest".to_string(),
                    ]);
                println!("{}", env.to_json());
            } else {
                print!("{}", render_summary_text(&result));
                println!("\nnext useful commands: duplicates · errors · slowest");
            }
            std::process::exit(if has_findings {
                ExitCode::Findings as i32
            } else {
                ExitCode::Clean as i32
            });
        }
    }
}

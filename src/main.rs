use clap::{Parser, Subcommand};
use har::analysis::endpoints::{compute_endpoints, render_endpoints_text};
use har::analysis::hosts::{compute_hosts, render_hosts_text};
use har::analysis::subsystems::{compute_subsystems, render_subsystems_text};
use har::analysis::summary::{compute_summary, render_summary_text};
use har::assemble::assemble;
use har::config::Config;
use har::filter::Filter;
use har::loader::load;
use har::model::CaptureMeta;
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

    /// Path to a wiretrail.yaml config (default: ./wiretrail.yaml if present).
    #[arg(long, global = true)]
    config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Executive summary of the capture (default).
    Summary,
    /// Per-host request/latency/byte/error breakdown.
    Hosts,
    /// Group hosts into named subsystems (vendor heuristics + config).
    Subsystems,
    /// Normalized endpoint inventory.
    Endpoints,
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
            let findings = result.error_count > 0 || !result.top_duplicates.is_empty();
            emit(
                cli.json,
                "summary",
                &cap.meta,
                &result,
                &render_summary_text(&result),
                &["duplicates", "errors", "slowest"],
            );
            exit(findings);
        }
        Command::Hosts => {
            let result = compute_hosts(&cap, &filter, cli.top);
            let findings = result.hosts.iter().any(|h| h.error_count > 0 || h.duplicate_count > 0);
            emit(
                cli.json,
                "hosts",
                &cap.meta,
                &result,
                &render_hosts_text(&result),
                &["subsystems", "endpoints", "errors"],
            );
            exit(findings);
        }
        Command::Subsystems => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let result = compute_subsystems(&cap, &filter, &config, cli.top);
            let findings = result.subsystems.iter().any(|s| s.error_count > 0 || s.duplicate_count > 0);
            emit(
                cli.json,
                "subsystems",
                &cap.meta,
                &result,
                &render_subsystems_text(&result),
                &["hosts", "endpoints", "duplicates"],
            );
            exit(findings);
        }
        Command::Endpoints => {
            let result = compute_endpoints(&cap, &filter, cli.top);
            let findings = result.endpoints.iter().any(|e| e.error_count > 0);
            emit(
                cli.json,
                "endpoints",
                &cap.meta,
                &result,
                &render_endpoints_text(&result),
                &["errors", "duplicates", "show-entry"],
            );
            exit(findings);
        }
    }
}

/// Print a result either as the stable JSON envelope or as terminal text.
fn emit<T: serde::Serialize>(
    json: bool,
    command: &'static str,
    meta: &CaptureMeta,
    result: &T,
    text: &str,
    next: &[&str],
) {
    if json {
        let env = Envelope::new(command, meta.clone(), result)
            .with_next_commands(next.iter().map(|s| s.to_string()).collect());
        println!("{}", env.to_json());
    } else {
        print!("{text}");
        println!("\nnext useful commands: {}", next.join(" · "));
    }
}

/// Exit 1 when findings exceed threshold, else 0.
fn exit(findings: bool) -> ! {
    std::process::exit(if findings {
        ExitCode::Findings as i32
    } else {
        ExitCode::Clean as i32
    });
}

use clap::{Parser, Subcommand};
use har::analysis::duplicates::{compute_duplicates, render_duplicates_text};
use har::analysis::endpoints::{compute_endpoints, render_endpoints_text};
use har::analysis::errors::{compute_errors, render_errors_text};
use har::analysis::hosts::{compute_hosts, render_hosts_text};
use har::analysis::redirects::{compute_redirects, render_redirects_text};
use har::analysis::retries::{compute_retries, render_retries_text};
use har::analysis::show_entry::{entry_detail, find_entry, render_entry_detail_text};
use har::analysis::slowest::{compute_slowest, render_slowest_text};
use har::analysis::subsystems::{compute_subsystems, render_subsystems_text};
use har::analysis::summary::{compute_summary, render_summary_text};
use har::analysis::timeline::{compute_timeline, render_timeline_text};
use har::analysis::transitions::{compute_transitions, render_transitions_text};
use har::analysis::curl::{compute_curl, entry_to_curl, render_curl_text, CurlResult};
use har::analysis::report::{compose_report, ReportResult};
use har::analysis::storms::{compute_storms, render_storms_text};
use har::analysis::pagination::{compute_pagination, render_pagination_text};
use har::analysis::rate_limit::{compute_rate_limit, render_rate_limit_text};
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

    /// Show raw secret values (auth headers, tokens, bodies) instead of redacting.
    #[arg(long, global = true)]
    unsafe_include_secrets: bool,
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
    /// Repeated requests (method + path + query fingerprint).
    Duplicates,
    /// Repeated requests that follow a failed attempt.
    Retries,
    /// 4xx/5xx/failed responses grouped by endpoint.
    Errors,
    /// Redirect responses, chains, and storms.
    Redirects,
    /// Top-N slowest requests with timing breakdown.
    Slowest,
    /// Status-code transition sequences (401->200, 429->429, ...).
    Transitions,
    /// Chronological per-request timeline.
    Timeline,
    /// Full redacted detail for one entry (by id `e000123` or index).
    ShowEntry {
        /// Entry id (e000123) or bare index.
        id: String,
    },
    /// Compose a dossier-style markdown report.
    Report,
    /// Sanitized curl replay commands (one entry, or all filtered).
    Curl {
        /// Optional entry id (e000123) or index; omit to emit all filtered entries.
        id: Option<String>,
    },
    /// Bursts of many calls to the same host or endpoint within a window.
    Storms {
        /// Window width in milliseconds.
        #[arg(long, default_value_t = 1000)]
        window_ms: u64,
        /// Minimum calls in the window to count as a storm.
        #[arg(long, default_value_t = 5)]
        min_count: usize,
    },
    /// Pagination loops and N+1 fan-out clusters.
    Pagination {
        /// Page count above which a sequence is flagged excessive.
        #[arg(long, default_value_t = 20)]
        max_pages: usize,
        /// Minimum fan-out to flag an N+1 cluster.
        #[arg(long = "fanout-min", default_value_t = 5)]
        fanout_min: usize,
        /// Window (ms) for N+1 clustering.
        #[arg(long, default_value_t = 2000)]
        window_ms: u64,
    },
    /// Rate-limit (429) events, Retry-After, and cooldown violations.
    RateLimit,
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
        Command::Duplicates => {
            let result = compute_duplicates(&cap, &filter, cli.top);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "duplicates",
                &cap.meta,
                &result,
                &render_duplicates_text(&result),
                &["retries", "errors", "show-entry"],
            );
            exit(findings);
        }
        Command::Retries => {
            let result = compute_retries(&cap, &filter, cli.top);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "retries",
                &cap.meta,
                &result,
                &render_retries_text(&result),
                &["errors", "transitions", "show-entry"],
            );
            exit(findings);
        }
        Command::Errors => {
            let result = compute_errors(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "errors",
                &cap.meta,
                &result,
                &render_errors_text(&result),
                &["transitions", "redirects", "show-entry"],
            );
            exit(findings);
        }
        Command::Redirects => {
            let result = compute_redirects(&cap, &filter, cli.top);
            let findings = result.groups.iter().any(|g| g.is_storm);
            emit(
                cli.json,
                "redirects",
                &cap.meta,
                &result,
                &render_redirects_text(&result),
                &["timeline", "errors", "show-entry"],
            );
            exit(findings);
        }
        Command::Slowest => {
            let result = compute_slowest(&cap, &filter, cli.top);
            emit(
                cli.json,
                "slowest",
                &cap.meta,
                &result,
                &render_slowest_text(&result),
                &["timeline", "hosts", "show-entry"],
            );
            exit(false);
        }
        Command::Transitions => {
            let result = compute_transitions(&cap, &filter, cli.top);
            let findings = !result.transitions.is_empty();
            emit(
                cli.json,
                "transitions",
                &cap.meta,
                &result,
                &render_transitions_text(&result),
                &["errors", "retries", "show-entry"],
            );
            exit(findings);
        }
        Command::Timeline => {
            let result = compute_timeline(&cap, &filter, cli.top);
            emit(
                cli.json,
                "timeline",
                &cap.meta,
                &result,
                &render_timeline_text(&result),
                &["slowest", "duplicates", "show-entry"],
            );
            exit(false);
        }
        Command::ShowEntry { id } => {
            let Some(e) = find_entry(&cap, &id) else {
                eprintln!("wiretrail: no entry with id or index '{id}'");
                std::process::exit(ExitCode::InvalidHar as i32);
            };
            let detail = entry_detail(e, cli.unsafe_include_secrets);
            emit(
                cli.json,
                "show-entry",
                &cap.meta,
                &detail,
                &render_entry_detail_text(&detail),
                &["timeline", "duplicates", "errors"],
            );
            exit(false);
        }
        Command::Report => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let markdown = compose_report(&cap, &filter, &config, cli.top, cli.unsafe_include_secrets);
            if cli.json {
                let result = ReportResult { markdown };
                let env = Envelope::new("report", cap.meta.clone(), &result);
                println!("{}", env.to_json());
            } else {
                print!("{markdown}");
            }
            exit(false);
        }
        Command::Curl { id } => {
            let result = match id {
                Some(id) => {
                    let Some(e) = find_entry(&cap, &id) else {
                        eprintln!("wiretrail: no entry with id or index '{id}'");
                        std::process::exit(ExitCode::InvalidHar as i32);
                    };
                    CurlResult {
                        commands: vec![entry_to_curl(e, cli.unsafe_include_secrets)],
                    }
                }
                None => compute_curl(&cap, &filter, cli.top, cli.unsafe_include_secrets),
            };
            emit(
                cli.json,
                "curl",
                &cap.meta,
                &result,
                &render_curl_text(&result),
                &["show-entry", "errors", "duplicates"],
            );
            exit(false);
        }
        Command::Storms { window_ms, min_count } => {
            let result = compute_storms(&cap, &filter, window_ms, min_count, cli.top);
            let findings = !result.storms.is_empty();
            emit(
                cli.json,
                "storms",
                &cap.meta,
                &result,
                &render_storms_text(&result),
                &["pagination", "duplicates", "timeline"],
            );
            exit(findings);
        }
        Command::Pagination { max_pages, fanout_min, window_ms } => {
            let result = compute_pagination(&cap, &filter, max_pages, fanout_min, window_ms, cli.top);
            let findings = !result.pages.is_empty() || !result.nplus1.is_empty();
            emit(
                cli.json,
                "pagination",
                &cap.meta,
                &result,
                &render_pagination_text(&result),
                &["storms", "duplicates", "endpoints"],
            );
            exit(findings);
        }
        Command::RateLimit => {
            let result = compute_rate_limit(&cap, &filter, cli.top);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "rate-limit",
                &cap.meta,
                &result,
                &render_rate_limit_text(&result),
                &["errors", "retries", "transitions"],
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

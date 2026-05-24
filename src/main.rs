use clap::{Parser, Subcommand, ValueEnum};
use har::analysis::auth::{compute_auth, render_auth_text};
use har::analysis::cascade::{compute_cascade, render_cascade_text};
use har::analysis::checks::{compute_checks, render_checks_text};
use har::analysis::compare::{compute_compare, render_compare_text, sev_rank};
use har::analysis::curl::{CurlResult, compute_curl, entry_to_curl, render_curl_text};
use har::analysis::diagnose::{compute_diagnose, render_diagnose_text};
use har::analysis::diff::{compute_diff, render_diff_text};
use har::analysis::duplicates::{compute_duplicates, render_duplicates_text};
use har::analysis::endpoints::{compute_endpoints, render_endpoints_text};
use har::analysis::errors::{compute_errors, render_errors_text};
use har::analysis::export::{export_records, render_csv, render_ndjson};
use har::analysis::extract::{Target, compute_extract, render_extract_text};
use har::analysis::handoff::{compute_handoff, render_handoff_text};
use har::analysis::hosts::{compute_hosts, render_hosts_text};
use har::analysis::jwt::{compute_jwt, render_jwt_text};
use har::analysis::pagination::{compute_pagination, render_pagination_text};
use har::analysis::rate_limit::{compute_rate_limit, render_rate_limit_text};
use har::analysis::redirects::{compute_redirects, render_redirects_text};
use har::analysis::report::{ReportResult, compose_report};
use har::analysis::retries::{compute_retries, render_retries_text};
use har::analysis::rules::{compute_rules, render_rules_text};
use har::analysis::search::{compute_search, render_search_text};
use har::analysis::show_entry::{entry_detail, find_entry, render_entry_detail_text};
use har::analysis::slowest::{compute_slowest, render_slowest_text};
use har::analysis::startup::{compute_startup, render_startup_text};
use har::analysis::storms::{compute_storms, render_storms_text};
use har::analysis::subsystems::{compute_subsystems, render_subsystems_text};
use har::analysis::summary::{compute_summary, render_summary_text};
use har::analysis::timeline::{compute_timeline, render_timeline_text};
use har::analysis::transitions::{compute_transitions, render_transitions_text};
use har::analysis::validate::{compute_validate, render_validate_text};
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TargetArg {
    Req,
    Resp,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormatArg {
    Ndjson,
    Csv,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SeverityArg {
    Critical,
    High,
    Medium,
    Low,
}

impl SeverityArg {
    fn as_str(self) -> &'static str {
        match self {
            SeverityArg::Critical => "critical",
            SeverityArg::High => "high",
            SeverityArg::Medium => "medium",
            SeverityArg::Low => "low",
        }
    }
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
    /// Find and decode JWTs (redacted: no signature, hashed sub).
    Jwt,
    /// Auth failures (401/403), inconsistent auth, and token-refresh flows.
    Auth,
    /// Backend trace-handoff blocks for failed + slowest requests.
    Handoff,
    /// What varies across repeated calls to the same endpoint.
    Diff,
    /// Built-in checks: required headers (config) + content-type mismatch.
    Checks,
    /// Ranked root-cause findings synthesized from all analyses.
    Diagnose,
    /// Boot/startup profile: concurrency, critical path, slow dependencies.
    Startup {
        /// Boot window in milliseconds (0 = whole capture).
        #[arg(long, default_value_t = 30000)]
        window_ms: u64,
    },
    /// Earliest failure and downstream failure cascades.
    Cascade {
        /// Window (ms) to attribute downstream failures to a trigger.
        #[arg(long, default_value_t = 5000)]
        window_ms: u64,
        /// Minimum downstream failures to report a cascade.
        #[arg(long = "min-downstream", default_value_t = 3)]
        min_downstream: usize,
    },
    /// Capture-quality and analysis-sufficiency report.
    Validate,
    /// Search request/response bodies (redaction-safe).
    Search {
        /// Pattern to search for.
        pattern: String,
        /// Treat the pattern as a regular expression.
        #[arg(long)]
        regex: bool,
        /// Case-insensitive match.
        #[arg(long = "ignore-case")]
        ignore_case: bool,
    },
    /// Extract a JSON path from request/response bodies.
    Extract {
        /// JSON path, e.g. `$.errors[0].code`.
        path: String,
        /// Which body to query.
        #[arg(long, value_enum, default_value_t = TargetArg::Resp)]
        target: TargetArg,
    },
    /// Flatten entries to NDJSON or CSV.
    Export {
        /// Output format.
        #[arg(long, value_enum, default_value_t = ExportFormatArg::Ndjson)]
        format: ExportFormatArg,
    },
    /// Compare this capture against a baseline HAR (regression diff).
    Compare {
        /// Path to the baseline HAR to diff against.
        baseline: PathBuf,
        /// Exit non-zero only when max severity reaches this level (CI gate).
        #[arg(long = "fail-on", value_enum)]
        fail_on: Option<SeverityArg>,
    },
    /// Evaluate config rules and built-in rule packs against the capture.
    Rules {
        /// Built-in packs to apply, e.g. `--pack auth,security`.
        #[arg(long = "pack", value_delimiter = ',')]
        pack: Vec<String>,
    },
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
            let findings = result
                .hosts
                .iter()
                .any(|h| h.error_count > 0 || h.duplicate_count > 0);
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
            let findings = result
                .subsystems
                .iter()
                .any(|s| s.error_count > 0 || s.duplicate_count > 0);
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
            let markdown =
                compose_report(&cap, &filter, &config, cli.top, cli.unsafe_include_secrets);
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
        Command::Storms {
            window_ms,
            min_count,
        } => {
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
        Command::Pagination {
            max_pages,
            fanout_min,
            window_ms,
        } => {
            let result =
                compute_pagination(&cap, &filter, max_pages, fanout_min, window_ms, cli.top);
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
        Command::Jwt => {
            let result = compute_jwt(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = result
                .tokens
                .iter()
                .any(|t| t.summary.expired == Some(true));
            emit(
                cli.json,
                "jwt",
                &cap.meta,
                &result,
                &render_jwt_text(&result),
                &["auth", "show-entry", "errors"],
            );
            exit(findings);
        }
        Command::Auth => {
            let result = compute_auth(&cap, &filter, cli.top);
            let findings = !result.failures.is_empty()
                || !result.missing_auth_hosts.is_empty()
                || result
                    .refreshes
                    .iter()
                    .any(|r| !r.success || r.old_token_reused || r.concurrent);
            emit(
                cli.json,
                "auth",
                &cap.meta,
                &result,
                &render_auth_text(&result),
                &["jwt", "transitions", "errors"],
            );
            exit(findings);
        }
        Command::Handoff => {
            let result = compute_handoff(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = !result.items.is_empty();
            emit(
                cli.json,
                "handoff",
                &cap.meta,
                &result,
                &render_handoff_text(&result),
                &["errors", "slowest", "curl"],
            );
            exit(findings);
        }
        Command::Diff => {
            let result = compute_diff(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = result.groups.iter().any(|g| {
                g.body_verdict == "meaningful"
                    || g.varying_header_names.iter().any(|n| n == "authorization")
            });
            emit(
                cli.json,
                "diff",
                &cap.meta,
                &result,
                &render_diff_text(&result),
                &["duplicates", "show-entry", "endpoints"],
            );
            exit(findings);
        }
        Command::Checks => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let result = compute_checks(&cap, &filter, &config, cli.top);
            let findings = !result.findings.is_empty();
            emit(
                cli.json,
                "checks",
                &cap.meta,
                &result,
                &render_checks_text(&result),
                &["errors", "show-entry", "endpoints"],
            );
            exit(findings);
        }
        Command::Diagnose => {
            let result = compute_diagnose(&cap, &filter, cli.top);
            let findings = !result.findings.is_empty();
            emit(
                cli.json,
                "diagnose",
                &cap.meta,
                &result,
                &render_diagnose_text(&result),
                &["errors", "auth", "duplicates"],
            );
            exit(findings);
        }
        Command::Startup { window_ms } => {
            let result = compute_startup(&cap, &filter, window_ms, cli.top);
            emit(
                cli.json,
                "startup",
                &cap.meta,
                &result,
                &render_startup_text(&result),
                &["slowest", "timeline", "storms"],
            );
            exit(false);
        }
        Command::Cascade {
            window_ms,
            min_downstream,
        } => {
            let result = compute_cascade(&cap, &filter, window_ms, min_downstream, cli.top);
            let findings = result.first_failure.is_some() || !result.cascades.is_empty();
            emit(
                cli.json,
                "cascade",
                &cap.meta,
                &result,
                &render_cascade_text(&result),
                &["errors", "transitions", "show-entry"],
            );
            exit(findings);
        }
        Command::Validate => {
            let result = compute_validate(&cap);
            let findings = !result.anomalies.is_empty();
            emit(
                cli.json,
                "validate",
                &cap.meta,
                &result,
                &render_validate_text(&result),
                &["summary", "diagnose", "errors"],
            );
            exit(findings);
        }
        Command::Search {
            pattern,
            regex,
            ignore_case,
        } => {
            match compute_search(
                &cap,
                &filter,
                &pattern,
                regex,
                ignore_case,
                cli.top,
                cli.unsafe_include_secrets,
            ) {
                Ok(result) => {
                    emit(
                        cli.json,
                        "search",
                        &cap.meta,
                        &result,
                        &render_search_text(&result),
                        &["show-entry", "extract", "errors"],
                    );
                    exit(false);
                }
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            }
        }
        Command::Extract { path, target } => {
            let target = match target {
                TargetArg::Req => Target::Req,
                TargetArg::Resp => Target::Resp,
            };
            let result = compute_extract(
                &cap,
                &filter,
                &path,
                target,
                cli.top,
                cli.unsafe_include_secrets,
            );
            emit(
                cli.json,
                "extract",
                &cap.meta,
                &result,
                &render_extract_text(&result),
                &["search", "show-entry", "errors"],
            );
            exit(false);
        }
        Command::Export { format } => {
            let records = export_records(&cap, &filter);
            let out = match format {
                ExportFormatArg::Ndjson => render_ndjson(&records),
                ExportFormatArg::Csv => render_csv(&records),
            };
            println!("{out}");
            exit(false);
        }
        Command::Compare { baseline, fail_on } => {
            let base_doc = match load(&baseline) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("wiretrail: baseline: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let base = assemble(base_doc);
            let result = compute_compare(&cap, &base, &filter, cli.top);
            emit(
                cli.json,
                "compare",
                &cap.meta,
                &result,
                &render_compare_text(&result),
                &["diagnose", "errors", "slowest"],
            );
            let any = result.max_severity != "none";
            let findings = match fail_on {
                Some(t) => any && sev_rank(&result.max_severity) >= sev_rank(t.as_str()),
                None => any,
            };
            exit(findings);
        }
        Command::Rules { pack } => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let result = compute_rules(&cap, &filter, &config, &pack, cli.top);
            let findings = !result.findings.is_empty();
            emit(
                cli.json,
                "rules",
                &cap.meta,
                &result,
                &render_rules_text(&result),
                &["checks", "errors", "diagnose"],
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

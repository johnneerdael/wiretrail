# Command line

```text
wiretrail <FILE> [COMMAND] [OPTIONS]
```

`<FILE>` is a HAR file. The command comes **after** the file. With no command, `summary` runs.

## Global options

These are accepted by every command, before or after the command name. A few commands ignore some of them: `validate` ignores `--filter` and `--top`, `show-entry` and `curl <id>` ignore `--filter` and `--top`, and `export` ignores `--top` and `--json`.

| Option | Default | Effect |
|---|---|---|
| `--json` | off | Print the result as the [JSON envelope](json-schemas.md) instead of text. |
| `--top <N>` | `10` | Bound the length of each list. It also bounds the ranked recommendations. `export`, `validate`, `show-entry` and `curl <id>` ignore it. |
| `--filter "<clauses>"` | none | Limit the command to the requests that match. Repeatable. `validate`, `show-entry` and `curl <id>` ignore it. See [Filter entries](../analyze/filters.md). |
| `--config <path>` | `./wiretrail.yaml` if present | A [configuration file](../compare/config-file.md). Only `subsystems`, `report`, `checks` and `rules` read it. |
| `--unsafe-include-secrets` | off | Turn masking off. See [How redaction works](../concepts/redaction.md). |
| `-h`, `--help` | | Print help. |
| `-V`, `--version` | | Print the version. |

## Commands

| Command | Arguments | Options | Page |
|---|---|---|---|
| `summary` | | | [Summary](../analyze/summary.md) |
| `auto` | | `--all`, `--min-severity <level>` | [Auto](../analyze/auto.md) |
| `diagnose` | | | [Diagnose](../analyze/diagnose.md) |
| `validate` | | | [Validate a capture](../analyze/validate.md) |
| `hosts` | | | [Hosts, subsystems and endpoints](../analyze/inventory.md#hosts) |
| `subsystems` | | | [Hosts, subsystems and endpoints](../analyze/inventory.md#subsystems) |
| `endpoints` | | | [Hosts, subsystems and endpoints](../analyze/inventory.md#endpoints) |
| `timeline` | | | [Timeline](../analyze/timeline.md) |
| `duplicates` | | | [Duplicates](../repeats/duplicates.md) |
| `retries` | | | [Retries](../repeats/retries.md) |
| `storms` | | `--window-ms <ms>`, `--min-count <n>` | [Storms](../repeats/storms.md) |
| `pagination` | | `--max-pages <n>`, `--fanout-min <n>`, `--window-ms <ms>` | [Pagination and N+1](../repeats/pagination.md) |
| `rate-limit` | | | [Rate limits](../repeats/rate-limits.md) |
| `diff` | | | [What varies between calls](../repeats/diff.md) |
| `errors` | | | [Errors](../failures/errors.md) |
| `redirects` | | | [Redirects](../failures/redirects.md) |
| `transitions` | | | [Status transitions](../failures/transitions.md) |
| `slowest` | | | [Slowest calls](../failures/slowest.md) |
| `startup` | | `--window-ms <ms>` | [Startup profile](../failures/startup.md) |
| `cascade` | | `--window-ms <ms>`, `--min-downstream <n>` | [Cascade](../failures/cascade.md) |
| `auth` | | | [Auth failures and refresh](../failures/auth.md) |
| `jwt` | | | [JWTs](../failures/jwt.md) |
| `show-entry` | `<id>` | | [Show an entry](../inspect/show-entry.md) |
| `search` | `<pattern>` | `--regex`, `--ignore-case` | [Search bodies](../inspect/search.md) |
| `extract` | `<path>` | `--target req\|resp` | [Extract a field](../inspect/extract.md) |
| `export` | | `--format ndjson\|csv` | [Export](../inspect/export.md) |
| `report` | | | [Report](../inspect/report.md) |
| `curl` | `[id]` | | [curl](../inspect/curl.md) |
| `handoff` | | | [Handoff](../inspect/handoff.md) |
| `compare` | `<baseline>` | `--fail-on <level>` | [Compare against a baseline](../compare/compare.md) |
| `rules` | | `--pack <a,b>` | [Rules and rule packs](../compare/rules.md) |
| `checks` | | | [Built-in checks](../compare/checks.md) |

`--min-severity` and `--fail-on` take `critical`, `high`, `medium` or `low`. `--format` takes `ndjson` or `csv`, and `--target` takes `req` or `resp`.

## Option defaults

| Option | Command | Default |
|---|---|---|
| `--window-ms` | `storms` | `1000` |
| `--min-count` | `storms` | `5` |
| `--max-pages` | `pagination` | `20` |
| `--fanout-min` | `pagination` | `5` |
| `--window-ms` | `pagination` | `2000` |
| `--window-ms` | `startup` | `30000` (`0` means the whole capture) |
| `--window-ms` | `cascade` | `5000` |
| `--min-downstream` | `cascade` | `3` |
| `--target` | `extract` | `resp` |
| `--format` | `export` | `ndjson` |
| `--min-severity` | `auto` | `medium` |

## The help text

??? example "`wiretrail --help`"

    <!-- output: help-main.txt cmd -->
    ```console
    $ wiretrail --help
    Fast HAR analyzer CLI

    Usage: wiretrail [OPTIONS] <FILE> [COMMAND]

    Commands:
      summary      Executive summary of the capture (default)
      hosts        Per-host request/latency/byte/error breakdown
      subsystems   Group hosts into named subsystems (vendor heuristics + config)
      endpoints    Normalized endpoint inventory
      duplicates   Repeated requests (method + path + query fingerprint)
      retries      Repeated requests that follow a failed attempt
      errors       4xx/5xx/failed responses grouped by endpoint
      redirects    Redirect responses, chains, and storms
      slowest      Top-N slowest requests with timing breakdown
      transitions  Status-code transition sequences (401->200, 429->429, ...)
      timeline     Chronological per-request timeline
      show-entry   Full redacted detail for one entry (by id `e000123` or index)
      report       Compose a dossier-style markdown report
      curl         Sanitized curl replay commands (one entry, or all filtered)
      storms       Bursts of many calls to the same host or endpoint within a window
      pagination   Pagination loops and N+1 fan-out clusters
      rate-limit   Rate-limit (429) events, Retry-After, and cooldown violations
      jwt          Find and decode JWTs (redacted: no signature, hashed sub)
      auth         Auth failures (401/403), inconsistent auth, and token-refresh flows
      handoff      Backend trace-handoff blocks for failed + slowest requests
      diff         What varies across repeated calls to the same endpoint
      checks       Built-in checks: required headers (config) + content-type mismatch
      diagnose     Ranked root-cause findings synthesized from all analyses
      startup      Boot/startup profile: concurrency, critical path, slow dependencies
      cascade      Earliest failure and downstream failure cascades
      validate     Capture-quality and analysis-sufficiency report
      search       Search request/response bodies (redaction-safe)
      extract      Extract a JSON path from request/response bodies
      export       Flatten entries to NDJSON or CSV
      compare      Compare this capture against a baseline HAR (regression diff)
      rules        Evaluate config rules and built-in rule packs against the capture
      auto         Smart one-shot: summary + auto-drill the top recommendations inline
      help         Print this message or the help of the given subcommand(s)

    Arguments:
      <FILE>  Path to the HAR file

    Options:
          --json                    Emit machine-readable JSON
          --top <TOP>               Max items per list (top-N) [default: 10]
          --filter <FILTER>         Filter clauses, e.g. --filter "host:api.foo.com status:>=400"
          --config <CONFIG>         Path to a wiretrail.yaml config (default: ./wiretrail.yaml if present)
          --unsafe-include-secrets  Show raw secret values (auth headers, tokens, bodies) instead of redacting
      -h, --help                    Print help
      -V, --version                 Print version
    ```

??? example "`wiretrail capture.har storms --help`"

    <!-- output: help-storms.txt cmd -->
    ```console
    $ wiretrail checkout.har storms --help
    Bursts of many calls to the same host or endpoint within a window

    Usage: wiretrail <FILE> storms [OPTIONS]

    Options:
          --window-ms <WINDOW_MS>   Window width in milliseconds [default: 1000]
          --min-count <MIN_COUNT>   Minimum calls in the window to count as a storm [default: 5]
          --json                    Emit machine-readable JSON
          --top <TOP>               Max items per list (top-N) [default: 10]
          --filter <FILTER>         Filter clauses, e.g. --filter "host:api.foo.com status:>=400"
          --config <CONFIG>         Path to a wiretrail.yaml config (default: ./wiretrail.yaml if present)
          --unsafe-include-secrets  Show raw secret values (auth headers, tokens, bodies) instead of redacting
      -h, --help                    Print help
    ```

??? example "`wiretrail capture.har compare --help`"

    <!-- output: help-compare.txt cmd -->
    ```console
    $ wiretrail checkout.har compare --help
    Compare this capture against a baseline HAR (regression diff)

    Usage: wiretrail <FILE> compare [OPTIONS] <BASELINE>

    Arguments:
      <BASELINE>  Path to the baseline HAR to diff against

    Options:
          --fail-on <FAIL_ON>       Exit non-zero only when max severity reaches this level (CI gate) [possible values: critical, high, medium, low]
          --json                    Emit machine-readable JSON
          --top <TOP>               Max items per list (top-N) [default: 10]
          --filter <FILTER>         Filter clauses, e.g. --filter "host:api.foo.com status:>=400"
          --config <CONFIG>         Path to a wiretrail.yaml config (default: ./wiretrail.yaml if present)
          --unsafe-include-secrets  Show raw secret values (auth headers, tokens, bodies) instead of redacting
      -h, --help                    Print help
    ```

??? example "`wiretrail capture.har auto --help`"

    <!-- output: help-auto.txt cmd -->
    ```console
    $ wiretrail checkout.har auto --help
    Smart one-shot: summary + auto-drill the top recommendations inline

    Usage: wiretrail <FILE> auto [OPTIONS]

    Options:
          --all                          Drill into every triggered recommendation, including LOW
          --min-severity <MIN_SEVERITY>  Only drill recommendations at or above this severity (default: medium) [possible values: critical, high, medium, low]
          --json                         Emit machine-readable JSON
          --top <TOP>                    Max items per list (top-N) [default: 10]
          --filter <FILTER>              Filter clauses, e.g. --filter "host:api.foo.com status:>=400"
          --config <CONFIG>              Path to a wiretrail.yaml config (default: ./wiretrail.yaml if present)
          --unsafe-include-secrets       Show raw secret values (auth headers, tokens, bodies) instead of redacting
      -h, --help                         Print help
    ```

Every command prints its own help with `wiretrail capture.har <command> --help`.

## Related

- [Errors and exit codes](errors-and-exit-codes.md) for what each command returns.
- [Choose the right command](../getting-started/choose-a-command.md) maps a question to a command.

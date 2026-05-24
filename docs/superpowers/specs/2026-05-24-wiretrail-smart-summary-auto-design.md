# wiretrail — Smart Summary & `auto` Command Design

Date: 2026-05-24
Status: Approved (brainstorming complete; ready for implementation planning)
Author: John Neerdael

## Summary

The next improvement is depth, not breadth. Today `summary` ends with a **static**
`hints` block and a **fixed** `next_commands` footer (`duplicates · errors · slowest`)
regardless of what the capture actually contains. This redesign makes the "what do I
run next?" guidance **evidence-backed and ranked**, and adds a single command that
acts on it.

Two deliverables, one shared core:

1. A shared **recommender** that ranks actionable recommendations (`command` + `filter`
   + `severity` + `why`) by running the existing analyses. This is `diagnose`'s
   composition generalized.
2. `summary` surfaces those recommendations; a new **`auto`** command executes the
   top ones and inlines their full scoped output — a single command for detailed,
   smart HAR analysis.

`diagnose` is refactored to consume the same core so the two never drift. New command
count: **32**.

## Locked decisions (from brainstorming)

1. **Shared recommender core.** One module produces ranked recommendations;
   `summary` prints them, `auto` runs them, `diagnose` reuses the core. No command
   is removed.
2. **Rank by real severity.** The recommender runs the relevant `compute_*` functions
   and ranks by actual magnitude (not cheap presence signals). `summary` becomes a
   fuller pass — still sub-second even on 143 MB.
3. **`auto` = triggered-only, full inline.** Run only the commands the recommender
   flagged, each scoped by its recommended `--filter`, inlining each command's full
   normal output. Nothing runs for a signal that isn't present.
4. **`summary` is additive.** Keep the existing stats blocks and `hints`; append a
   ranked "recommended next steps" section. `summary`'s `next_commands` footer is
   derived from the top recommendations (the one non-additive change, central to the
   ask). Other commands keep their curated static footers.
5. **`auto` severity gate.** Default drills HIGH + MEDIUM (and CRITICAL, above the
   floor); lists lower ones as one-line suggestions. `--all` (≡ `--min-severity low`)
   drills everything triggered; `--min-severity high` narrows to high/critical.
   `summary` always *lists* all recommendations (it never runs them).

## Architecture

```
src/recommender.rs       # NEW (lib): recommend(cap, filter, top) -> Vec<Recommendation>
src/analysis/diagnose.rs # MODIFY: render over recommend() (output unchanged)
src/analysis/summary.rs  # MODIFY: add recommendations field + render section + dynamic footer
src/main.rs              # MODIFY: `auto` subcommand + drill-down executor + JSON nesting
tests/cli_auto.rs        # NEW: integration tests
```

The recommender is **pure and lib-level** (testable in isolation). The drill-down
**executor lives in `main.rs`**, where every `compute_*`/`render_*` is already
imported and the subcommand dispatch already exists; `auto` reuses that surface
rather than duplicating analysis wiring into the library.

### `Recommendation` (in `src/recommender.rs`)

```rust
#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub severity: String,        // "critical" | "high" | "medium" | "low"
    pub kind: String,            // "retry-exhaustion", "5xx-cluster", ...
    pub why: String,             // human one-liner (today's diagnose title + detail)
    pub evidence_ids: Vec<String>,
    pub command: String,         // drill-down subcommand: "retries", "errors", ...
    pub filter: Option<String>,  // scoping filter expression, e.g. "host:api.ntsk.cloud"
}

pub fn recommend(cap: &Capture, filter: &Filter, top: usize) -> Vec<Recommendation>;
pub fn sev_rank(s: &str) -> u8; // critical=3, high=2, medium=1, _=0 (shared)
```

`recommend()` is the current body of `compute_diagnose`, with each finding's
`suggested_command` string decomposed into structured `command` + `filter`.
Recommendation kind → drill-down command mapping:

| kind | command | filter |
|---|---|---|
| `5xx-cluster` | `errors` | `host:<host>` |
| `4xx` | `errors` | — |
| `token-refresh-race` | `auth` | — |
| `auth-failures` | `auth` | — |
| `rate-limit-no-backoff` | `rate-limit` | — |
| `retry-exhaustion` | `retries` | `host:<host>` (when available) |
| `request-storm` | `storms` | — |
| `wasteful-duplicates` | `diff` | `host:<host>` (when available) |
| `redirect-storm` | `redirects` | — |
| `slow-backend` | `slowest` | — |

Findings are sorted by `sev_rank` desc, then evidence count desc, then kind
(unchanged from today's `diagnose` ordering) and truncated to `top`.

### `diagnose` refactor

`compute_diagnose` becomes a thin adapter: call `recommend()`, map each
`Recommendation` back to the existing `Diagnosis { severity, kind, title, detail,
evidence_ids, suggested_command }` where `suggested_command = command` + optional
` --filter "<filter>"`. `render_diagnose_text` is unchanged. The existing `diagnose`
unit + CLI tests are the parity guard — they must stay green untouched.

### `summary` integration

`SummaryResult` gains `pub recommendations: Vec<Recommendation>`. `compute_summary`
calls `recommend(cap, filter, top)` and stores the result. The existing fields
(`hints`, stats, `top_*`) are untouched.

`render_summary_text` appends, after `hints`:

```
recommended next steps:
  [HIGH] retries --filter "host:api.ntsk.cloud"
         8x 500 on /ratings/bulk, backoff exhausted
  [MED]  diff --filter "host:...supabase.co"
         27x identical POST (likely redundant)
  [LOW]  slowest
         one 2.2s TTFB call
```

In `main.rs`, the `summary` dispatch derives the `next_commands` footer from the top
recommendations' `command` values (deduped, order-preserved) instead of the static
`["duplicates", "errors", "slowest"]`. When there are no recommendations it falls
back to the current static list so the footer is never empty.

### `auto` command (in `main.rs`)

CLI:

```
wiretrail <file> auto [--all] [--min-severity <critical|high|medium|low>]
```

- `--min-severity` default = `medium`. `--all` sets it to `low`. Mutually
  informative; if both given, `--all` wins (documented).
- Honors global `--filter`, `--top`, `--config`, `--unsafe-include-secrets`, `--json`.

Execution:

1. `let recs = recommend(&cap, &filter, cli.top);`
2. Print the summary block (reuse `compute_summary` + `render_summary_text`, which now
   includes the recommendations section).
3. Partition `recs` into *drilled* (`sev_rank(r.severity) >= floor`) and *not drilled*.
4. For each drilled rec (severity order): compose `combined = global_filter_clauses ∪
   rec.filter`, build a `Filter`, dispatch `rec.command` to its `compute_+render_`,
   and inline the output beneath a header + the reproducing `$ wiretrail …` line.
5. List *not drilled* recs as one-liners with their run command.

Drill-down executor — a bounded `match rec.command.as_str()` over only the commands
recommendations can emit (`errors`, `auth`, `rate-limit`, `retries`, `storms`,
`diff`, `redirects`, `slowest`), each producing either rendered text or a
`serde_json::Value` (via `serde_json::to_value(compute_X(...))`). An unknown command
is skipped defensively (cannot happen given the fixed mapping, but no panic).

Filter composition: `Filter::parse` accepts `&[String]`; combine by concatenating the
user's `--filter` clause vector with the rec's single filter clause (logical AND), so
`auto --filter "host:api.x"` scopes the entire run.

### `auto` output

**Text (default):**

```
== wiretrail auto ==
<full summary block, including recommended next steps>

────────────────────────────────────────
[HIGH] retries — 8x 500 on /ratings/bulk, backoff exhausted
$ wiretrail capture.har retries --filter "host:api.ntsk.cloud"
<full scoped retries output>

[MED] diff — 27x identical POST (likely redundant)
$ wiretrail capture.har diff --filter "host:...supabase.co"
<full scoped diff output>

not drilled (below threshold):
  [LOW] slowest — one 2.2s TTFB call   (run: wiretrail capture.har slowest)
```

**JSON (`--json`):** one envelope, `command: "auto"`:

```json
{ "tool": "wiretrail", "schema_version": 1, "command": "auto",
  "capture": { ... },
  "result": {
    "summary": { ...SummaryResult... },
    "drilldowns": [
      { "severity": "high", "kind": "retry-exhaustion",
        "command": "retries", "filter": "host:api.ntsk.cloud",
        "why": "...", "evidence_ids": ["e000112"],
        "result": { ...RetriesResult as Value... } }
    ],
    "not_drilled": [
      { "severity": "low", "kind": "slow-backend", "command": "slowest",
        "filter": null, "why": "..." }
    ]
  },
  "next_commands": [ ...derived... ] }
```

`auto`'s `result` is assembled as a `serde_json::Value` (since drill-down sub-results
are heterogeneous) and emitted through the existing `Envelope`.

## Error handling & exit codes

- `auto` exits `1` when `recommend()` returns any recommendation (findings present),
  `0` when empty — consistent with the other findings-based commands. Exit `2` only
  on an invalid/unreadable HAR (unchanged).
- A drill-down that legitimately finds nothing prints its normal empty section; it is
  not an error.
- Redaction is unchanged: each drill-down runs through its own redactor;
  `--unsafe-include-secrets` propagates into the executor (used by `errors`, `diff`,
  etc.).

## Testing

- **`recommender` unit tests:** a 5xx cluster + old-token-reuse refresh yields HIGH
  recommendations sorted first, each with the correct `command` and a populated
  `filter` where the mapping specifies one; a clean capture yields `[]`.
- **`diagnose` parity:** the existing `diagnose` unit + CLI tests remain unchanged and
  green after the refactor — this is the regression guard against output drift.
- **`summary` unit test:** `compute_summary` populates `recommendations`; a capture
  with errors produces a non-empty list; rendered text contains
  "recommended next steps".
- **`tests/cli_auto.rs`:** `auto --json` emits `"command": "auto"` with
  `result.summary` and `result.drilldowns`; on `someapi123.har`, `auto` inlines at
  least one drill-down (its `== wiretrail <cmd> ==` header appears) plus the
  reproducing `$ wiretrail …` line; `--min-severity high` yields ≤ default
  drill-downs and `--all` yields ≥ default; a clean fixture exits 0 and drills
  nothing.
- **Real-HAR confidence check** (both 11 MB and 143 MB): `auto` surfaces the ntsk
  5xx/retry cluster + Supabase auth story inline, lists LOW items without drilling,
  and a leak-scan of the full `auto` output confirms the torbox/premiumize keys stay
  redacted.

## Non-goals / deferred

- Making **all 31 commands'** `next_commands` footers dynamic — only `summary` (and
  the implicit `auto`) use the recommender; other commands keep curated static
  footers.
- New recommendation kinds beyond `diagnose`'s current set (pagination/N+1, jwt-expiry,
  cascade) — the recommender is extensible, but this redesign starts from the proven
  `diagnose` finding set to guarantee parity. Adding kinds is a follow-up.
- Caching analysis results between the recommender pass and the `auto` drill-down
  re-runs — analyses are sub-second on parsed data, so `auto` re-running the chosen
  commands is acceptable (YAGNI).
- `report` is unchanged (it remains the static markdown dossier).

## Rollout

One implementation plan, ~5 tasks:

1. Extract `recommender` from `diagnose`; refactor `diagnose` to render over it; prove
   parity (existing diagnose tests green).
2. Wire `recommendations` into `summary` (field + render section + dynamic footer).
3. `auto` subcommand + drill-down executor + filter composition (text output).
4. `auto` `--json` nesting + `--all`/`--min-severity` flags + exit codes.
5. Integration tests (`tests/cli_auto.rs`) + real-HAR confidence check.

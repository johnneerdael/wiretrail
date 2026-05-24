---
description: Analyse a HAR network capture with wiretrail — runs one-shot smart analysis (`auto`) and reports the findings.
argument-hint: <path-to.har> [focus, e.g. errors|auth|duplicates]
---

Analyse the HAR capture at: **$ARGUMENTS**

Follow the `analysing-har` skill. Concretely:

1. **Ensure wiretrail is installed** — `command -v wiretrail >/dev/null && wiretrail --version || cargo install wiretrail`.
2. If no path was given in `$ARGUMENTS`, look for a `.har` file in the conversation
   or working tree and confirm which one to use.
3. **Run `wiretrail <file> auto`** — one shot: it prints the summary, ranks the
   problems, and inlines the relevant deeper analysis scoped to where the trouble is.
   This usually answers the whole question. (Use `auto --all` to include LOW findings.)
4. **Go deeper only where needed.** If `$ARGUMENTS` names a focus (e.g. "errors",
   "auth", "duplicates"), or `auto` points somewhere specific, run that command
   directly — optionally with the `--filter` shown in the recommendation. The manual
   triage order is `subsystems`/`hosts` → `duplicates`+`diff`/`storms`/`pagination`/
   `rate-limit` → `errors`/`retries`/`cascade`/`slowest` → `auth`/`jwt` →
   `search`/`extract` → `handoff`/`report`/`show-entry`.
5. **Report the findings** with the exact wiretrail evidence (entry IDs, counts,
   statuses, snippets). The output is redacted and safe to quote directly.
6. Only use `--unsafe-include-secrets` if the user needs a *replayable* request, and
   warn them the output then contains live credentials.

Prefer `--json` piped to `jq` when you need to filter or extract specific fields.
Do not hand-parse the raw HAR JSON — wiretrail already normalizes routes and redacts
secret-bearing URL blobs.

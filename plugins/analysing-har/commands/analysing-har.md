---
description: Analyse a HAR network capture with wiretrail — runs the standard triage and reports findings.
argument-hint: <path-to.har> [focus, e.g. errors|auth|duplicates]
---

Analyse the HAR capture at: **$ARGUMENTS**

Follow the `analysing-har` skill. Concretely:

1. **Ensure wiretrail is installed** — `command -v wiretrail >/dev/null && wiretrail --version || cargo install wiretrail`.
2. If no path was given in `$ARGUMENTS`, look for a `.har` file in the conversation
   or working tree and confirm which one to use.
3. **Run `wiretrail <file> summary`** first. Read the `hints` and `next useful commands`.
4. **Follow the triage top-down**, running only the commands the findings point to:
   `subsystems`/`hosts` → `duplicates`+`diff`/`storms`/`pagination`/`rate-limit` →
   `errors`/`retries`/`transitions`/`slowest` → `auth`/`jwt` → `handoff`/`report`/`show-entry`.
   If `$ARGUMENTS` names a focus (e.g. "errors", "auth", "duplicates"), go straight to it after `summary`.
5. **Report the findings** with the exact wiretrail evidence (entry IDs, counts,
   statuses, snippets). The output is redacted and safe to quote directly.
6. Only use `--unsafe-include-secrets` if the user needs a *replayable* request, and
   warn them the output then contains live credentials.

Prefer `--json` piped to `jq` when you need to filter or extract specific fields.
Do not hand-parse the raw HAR JSON — wiretrail already normalizes routes and redacts
secret-bearing URL blobs.

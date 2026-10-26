# Automate

WireTrail is built to be run by something other than a person: a script, a build, an agent. Output is
deterministic, every command has a JSON form, entry ids are the same from one command to the next, and for most commands the exit code says whether
there was something to read.

| You want to | Use | Page |
|---|---|---|
| Read results in a script | `--json` and a stable envelope | [JSON output](json-output.md) |
| Fail a build on a regression or a finding | Exit codes, `compare --fail-on` | [Gate a build](ci-gates.md) |
| Put every request in a spreadsheet or a database | `export` | [Work with exported data](export-data.md) |
| Let an AI agent run the analysis | A Claude Code plugin | [The Claude Code plugin](claude-code-plugin.md) |

## What makes it automatable

- **Exit codes.** `0` means nothing to report, `1` means findings, `2` means WireTrail could not read its input.
  [Errors and exit codes](../reference/errors-and-exit-codes.md) lists them for every command.
- **A stable shape.** Every command's JSON has the same outer envelope, with a `schema_version`.
- **Stable ids.** An entry id such as `e000069` names the same request in every command, so a script can find a problem and open it.
- **Clean output.** JSON carries no colors, banners or decoration. Errors go to stderr as plain text.
- **No state.** Each run reads the capture again and keeps nothing between runs.

## Related

- [Compare](../compare/index.md) for the checks you gate on.
- [Reference: JSON schemas](../reference/json-schemas.md) for every field.

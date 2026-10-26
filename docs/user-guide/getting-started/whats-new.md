# What's new

Each release, newest first. The [GitHub releases page](https://github.com/johnneerdael/wiretrail/releases)
has the downloads and the notes as they were published.

## 0.3.2 — October 26, 2026

This release is about the documentation and the identity. The analysis output is unchanged.

- **A user guide at [wiretrail.neerdael.nl](https://wiretrail.neerdael.nl/).** Tutorials, how-tos,
  recipes, concepts and a reference. Every block of output in it comes from a real run on a demo
  capture, and CI fails the build if a block of output no longer matches a real run or a page uses a flag
  the tool does not have. The demo captures are downloadable from [Follow along](follow-along.md).
- **A new README and the WireTrail brand.** The README opens with the project banner, which follows your light or
  dark setting, and the release notes use the same banner. The guide uses the WireTrail typefaces and colors.
- **The README and the guide say what redaction does not do.** WireTrail masks known secret headers, known
  secret JSON keys and values that look like random tokens. It does not recognize every secret.
  [How redaction works](../concepts/redaction.md) lists what is and is not masked, command by command.
- **`USERGUIDE.md` is now a map into the guide.** It lists where each of its old sections went. Its
  examples came from a private capture, and the guide's demo captures replace them.
- **The Claude Code plugin links to the guide.** Update the plugin to get the new text.
- **The crate links to the guide** as its documentation, and the published package no longer carries
  the guide's sources.

## 0.3.1 — May 26, 2026

- **`show-entry` no longer truncates response bodies when `--unsafe-include-secrets` is on.**
  By default a body is shortened to 2,000 characters. With the flag, you now get the whole body.

## 0.3.0 — May 24, 2026

- **`auto`: one command for a full first analysis.** It prints the summary, ranks the likely
  problems and runs the follow-up analysis for each serious one. A 5xx cluster or a repeated call is
  followed up on its own host or route. See [Auto](../analyze/auto.md).
- **`summary` ends with recommended next steps.** They are the same evidence-backed
  recommendations that power `diagnose` and `auto`, each naming the command to run next.
- **One recommender behind all three.** `summary`, `diagnose` and `auto` share it, so they cannot disagree.
- `auto` brings the total to 32 commands.

## 0.2.0 — May 24, 2026

Nine commands, for 31 in total.

- `diagnose`, `validate`, `startup` and `cascade` for diagnosis and capture quality.
- `search`, `extract` and `export` to get at bodies and data.
- `compare` and `rules`, with `--fail-on` to gate a build.

## 0.1.0 — May 24, 2026

The first release: 22 commands, a stable `--json` envelope, a shared filter language, exit codes,
redaction by default, an optional `wiretrail.yaml`, and a loader that reads the file once through memory
mapping. It began as a fork of `har-rs`, whose HAR types it still uses.

## Which version added what

| Command or feature | Version |
|---|---|
| `summary`, `hosts`, `subsystems`, `endpoints`, `timeline`, `duplicates`, `retries`, `storms`, `pagination`, `rate-limit`, `errors`, `redirects`, `transitions`, `slowest`, `jwt`, `auth`, `handoff`, `show-entry`, `diff`, `checks`, `report`, `curl` | 0.1.0 |
| `--json`, `--filter`, `--top`, `--config`, `--unsafe-include-secrets` | 0.1.0 |
| `diagnose`, `validate`, `startup`, `cascade`, `search`, `extract`, `export`, `compare`, `rules` | 0.2.0 |
| `compare --fail-on` | 0.2.0 |
| `auto`, recommended next steps in `summary` | 0.3.0 |

## Related

- [Download and install](install.md) to update.
- [Command line](../reference/command-line.md) for every command and flag.

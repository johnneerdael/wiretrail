<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://github.com/johnneerdael/wiretrail/raw/HEAD/docs/assets/brand/wiretrail-readme-dark-1600x480.png">
    <img src="https://github.com/johnneerdael/wiretrail/raw/HEAD/docs/assets/brand/wiretrail-readme-light-1600x480.png" alt="WireTrail. Follow the request. Find the cause." width="100%">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/johnneerdael/wiretrail/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/johnneerdael/wiretrail?style=flat-square&color=28C5DC&label=release"></a>
  <a href="https://github.com/johnneerdael/wiretrail/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/johnneerdael/wiretrail/ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="https://crates.io/crates/wiretrail"><img alt="crates.io" src="https://img.shields.io/crates/v/wiretrail?style=flat-square&color=101820"></a>
  <a href="https://wiretrail.neerdael.nl/"><img alt="User guide" src="https://img.shields.io/badge/user%20guide-online-101820?style=flat-square"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/Linux%20%C2%B7%20macOS%20%C2%B7%20Windows-101820?style=flat-square">
  <a href="https://github.com/johnneerdael/wiretrail/blob/main/LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-101820?style=flat-square"></a>
</p>

WireTrail is a command-line analyzer for HAR network captures. It groups repeated calls, finds the failures and the slow requests, ranks the likely causes, and compares one capture with another. Every investigation is one command that prints a report you can read, paste into a ticket after you have checked it, or parse as JSON.

It is a single binary with no runtime dependencies. On a 150 MB capture most commands take about half a second on a laptop.

<p align="center">
  <a href="https://github.com/johnneerdael/wiretrail/releases/latest"><b>Download</b></a> &nbsp;·&nbsp;
  <a href="https://wiretrail.neerdael.nl/getting-started/install/"><b>Install</b></a> &nbsp;·&nbsp;
  <a href="https://wiretrail.neerdael.nl/"><b>User guide</b></a> &nbsp;·&nbsp;
  <a href="https://github.com/johnneerdael/wiretrail/issues"><b>Report an issue</b></a>
</p>

The [user guide](https://wiretrail.neerdael.nl/) has tutorials, how-tos, recipes, concepts and the full reference. Every block of output in it comes from a real run on a demo capture that you can download. Its sources are in [`docs/user-guide/`](docs/user-guide/).

## Install

```bash
cargo install wiretrail
```

Or download a prebuilt binary for Linux, macOS or Windows (x86-64 and ARM64) from the [releases page](https://github.com/johnneerdael/wiretrail/releases/latest), and put it on your `PATH`. [Download and install](https://wiretrail.neerdael.nl/getting-started/install/) has the details, including building from source.

## First triage

```bash
wiretrail capture.har              # the summary: what is in it, and what to run next
wiretrail capture.har auto         # the summary, then the follow-up analysis for the serious findings
wiretrail capture.har diagnose     # only the ranked findings, with the evidence behind each
wiretrail capture.har errors       # failures, with what the server said
wiretrail capture.har show-entry e000069   # one request in full
wiretrail new.har compare baseline.har --fail-on high   # what got worse; exits 1 if it did
```

The command comes after the file. This is what `diagnose` prints on the guide's demo capture, `checkout.har`:

```bash
wiretrail checkout.har diagnose --top 3
```

<!-- output: home-hero.txt from="[HIGH] retry-exhaustion" until="[HIGH] 5xx-cluster" -->
```text
[HIGH] retry-exhaustion — 4 retries, final 500 on POST /v1/cart/sync
  repeated retries did not recover
  evidence: e000069, e000070, e000072, e000073, e000076
  -> wiretrail <file> retries
```

[Your first triage](https://wiretrail.neerdael.nl/getting-started/first-triage/) walks through this on the demo capture and explains each line.

## What it does

| You want to know | Use | Guide |
|---|---|---|
| What is in the capture, and where to look first | `summary`, `auto`, `diagnose`, `validate` | [Analyze](https://wiretrail.neerdael.nl/analyze/) |
| What repeated, and whether it was a retry, a burst or a loop | `duplicates`, `retries`, `storms`, `pagination`, `rate-limit`, `diff` | [Repeats](https://wiretrail.neerdael.nl/repeats/) |
| What failed, what was slow, and how sign-in went | `errors`, `redirects`, `transitions`, `slowest`, `startup`, `cascade`, `auth`, `jwt` | [Failures](https://wiretrail.neerdael.nl/failures/) |
| One request in full, or text inside bodies | `show-entry`, `search`, `extract`, `curl` | [Inspect](https://wiretrail.neerdael.nl/inspect/) |
| Something to hand to another team | `report`, `handoff`, `export` | [Inspect](https://wiretrail.neerdael.nl/inspect/) |
| What got worse since a good capture, or what breaks your rules | `compare`, `rules`, `checks` | [Compare](https://wiretrail.neerdael.nl/compare/) |
| Output for scripts and CI | `--json`, exit codes | [Automate](https://wiretrail.neerdael.nl/automate/) |

Not sure which command fits? [Choose the right command](https://wiretrail.neerdael.nl/getting-started/choose-a-command/) starts from your question.

`--filter` limits any command to the requests that match, `--top` bounds the lists, and `--config` names a [configuration file](https://wiretrail.neerdael.nl/compare/config-file/) for subsystem names and rules. Every command accepts `--json`, with a stable envelope.

WireTrail has no window to click through. To watch traffic as it happens, or to edit and resend a request, use your browser's DevTools or a proxy, and run WireTrail on the capture you export. A ranked finding is a pattern, and not a diagnosis: [Limitations](https://wiretrail.neerdael.nl/reference/limitations/) lists what WireTrail does not do and where a result can mislead.

## Get a capture

Chrome, Edge, Firefox and Safari save one from the Network tab of their developer tools, and proxies such as Charles, Proxyman, HTTP Toolkit and Fiddler export one. [Capture](https://wiretrail.neerdael.nl/capture/) covers each, and what a capture needs so that the analysis can use it.

## Redaction and safety

A HAR holds cookies, tokens and personal data. WireTrail masks the headers it knows are secret, the JSON keys it knows hold secrets, and values that look like random tokens, and `--unsafe-include-secrets` turns that off.

**Masking is pattern-based, and it is not a guarantee.** A JWT in a header it does not know, a form-encoded body, a short secret in a query string, a server's error message, and anything `search` or `extract` prints can reach the output. Read what you are about to share. [How redaction works](https://wiretrail.neerdael.nl/concepts/redaction/) lists what each command masks, and [Handle captures safely](https://wiretrail.neerdael.nl/capture/handle-captures-safely/) says how to strip a capture before you send it.

## Use it from Claude Code

This repository is also a [Claude Code](https://claude.com/claude-code) plugin marketplace. The plugin adds a skill that runs `wiretrail` for you when you mention a HAR capture:

```
/plugin marketplace add johnneerdael/wiretrail
/plugin install analysing-har@analysing-har
```

[The Claude Code plugin](https://wiretrail.neerdael.nl/automate/claude-code-plugin/) explains what it does, and how to update or remove it.

## Formats

HAR 1.2 and 1.3, as JSON in UTF-8, without compression. [HAR support](https://wiretrail.neerdael.nl/reference/har-support/) lists the fields it reads, and [Performance](https://wiretrail.neerdael.nl/reference/performance/) has measured time and memory.

## Contributing

Issues and pull requests are welcome. Before you open one:

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The user guide is built with MkDocs, and every block of command output in it is generated from a real run. [About](https://wiretrail.neerdael.nl/help/about/#working-on-this-guide) explains how to work on it.

## Credits

WireTrail began as a fork of [mandrean/har-rs](https://github.com/mandrean/har-rs), which contributes the HAR 1.2 and 1.3 type definitions. It is the sibling of [HeapTrail](https://heaptrail.neerdael.nl/), an analyzer for heap dumps, and shares its design: one command per question, deterministic output, and JSON for machines.

## License

[MIT](https://github.com/johnneerdael/wiretrail/blob/main/LICENSE).

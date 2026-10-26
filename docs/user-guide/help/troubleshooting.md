# Troubleshooting

Find your symptom. Each entry says what is happening and what to try.

## `command not found: wiretrail`

The folder that holds the `wiretrail` file is not on your `PATH`. Run it by its full path to confirm it works, then move it to a folder that is on the `PATH`, such as `/usr/local/bin`, or add its
folder to the `PATH`. With `cargo install`, the folder is `~/.cargo/bin`. See [Download and install](../getting-started/install.md).

## macOS or Windows blocks the program

The release binaries are not code-signed. A file saved by a browser can be blocked by macOS Gatekeeper or warned about by Windows SmartScreen. Download with `curl` instead, as the
install page shows, or build from source with `cargo install wiretrail`.

## `wiretrail: failed to parse HAR JSON`

The file is not something WireTrail can read. The message does not say why. The usual causes:

- The file is **empty or cut off**, for example from a download that stopped. Check its size.
- It is **compressed**. Unpack a `.gz` or `.zip` first.
- It starts with a **byte-order mark**. Some Windows tools write one. Remove the first three bytes.
- It is **valid JSON that is not a HAR**: there is no top-level `log`.
- An entry has **no `request` or `response`**, or a **number of the wrong kind**: a status or a size with a decimal point, or `null` for `time`, a timing or a header value.
- It is **not JSON**, such as a proxy's own session format.

Look at the start of the file with `head -c 200 capture.har`. It should begin with `{"log"` or `{ "log"`. See [Errors and exit codes](../reference/errors-and-exit-codes.md).

## `wiretrail: failed to read HAR file`

The path is wrong, or the file cannot be opened. Check the spelling, and remember that the file name comes **before** the command: `wiretrail capture.har errors`.

## A command prints its header and nothing else

There is nothing to report, or `--top` or `--filter` removed it. A clean capture has no storms, no 429s and no retries, and an empty result is correct. Check that the filter matches
something: `wiretrail capture.har --filter "…" summary --top 1` shows how many entries matched.

## The exit code is 1 but nothing looks wrong

`1` means the command reported something, and that can be an informational line such as `hosts with inconsistent Authorization` in `auth`. It is not an error. See
[Errors and exit codes](../reference/errors-and-exit-codes.md#when-each-command-returns-1).

## My filter matches nothing

- **Quote it.** `--filter "status:>=500"`. Without quotes your shell reads `>` as a redirect.
- **`path:` is the raw path**, with the real ids, not the `{id}` form in the output. Use `*` where the id goes.
- **`path:` does not see the query string.**
- **`host:` must match the whole host.** `host:shop.example` does not match `api.shop.example`. Use `host:*shop.example`.
- **`status:>=400` skips requests with no status.** Use `status:0` for those.

See [Filter entries](../analyze/filters.md).

## `auto` prints far too much

It runs every drill-down at or above medium severity. Use `--min-severity high`, or run `diagnose` for the list and open the findings you care about. Add `--filter` to scope it to one host. See [Auto](../analyze/auto.md).

## Findings I expect are missing

The ranked list is cut to `--top` entries, default 10, and the low-severity findings go first. Run `diagnose --top 20`. On a capture with many error groups a serious finding can also be cut
before it is ranked: see [Limitations](../reference/limitations.md#-top-cuts-each-check-before-the-findings-are-ranked). A pattern below a rule's threshold raises no finding: see
[How findings are ranked](../concepts/how-findings-are-ranked.md).

## `jwt` or `auth` shows nothing

The capture may not hold credentials. Run `validate`. A capture that was stripped on export has no `Authorization` headers, and `jwt` and `auth` then have nothing to read. See
[Validate a capture](../analyze/validate.md).

## `errors` shows a status and no message

The capture has no response bodies, or the body is not JSON, or the message key is not one WireTrail looks for. Open one entry with `show-entry`.
See [Errors](../failures/errors.md).

## A secret I need shows as `<redacted>`

Add `--unsafe-include-secrets` to `show-entry`, `curl`, `handoff`, `errors`, `search`, `extract`, `diff`, `jwt` or `auto`. `report` ignores it. The output then holds live credentials, so keep it private.

## A secret I did not expect to see is in the output

Masking works by name and by shape, and some places are not covered. See [How redaction works](../concepts/redaction.md#what-gets-through), and do not share the output.

## `subsystems` lists raw host names

A host that is not a known vendor and is not in your configuration file shows under its own name. Add an `ownership` rule. See [The configuration file](../compare/config-file.md).

## A configuration section has no effect

Check the spelling of the key. A misspelled key is ignored without an error. Only `subsystems`, `report`, `checks` and `rules` read the file, and `wiretrail.yaml` is read from the **current folder**.

## It is slow, or uses a lot of memory

It holds the whole capture in memory, about three and a half times the file size, and about four times for `summary`, `diagnose` and `auto`. Those three, and `report`, do more work than the others, and a route with thousands of 429s makes them much slower.
See [Performance](../reference/performance.md).

## The times are all wrong

A start time that cannot be read becomes offset `0` with no warning. Check the dates in the file. See [How WireTrail reads a HAR](../concepts/how-wiretrail-reads-a-har.md#time).

## Still stuck

[Open an issue](https://github.com/johnneerdael/wiretrail/issues) with the output of `wiretrail --version`, your operating system, the command you ran and the message you got. Do **not** attach the
capture unless you have stripped it. See [Handle captures safely](../capture/handle-captures-safely.md).

## Related

- [FAQ](faq.md) for common questions.
- [Errors and exit codes](../reference/errors-and-exit-codes.md) lists every message.

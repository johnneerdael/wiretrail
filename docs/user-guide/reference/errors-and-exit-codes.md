# Errors and exit codes

## Exit codes

| Code | Meaning |
|---|---|
| `0` | The command ran and had nothing to report. |
| `1` | The command ran and reported findings. This is normal, and it is the signal a gate uses. |
| `2` | WireTrail could not do what it was asked: the capture or config could not be read, the filter or regex was invalid, an id did not exist, or the arguments were wrong. |

`3` is defined inside WireTrail and is **never returned**.

One more code can appear. If the program that reads WireTrail's output closes the pipe early, for example `| head` on a result larger than the pipe's buffer, WireTrail stops with a panic message
(`failed printing to stdout: Broken pipe`) and exit code `101`. Write a large result to a file, or read all of it.

## When each command returns 1

| Command | Exits `1` when |
|---|---|
| `summary` | There is any error response, or any repeated call. |
| `auto`, `diagnose` | There is at least one ranked finding. |
| `validate` | There is any anomaly, such as a request with no status. |
| `hosts`, `subsystems` | Any host or subsystem that is listed has an error or a duplicate. |
| `endpoints` | Any endpoint that is listed has an error. |
| `duplicates`, `retries`, `errors`, `transitions`, `storms`, `rate-limit`, `checks`, `rules` | The command lists at least one item. |
| `handoff` | The capture has any entry that matches, because it lists the slowest requests as well as the failures. It exits `1` on a capture with no errors. |
| `redirects` | A group is a storm. |
| `pagination` | There is any sequence or any N+1. |
| `diff` | A group that is listed has a `meaningful` body or a varying `authorization` header. |
| `jwt` | A token was expired when it first appeared in the capture. |
| `auth` | There is an auth failure, a host with inconsistent `Authorization`, or a refresh that failed, reused an old token or ran concurrently. |
| `cascade` | There is any failure at all. |
| `compare` | There is a new error, a latency regression or a payload growth, or with `--fail-on`, one at or above that level. New and removed hosts and endpoints never change the exit code. |
| `slowest`, `startup`, `timeline`, `show-entry`, `report`, `curl`, `search`, `extract`, `export` | Never. They return `0`, or `2` for an error. |

The exit code of `hosts`, `subsystems`, `endpoints` and `diff` is worked out from the rows that are printed, so `--top` (10 unless you change it) can hide what would have made them exit `1`. For a gate, use
`errors`, or raise `--top`.

## Messages

WireTrail prints errors on standard error, and each one starts with `wiretrail:`. Most are one short line that does not name the underlying cause. `invalid filter` and `invalid regex` do name it, and the
regex message runs to several lines.

| Message | Cause | Example |
|---|---|---|
| `wiretrail: failed to read HAR file` | The file does not exist or cannot be opened. | `wiretrail nope.har` |
| `wiretrail: failed to parse HAR JSON` | The file is not valid JSON, is not a HAR, or is empty, truncated, compressed or has a byte-order mark. An entry with no `request` or `response`, a status or size that is not a whole number, or a `null` for `time`, a timing or a header value, fails it too. | `wiretrail empty.har` |
| `wiretrail: baseline: failed to read HAR file` | The baseline for `compare` cannot be opened. The same for `failed to parse HAR JSON`. | `wiretrail a.har compare nope.har` |
| `wiretrail: failed to read config file` | `--config` names a file that cannot be read, or the default `./wiretrail.yaml` cannot be. Only `subsystems`, `report`, `checks` and `rules` read the file. | `--config nope.yaml` |
| `wiretrail: failed to parse config YAML` | The config is not valid YAML, or a key has the wrong shape, such as a rule with no `name`. | `--config broken.yaml` |
| `wiretrail: invalid filter: …` | A filter clause is unknown or malformed. | `--filter "colour:red"` |
| `wiretrail: no entry with id or index '…'` | `show-entry` or `curl` was given an id that does not exist. | `show-entry e999999` |
| `wiretrail: invalid regex: …` | `search --regex` was given a bad pattern. The regex library's message follows. | `search "(" --regex` |

The three file errors in action:

```bash
wiretrail nope.har
```

<!-- output: err-missing.txt from="wiretrail:" -->
```text
wiretrail: failed to read HAR file
```

```bash
wiretrail truncated.har
```

<!-- output: err-truncated.txt from="wiretrail:" -->
```text
wiretrail: failed to parse HAR JSON
```

A file with a byte-order mark, a gzip file, an empty file, valid JSON that is not a HAR, and a JSON object with no `log` all give the same parse message, and the cause is not named:

```bash
printf '\357\273\277{"log":{"version":"1.2","entries":[]}}' > bom.har; wiretrail bom.har
```

<!-- output: err-bom.txt from="wiretrail:" -->
```text
wiretrail: failed to parse HAR JSON
```

```bash
wiretrail not-a-har.json
```

<!-- output: err-notahar.txt from="wiretrail:" -->
```text
wiretrail: failed to parse HAR JSON
```

If you are not sure which, check the start of the file with `head -c 200`: it should begin `{"log"` or `{ "log"`, with no unusual bytes before it. To read a compressed
capture, unpack it first.

## Filter messages

```bash
wiretrail checkout.har --filter "colour:red" summary
```

<!-- output: flt-bad.txt from="wiretrail:" -->
```text
wiretrail: invalid filter: unknown filter key: colour
```

The other two are `invalid filter clause: <text>` for a token with no colon, and `invalid number: <text>` for a `status:` or `time:` value that is not a number.

## Argument errors

A wrong command line is caught before WireTrail reads anything, and the message comes from the argument parser in its own format, with the usage line. The exit code is `2`.

```bash
wiretrail
```

<!-- output: err-nofile.txt from="error:" -->
```text
error: the following required arguments were not provided:
  <FILE>

Usage: wiretrail <FILE>

For more information, try '--help'.
```

```bash
wiretrail checkout.har hosts --top abc
```

<!-- output: err-top.txt from="error:" -->
```text
error: invalid value 'abc' for '--top <TOP>': invalid digit found in string

For more information, try '--help'.
```

## An empty capture is not an error

A well-formed capture with no entries loads, and every command reports nothing, with exit code `0`:

```bash
echo '{"log":{"version":"1.2","entries":[]}}' > noentries.har; wiretrail noentries.har; echo "exit code: $?"
```

<!-- output: har-empty-entries.txt from="== wiretrail summary" -->
```text
== wiretrail summary ==
entries: 0 total, 0 after filter
duration (first start to last response): 0ms

status classes:

resource types:

top hosts (by request count):

slowest requests:

biggest payloads:

next useful commands: duplicates · errors · slowest
exit code: 0
```

## Related

- [Troubleshooting](../help/troubleshooting.md) has fixes for the common ones.
- [Gate a build](../automate/ci-gates.md) uses the exit codes.
- [HAR support](har-support.md) says what is read.

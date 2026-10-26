# FAQ

## What is WireTrail?

A command-line analyzer for HAR network captures. It answers questions about a capture: what repeated, what failed, what was slow, how a sign-in went, what changed since a
good capture. It is a single program with no window, and its output is plain text or JSON. See [Get started](../getting-started/index.md).

## When should I use DevTools or a proxy instead?

Use them when you want to watch traffic as it happens, edit a request and send it again, or explore with no question in mind. WireTrail starts after that: you record a capture with DevTools
or a proxy, and WireTrail answers questions about the file. They work well together. It also helps where there is no interface, on a CI runner or in a script, or when an agent should read the result.

## Does it change my capture, or send it anywhere?

No. WireTrail opens the file read-only, writes only to standard output, and makes no network connections. See [How WireTrail reads a HAR](../concepts/how-wiretrail-reads-a-har.md).

## Is the output safe to paste into a ticket?

Only after you have read it. WireTrail masks the headers, JSON keys and token-shaped values it recognizes. It does not recognize every secret, and `search` and `extract` mask
less than the other commands. [How redaction works](../concepts/redaction.md) lists exactly what is and is not covered, and [Handle captures safely](../capture/handle-captures-safely.md)
says how to check.

## Which command should I run first?

`wiretrail capture.har`, which runs `summary`. If you want the follow-up analysis done for you, `wiretrail capture.har auto`. See [Choose the right command](../getting-started/choose-a-command.md).

## Why does a command that worked exit with 1?

`1` means the command reported findings. `0` means it found nothing, and `2` means it could not do what you asked. A script uses this as a signal. See [Errors and exit codes](../reference/errors-and-exit-codes.md).

## Why is the list of recommendations shorter than I expected?

`--top` limits it, and the default is 10. The list is sorted by severity before it is cut, so low-severity findings go first. Use `--top 20`. Each check behind the list is also cut to `--top` groups
before it is ranked, so a serious finding can be missing from a capture with many error groups. See [Diagnose](../analyze/diagnose.md) and
[Limitations](../reference/limitations.md#-top-cuts-each-check-before-the-findings-are-ranked).

## Why does `timeline` show only ten rows?

`--top` applies to it too. Raise it, or filter to the part you want. See [Timeline](../analyze/timeline.md).

## What is `e000069`?

An entry id: the request's position in the file, written as `e` and six digits. It is the same in every command. See [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md).

## Why is a path shown as `{id}` or `{blob}`?

Normalization replaces identifiers and opaque segments, so that `/products/1042/price` and `/products/1043/price` count as one endpoint. `--filter "path:..."` matches the raw path.
See [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md).

## How big a capture can it read?

It holds the whole file in memory, and used about three and a half times the file size on the capture measured, and about four times for `summary`, `diagnose` and `auto`. Most commands take about half a second on 150 MB. See [Performance](../reference/performance.md).

## Can it read a gzip file, or a HAR from my tool?

Not a compressed file: unpack it first. It reads HAR 1.2 and 1.3 JSON from any tool, as long as the file is UTF-8 with no byte-order mark. See [HAR support](../reference/har-support.md).

## Does it analyze WebSocket or HTTP/3 traffic?

It reads whatever the HAR records as entries. A HAR has no standard place for the messages of a WebSocket, and WireTrail does not read tool-specific fields for them. It does not know which
protocol version carried a request beyond the string the HAR gives.

## Can I use it in CI?

Yes. Exit codes and `compare --fail-on` are made for it. See [Gate a build](../automate/ci-gates.md).

## Is there a graphical interface?

No. Everything is a command. For clicking through requests, use your browser's DevTools or a proxy.

## Does it work offline?

Yes. It needs no network.

## Which platforms does it run on?

Linux, macOS and Windows, on x86-64 and ARM64. See [Download and install](../getting-started/install.md).

## How is it related to HeapTrail?

HeapTrail is the same author's analyzer for heap dumps. WireTrail uses the same design: one command per question, deterministic output, JSON for machines, and a shared visual identity. They are separate
programs.

## Which license is it under?

MIT. See [About](about.md).

## Related

- [Troubleshooting](troubleshooting.md) for things that go wrong.
- [Limitations](../reference/limitations.md) for what WireTrail does not do.

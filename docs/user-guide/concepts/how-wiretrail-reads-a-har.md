# How WireTrail reads a HAR

WireTrail reads the capture once, turns each request into a flat record, and answers every command from that list of records. Knowing the
steps explains why some things are fast, why some are approximations, and what a strange file does.

## What is in a HAR

A HAR is JSON. At the top is a `log`, and inside it a list of `entries`, one per request. Each entry has a start time, a total time, the
request, the response and the timings. This is the first entry of the demo capture, cut down to the parts WireTrail uses most:

```bash
jq '.log.entries[0] | {startedDateTime, time, request: (.request | {method, url, httpVersion}), response: (.response | {status, statusText, content: (.content | {size, mimeType})}), timings}' checkout.har
```

<!-- output: har-entry.txt -->
```text
{
  "startedDateTime": "2026-10-20T09:14:02.000Z",
  "time": 21,
  "request": {
    "method": "GET",
    "url": "http://www.shop.example/",
    "httpVersion": "h2"
  },
  "response": {
    "status": 301,
    "statusText": "Moved Permanently",
    "content": {
      "size": 0,
      "mimeType": "text/html"
    }
  },
  "timings": {
    "blocked": -1,
    "dns": -1,
    "connect": -1,
    "ssl": -1,
    "send": 1,
    "wait": 18,
    "receive": 2
  }
}
```

The real entry has more fields: headers, cookies, the body of the request and the response, the server's address. [HAR support](../reference/har-support.md)
lists every field WireTrail reads.

## What WireTrail does

1. **Maps the file into memory.** The operating system pages it in as it is read, so WireTrail does not copy it first.
2. **Parses it once, into typed fields.** There is no generic JSON tree in between. Fields it does not use are skipped, and most fields that are
   missing get a default, so a capture from a tool that leaves out `cookies` or `cache` still loads. The `log`, and the `request` and `response` of every entry, must be present.
3. **Builds one record per entry.** The record has an id, a start offset, the host, the path and its [normalized form](entries-ids-endpoints.md),
   the query, the status, the headers, the bodies, the timing phases, the sizes, the server address, any redirect target and the correlation ids.
4. **Applies `--filter`.** Commands work on the entries that match. `validate` ignores the filter, and `show-entry` and `curl` ignore it when you give an id.
5. **Computes the answer** for the command, and prints it as text or as JSON.

Nothing is written to disk and nothing is sent anywhere. WireTrail only reads the file you name, and writes to standard output.

## Time

The capture starts at the **earliest start time that WireTrail can parse**. Every entry's `offset` is its own start minus that. The duration of an entry is
the HAR's `time` field for it, which a tool measures as one number, so it is not recomputed from the phases.

A start time that cannot be read, such as a word where a date should be, is skipped. That entry gets offset `0`, and it takes no part in
working out where the capture starts. Here the first entry's start was replaced with `"yesterday"`, and the second entry became the start of the capture:

```bash
jq '.log.entries[0].startedDateTime = "yesterday"' checkout.har > badtime.har; wiretrail badtime.har timeline --top 2
```

<!-- output: har-badtime.txt from="0ms" until="next useful" -->
```text
     0ms     21ms  e000000 GET www.shop.example/  [301] 0 B
     0ms    261ms  e000001 GET shop.example/  [200] 19.4 KiB
```

Both entries show offset `0ms`. Time-based commands such as `storms`, `cascade` and `rate-limit` read these offsets, so a capture with unreadable dates
gives wrong answers without an error.

## Same input, same output

WireTrail has no random choices, no clock and no network. The same file and the same flags give the same output, and anything that could tie is
sorted by a fixed rule (by count, then by name or id). That is why the output diffs cleanly and why a script can compare two runs. The one exception is the hash
printed by `jwt`, which depends on the build. See [JWTs](../failures/jwt.md).

## What it holds in memory

WireTrail keeps the whole capture in memory while it runs, bodies included. On the 150 MB file measured for [Performance](../reference/performance.md), a typical
command used about 500 MiB, roughly three and a half times the size of the file. A very large capture needs a machine with that much memory to spare.

## What it does not do

- It does not check that a HAR is well formed beyond reading it. A HAR with impossible numbers, such as a negative duration, loads, and `validate` flags some of them.
- It does not read a compressed HAR, a HAR with a byte-order mark, or a HAR inside another file. [Errors and exit codes](../reference/errors-and-exit-codes.md) shows what each does.
- It does not know why a request was made. It sees the order and the timing, and never the code that sent them.

## Related

- [Entries, ids and endpoints](entries-ids-endpoints.md) explains the ids and the grouping.
- [HAR support](../reference/har-support.md) lists the fields and versions.
- [Limitations](../reference/limitations.md) lists what the analysis cannot tell you.

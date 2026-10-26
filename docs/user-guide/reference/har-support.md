# HAR support

## Versions

WireTrail reads HAR **1.2** and **1.3** through one permissive model. The `version` string is recorded and shown, and it does not change how the file is read, so a file that says `1.1`
loads as well:

```bash
sed 's/"version":"1.2"/"version":"1.1"/' checkout.har > old.har; wiretrail old.har --json --top 1 | jq '.capture.har_version'
```

<!-- output: har-old.txt from='"1.1"' -->
```text
"1.1"
```

## What the file must be

- **JSON**, in UTF-8, with **no byte-order mark**.
- **Uncompressed.** A `.har.gz` or a zipped file must be unpacked first.
- **An object with a `log` key.** `log` can be missing its other keys.
- **Every entry has a `request` and a `response`.** `status` and the sizes are whole numbers, and `time`, `send`, `wait` and `receive` are numbers and not `null`. `null` is accepted for `blocked`,
  `dns`, `connect`, `ssl`, `mimeType`, `text`, `redirectURL` and `serverIPAddress`.

A file that does not meet these stops with `wiretrail: failed to parse HAR JSON`. See [Errors and exit codes](errors-and-exit-codes.md).

## Fields that are read

| In the HAR | Used for |
|---|---|
| `log.version`, `log.creator.name`, `log.creator.version`, `log.browser.name` | `validate`, the `capture` object of every JSON result, `report` |
| `entries[].startedDateTime` | The start offset. Must be an RFC 3339 date. An unreadable one is treated as missing. |
| `entries[].time` | The duration of the request, in milliseconds |
| `entries[].serverIPAddress` | `show-entry`, `handoff` |
| `request.method`, `request.url` | Method, host, path, query. The query is read from the URL. |
| `request.httpVersion`, `response.httpVersion` | `show-entry` (the response's, or the request's when empty) |
| `request.headers`, `response.headers` | Auth, cookies, rate limits, correlation ids, `rules`, `checks`, `show-entry`, `curl` |
| `request.postData.text` | Request bodies: `diff`, `search`, `extract --target req`, `validate`, `show-entry`, `curl` |
| `request.bodySize` | `hosts` (bytes sent) |
| `response.status`, `response.statusText` | Status. A missing status is `0`. |
| `response.content.text`, `.mimeType`, `.size` | Response bodies, content type, resource type, payload size |
| `response.redirectURL` | `redirects` |
| `response.bodySize` | Payload size (the larger of the body size and the content size is used) |
| `timings.blocked`, `dns`, `connect`, `ssl`, `send`, `wait`, `receive` | `slowest`, `show-entry`, `report`, `validate` |

A phase of `-1` means "does not apply" and is read as not present. A negative `send`, `wait` or `receive` is treated as `0`.

## Fields that are ignored

`cookies` (in requests and responses), `queryString` (the URL is used instead), `response.headersSize`, `cache`, `pageref`, `pages`, `connection`, `comment`, `postData.mimeType`, `postData.params`, `content.compression`,
and every custom field that starts with an underscore, such as Chrome's `_initiator` or `_error`. WireTrail does not read a header from the `cookies` array: it reads the `Cookie` and `Set-Cookie` headers in the
`headers` lists.

!!! note "Base64 bodies are not decoded"
    A response body that a tool stored encoded, with `content.encoding` set to `base64`, is read as the encoded text. `search` will not find words inside it and `extract` will
    skip it, because it is not JSON. Text and JSON bodies are stored as they are, so they work.

## Which tools write them

The project's README names Chrome, Edge, Firefox and Safari DevTools, and Charles, Proxyman, HTTP Toolkit and Fiddler as sources of exports it reads, and other tools' files load the same way because WireTrail asks
little of them. The two sample captures in the repository come from Charles. The demo captures in this guide are generated.
[Capture](../capture/index.md) says how to export one from each.

## What a tool can leave out

Tools differ in what they save. A capture may have no bodies, no cookies, stripped credentials or no timing phases, and `validate` reports which. The commands that need each part are in
[What a useful capture holds](../capture/what-a-capture-needs.md).

## Related

- [How WireTrail reads a HAR](../concepts/how-wiretrail-reads-a-har.md) for the steps.
- [Validate a capture](../analyze/validate.md) checks a file.
- [Timing phases](../concepts/timing-phases.md) defines the phases.

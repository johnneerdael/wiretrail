# Glossary

**Anomaly**
:   A request that looks broken in the capture itself, such as a status of `0`. `validate` counts them. See [Validate a capture](../analyze/validate.md).

**Baseline**
:   A capture of the same flow from a build you accept, used by `compare` as the thing to measure against. See [Compare against a baseline](../compare/compare.md).

**Blob**
:   A long path segment that looks like an encoded or random value, shown as `{blob}`. See [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md).

**Bottleneck**
:   The phase of a request that took the longest, named by `slowest`. See [Timing phases](../concepts/timing-phases.md).

**Cascade**
:   A failure followed within a window by several more. The later ones are *downstream*. See [Cascade](../failures/cascade.md).

**Cooldown**
:   The period a server's `Retry-After` header asks a client to wait. A call inside it is a *cooldown violation*. See [Rate limits](../repeats/rate-limits.md).

**Correlation id**
:   An identifier a server puts in its response headers, such as `x-request-id`, so that a request can be found in its logs. See [Errors](../failures/errors.md).

**Duplicate**
:   A request with the same fingerprint as another. See [Duplicates](../repeats/duplicates.md).

**Endpoint**
:   A method, a host and a normalized path. See [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md).

**Entry**
:   One request and its response, as recorded in a HAR. WireTrail names entries `e000000`, `e000001`, and so on, in file order.

**Envelope**
:   The outer JSON object every command prints with `--json`: `tool`, `schema_version`, `command`, `capture`, `result`, `warnings` and `next_commands`. See [JSON schemas](json-schemas.md).

**Evidence**
:   The entry ids behind a finding.

**Filter**
:   A set of `key:value` clauses given with `--filter` that limits a command to some requests. See [Filter entries](../analyze/filters.md).

**Fingerprint**
:   The method, host, normalized path and query of a request, without cache-buster keys. Requests with the same fingerprint are duplicates.

**Finding**
:   A ranked item from `summary`, `diagnose` or `auto`, with a severity, a reason and evidence. See [How findings are ranked](../concepts/how-findings-are-ranked.md).

**HAR**
:   HTTP Archive: a JSON file that records the requests and responses of a session. See [Get a HAR](../capture/index.md).

**Normalized path**
:   A URL path with its identifiers replaced by `{id}` and its opaque segments by `{blob}`.

**N+1**
:   One request for a list followed by one request for each item. See [Pagination and N+1](../repeats/pagination.md).

**Offset**
:   The time from the start of the capture to the start of a request, in milliseconds.

**Opaque**
:   Looks like a long random value. WireTrail masks opaque values wherever it finds them, with the exceptions on [How redaction works](../concepts/redaction.md).

**Phase**
:   One step in the life of a request: `blocked`, `dns`, `connect`, `ssl`, `send`, `wait`, `receive`.

**p50, p95**
:   The median, and the value that 95 percent of requests are below.

**Recommendation**
:   A ranked item from `summary` or `auto`: a severity, a reason, the evidence, and the command to run next. `diagnose` shows the same items as findings.

**Redaction**
:   Replacing a secret with `<redacted>` in output. See [How redaction works](../concepts/redaction.md).

**Retry**
:   A request that repeats a call after an attempt that failed with a 5xx, a 429 or no status. See [Retries](../repeats/retries.md).

**Severity**
:   `critical`, `high`, `medium` or `low`: a rule's judgement of how likely a pattern is to matter. See [How findings are ranked](../concepts/how-findings-are-ranked.md).

**Storm**
:   Many calls to one host or endpoint within a short window. See [Storms](../repeats/storms.md).

**Subsystem**
:   A named group of hosts, from your configuration file or a built-in vendor list. See [Hosts, subsystems and endpoints](../analyze/inventory.md#subsystems).

**TTFB**
:   Time to first byte: the `wait` phase.

**Volatile**
:   A body key that changes on every call, such as a timestamp. See [What varies between calls](../repeats/diff.md).

## Related

- [Concepts](../concepts/index.md) explains the main ideas.
- [Command line](command-line.md) lists the commands.

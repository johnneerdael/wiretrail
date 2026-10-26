# JSON schemas

With `--json`, every command prints one JSON document with the same envelope. This page lists the envelope and the fields of each command's `result`. The field names
below were read from real output, so they are exact.

## The envelope

```json
{
  "tool": "wiretrail",
  "schema_version": 1,
  "command": "errors",
  "capture": { "har_version": "1.2", "creator": "…", "creator_version": "…", "browser": null,
               "entry_count": 99, "start_ms": 1792487642000, "end_ms": 1792487711953, "duration_ms": 69953.0 },
  "result": { },
  "warnings": [],
  "next_commands": ["transitions", "redirects", "show-entry"]
}
```

| Field | Type | Meaning |
|---|---|---|
| `tool` | string | Always `wiretrail`. |
| `schema_version` | integer | `1` for every command. |
| `command` | string | The command name. |
| `capture` | object | `har_version`, `creator`, `creator_version`, `browser` (or `null`), `entry_count`, `start_ms` and `end_ms` (Unix milliseconds, or `null`), `duration_ms`. |
| `result` | object | The command's result, below. |
| `warnings` | array | Reserved. Always empty in this version. |
| `next_commands` | array | The commands the text output suggests. For `auto`, the commands of its recommendations. Empty for `report`. |

`export` prints raw NDJSON rows with no envelope, and `report` puts its markdown in `result.markdown`.

## Conventions

- **Times** are milliseconds and numbers: `offset_ms`, `duration_ms`, `gap_ms`. Offsets count from the start of the capture.
- **Entry ids** are strings such as `"e000069"`. A list of them is named `entry_ids`, `evidence_ids` or similar.
- **Statuses** are integers. `0` means no status.
- **A value that does not apply** is `null`. A list with nothing in it is `[]`.
- **Counts** are integers, and **shares** such as `pct_with_timings` are fractions between 0 and 1.
- **Order** inside a list is the order of the text output.

## Fields by command

??? example "Every command's result, as printed by --json"

    <!-- output: schema-keys.txt from="summary:" cmd -->
    ```console
    $ for c in summary hosts subsystems endpoints duplicates retries errors redirects slowest transitions timeline storms pagination rate-limit jwt auth handoff diff checks diagnose startup cascade validate; do printf '%s: ' $c; wiretrail checkout.har $c --json --top 2 | jq -r '.result | to_entries | map(.key + (if (.value|type)=="array" and (.value|length)>0 and (.value[0]|type)=="object" then " [" + (.value[0]|keys_unsorted|join(", ")) + "]" else "" end)) | join(", ")'; done
    summary: total_entries, filtered_entries, duration_ms, start_ms, end_ms, resource_breakdown, status_classes, error_count, top_hosts [host, count], top_duplicates [fingerprint, count, example_id], slowest [id, method, host, norm_path, status, duration_ms], biggest_payloads [id, host, norm_path, bytes], hints, recommendations [severity, kind, title, detail, evidence_ids, command, filter]
    hosts: hosts [host, count, methods, status_classes, error_count, p50_ms, p95_ms, max_ms, bytes_sent, bytes_received, duplicate_count, first_offset_ms, last_offset_ms]
    subsystems: subsystems [name, owner, criticality, count, hosts, error_count, duplicate_count, first_offset_ms, last_offset_ms]
    endpoints: endpoints [host, method, norm_path, count, statuses, content_types, sample_query_keys, error_count]
    duplicates: groups [fingerprint, count, method, host, norm_path, statuses, entry_ids, first_offset_ms, last_offset_ms, is_retry_pattern]
    retries: groups [fingerprint, method, host, norm_path, attempts, retry_count, trigger_statuses, gaps_ms, entry_ids, final_status]
    errors: groups [host, method, norm_path, status, count, error_message, error_code, body_snippet, correlation_ids, entry_ids, first_offset_ms, last_offset_ms]
    redirects: groups [host, method, norm_path, status, count, target_host, cross_host, is_storm, entry_ids, first_offset_ms, last_offset_ms]
    slowest: entries [id, method, host, norm_path, status, duration_ms, phases, bottleneck]
    transitions: transitions [host, method, norm_path, from_status, to_status, from_id, to_id, gap_ms, label]
    timeline: rows [id, offset_ms, duration_ms, method, host, norm_path, status, bytes, correlation_id, marker]
    storms: storms [scope_kind, scope, peak_count, window_ms, first_offset_ms, last_offset_ms, calls_per_sec, entry_ids]
    pagination: pages [host, method, norm_path, param_keys, page_count, repeated_cursor, excessive, entry_ids], nplus1 [host, method, norm_path, fanout, parent_id, first_offset_ms, last_offset_ms, entry_ids]
    rate-limit: groups [host, norm_path, count_429, retry_after_secs, ratelimit_headers, cooldown_violated, violating_ids, entry_ids]
    jwt: tokens [token_hash, source, summary, occurrences, first_entry_id, last_entry_id, raw_token]
    auth: failures [host, norm_path, status, count, entry_ids], missing_auth_hosts, token_changes [host, distinct_tokens], refreshes [id, host, status, success, concurrent, old_token_reused, reusing_ids]
    handoff: items [id, method, host, norm_path, status, timestamp, offset_ms, correlation_ids, server_ip, curl]
    diff: groups [host, method, norm_path, count, varying_query, varying_header_names, body_verdict, entry_ids]
    checks: findings [rule, host, norm_path, detail, entry_ids]
    diagnose: findings [severity, kind, title, detail, evidence_ids, suggested_command]
    startup: window_ms, requests_in_window, max_concurrency, critical_path_ms, critical_path [id, method, host, norm_path, offset_ms, duration_ms, status], slowest [id, method, host, norm_path, offset_ms, duration_ms, status]
    cascade: first_failure, cascades [trigger_id, trigger_kind, downstream_failures, downstream_ids]
    validate: har_version, creator, entry_count, pct_with_timings, pct_with_resp_body, pct_post_with_req_body, with_auth, with_cookies, anomalies [kind, count], sanitized, sufficiency_notes
    ```

    <!-- output: schema-keys-args.txt from="show-entry" cmd -->
    ```console
    $ for spec in 'show-entry e000076' 'curl e000076' 'search internal' 'extract $.error.code' 'compare baseline.har' 'report' 'rules --pack payments' 'auto'; do printf '%s: ' "$spec"; wiretrail checkout.har $spec --json --top 2 | jq -r '.result | to_entries | map(.key + (if (.value|type)=="array" and (.value|length)>0 and (.value[0]|type)=="object" then " [" + (.value[0]|keys_unsorted|join(", ")) + "]" else "" end)) | join(", ")'; done
    show-entry e000076: id, index, method, url, host, norm_path, status, status_text, http_version, server_ip, resource_type, content_type, started_offset_ms, duration_ms, query, req_headers, resp_headers, correlation, timings, req_body_snippet, resp_body_snippet
    curl e000076: commands [id, safe, label, command]
    search internal: matches [id, location, snippet]
    extract $.error.code: values [id, value]
    compare baseline.har: new_hosts, removed_hosts, new_endpoints, removed_endpoints, new_errors [endpoint, status, count, severity], latency_regressions [endpoint, base_p50_ms, new_p50_ms, severity], payload_growth [endpoint, base_bytes, new_bytes, severity], max_severity
    report: markdown
    rules --pack payments: findings [rule, severity, detail, entry_ids]
    auto: drilldowns [command, detail, evidence_ids, filter, kind, result, severity, title], not_drilled, summary
    ```

A name in square brackets is the fields of one element of that list, and a name with no brackets is a single field. A few fields have their own structure, listed here:

| Field | Structure |
|---|---|
| `slowest[].phases` | `blocked`, `dns`, `connect`, `ssl` (each a number or `null`), `send`, `wait`, `receive` (numbers). |
| `jwt.tokens[].summary` | `alg`, `typ`, `iss`, `aud`, `sub_hash`, `iat`, `nbf`, `exp`, `expired`, `seconds_to_expiry`, `clock_skew_hint`. Each can be `null`. |
| `show-entry` `timings` | The same seven phases as `slowest`. |
| `show-entry` `query`, `req_headers`, `resp_headers`, `correlation` | A list of `[name, value]` pairs. |
| `cascade.first_failure` | `null`, or `id`, `status`, `host`, `norm_path`, `before_ids`, `after_ids`. |
| `auto` | `summary` (the `summary` result), `drilldowns` (each with `severity`, `kind`, `command`, `filter`, `title`, `detail`, `evidence_ids` and the drill-down's own `result`), `not_drilled` (the same without `evidence_ids` and `result`). |
| `rate-limit.groups[].ratelimit_headers` | An object of header name to value, for the `x-ratelimit-*` headers. |
| `hosts.hosts[].methods`, `status_classes`, `endpoints.endpoints[].statuses` | An object from name to count. |

## Stability

`schema_version` is `1`, and WireTrail keeps the envelope and the meaning of existing fields. New fields can appear, so a script should read the fields it needs by
name and ignore the rest. A field is not removed without a new `schema_version`.

## Related

- [JSON output](../automate/json-output.md) shows how to use them.
- [Command line](command-line.md) lists the commands.

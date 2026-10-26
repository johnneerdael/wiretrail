# JSON output

Add `--json` to any command and it prints one JSON document instead of text. The document has the same
outer shape for every command, so a script can handle them all the same way.

```bash
wiretrail checkout.har errors --top 1 --json | jq '{tool, schema_version, command, warnings, next_commands, capture}'
```

<!-- output: json-envelope.txt -->
```text
{
  "tool": "wiretrail",
  "schema_version": 1,
  "command": "errors",
  "warnings": [],
  "next_commands": [
    "transitions",
    "redirects",
    "show-entry"
  ],
  "capture": {
    "har_version": "1.2",
    "creator": "WireTrail demo generator (synthetic)",
    "creator_version": "1",
    "browser": null,
    "entry_count": 99,
    "start_ms": 1792487642000,
    "end_ms": 1792487711953,
    "duration_ms": 69953.0
  }
}
```

## The envelope

| Field | Meaning |
|---|---|
| `tool` | Always `"wiretrail"`. |
| `schema_version` | The version of the envelope. It is `1` for every command. |
| `command` | The command that produced it. |
| `capture` | A description of the capture: the HAR version, the program that wrote it, the entry count, the start and end in Unix milliseconds, and the duration. |
| `result` | The command's own result. Each command has its own fields. |
| `warnings` | A list reserved for notices. It is empty in this version. |
| `next_commands` | The commands the text output suggests next, as a list. |

The `result` is different for each command, and [JSON schemas](../reference/json-schemas.md) lists them. The envelope does not
change, and neither does the meaning of a field once it exists.

## Clean for machines

The output has no color codes, no logo, no banner and no trailing text. It begins with `{`:

```bash
wiretrail checkout.har summary --json | head -c 1
```

<!-- output: json-first-char.txt -->
```text
{
```

WireTrail does not write ANSI escape sequences in text output either, so `NO_COLOR` makes no difference to it.

## Errors are not JSON

When a command stops with exit code `2`, the message goes to **stderr** as plain text, and stdout is empty. A script
that pipes `--json` into `jq` should check the exit code of the first command:

```bash
wiretrail checkout.har errors --json --filter "colour:red"
```

<!-- output: json-bad-filter.txt from="wiretrail:" -->
```text
wiretrail: invalid filter: unknown filter key: colour
```

```bash
wiretrail checkout.har errors --json --filter "colour:red" 2>/dev/null | wc -c | tr -d ' '
```

<!-- output: json-bad-filter-out.txt from="0" -->
```text
0
```

## Follow a finding

Entry ids let a script take its next step without searching. Take the first error group, open its first request:

```bash
wiretrail checkout.har errors --json | jq -r '.result.groups[0].entry_ids[0]'
```

<!-- output: json-pivot.txt from="e000" -->
```text
e000069
```

```bash
wiretrail checkout.har show-entry "$(wiretrail checkout.har errors --json | jq -r '.result.groups[0].entry_ids[0]')" --json | jq '.result | {id, status, norm_path}'
```

<!-- output: json-pivot-entry.txt -->
```text
{
  "id": "e000069",
  "status": 503,
  "norm_path": "/v1/cart/sync"
}
```

## Count and aggregate

`jq` turns a result into a number for a threshold or a metric:

```bash
wiretrail checkout.har errors --json | jq '[.result.groups[].count] | add'
```

<!-- output: json-total.txt from="16" -->
```text
16
```

Sixteen error responses in all, the same figure as the summary's hint.

## The next commands

`next_commands` is the same list the text output prints after "next useful commands". A script can read it to decide what
to run:

```bash
wiretrail checkout.har summary --json | jq '.next_commands'
```

<!-- output: json-next.txt -->
```text
[
  "auth",
  "diff",
  "errors",
  "rate-limit",
  "retries",
  "storms"
]
```

## Which commands have a result

Every command has `--json`, with two differences. `report` puts its markdown in one string, `result.markdown`. `export` ignores
`--json` and prints its raw rows. `auto` nests the other commands' results; see [Auto](../analyze/auto.md#json).

## Exit code

`--json` does not change the exit code. A command that finds something exits `1` whether it prints text or JSON.

## Related

- [JSON schemas](../reference/json-schemas.md) lists the fields of each command.
- [Gate a build](ci-gates.md) uses the exit codes.
- [Work with exported data](export-data.md) for flat rows.

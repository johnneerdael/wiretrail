# Export

`export` writes one row per entry, with the numbers and names that describe the request and none of its
content. The output is plain data for `jq`, a spreadsheet, DuckDB or a script. It has no headers, cookies or
bodies in it.

```bash
wiretrail checkout.har export --filter "status:>=500" --top 1
```

<!-- output: exp-ndjson.txt head="3" -->
```text
{"id":"e000054","offset_ms":13000.0,"duration_ms":221.0,"method":"GET","host":"api.shop.example","norm_path":"/v1/inventory/{id}","status":502,"bytes":56,"content_type":"application/json","resource_type":"api","correlation":"req_52f70893f4"}
{"id":"e000069","offset_ms":25000.0,"duration_ms":243.0,"method":"POST","host":"api.shop.example","norm_path":"/v1/cart/sync","status":503,"bytes":83,"content_type":"application/json","resource_type":"api","correlation":"req_fa6f840121"}
{"id":"e000070","offset_ms":26200.0,"duration_ms":210.0,"method":"POST","host":"api.shop.example","norm_path":"/v1/cart/sync","status":503,"bytes":83,"content_type":"application/json","resource_type":"api","correlation":"req_3ad0e75ca3"}
```

That command printed six rows, not one, and the block shows the first three. See [`--top` and `--json` are ignored](#-top-and-json-are-ignored).

## The fields

| Field | Meaning |
|---|---|
| `id` | The [entry id](../concepts/entries-ids-endpoints.md). |
| `offset_ms` | When the request started, in milliseconds from the start of the capture. |
| `duration_ms` | How long it took. |
| `method`, `host` | The HTTP method and the host. |
| `norm_path` | The [normalized path](../concepts/entries-ids-endpoints.md), with ids and opaque blobs replaced. |
| `status` | The status code, `0` for no response. |
| `bytes` | The response size: the larger of the content size and the body size, and `0` when the capture does not know it. |
| `content_type` | The response content type, or `null`. |
| `resource_type` | `api`, `media`, `static`, `analytics`, `document` or `other`. |
| `correlation` | The first correlation id from the response headers, or `null`. |

Because it keeps only these fields, `export` is safe to share **as far as these fields are**: a host name
or a path can still tell a reader something, and `content_type` and `correlation` are copied from the response headers as they are. No request header, cookie, body or query value leaves the tool.

## NDJSON or CSV

`--format` chooses. The default is NDJSON, one JSON object per line. `--format csv` writes a header and one row
per entry:

```bash
wiretrail checkout.har export --format csv --filter "status:>=500"
```

<!-- output: exp-csv.txt from="id,offset_ms" -->
```text
id,offset_ms,duration_ms,method,host,norm_path,status,bytes,content_type,resource_type,correlation
e000054,13000,221,GET,api.shop.example,/v1/inventory/{id},502,56,application/json,api,req_52f70893f4
e000069,25000,243,POST,api.shop.example,/v1/cart/sync,503,83,application/json,api,req_fa6f840121
e000070,26200,210,POST,api.shop.example,/v1/cart/sync,503,83,application/json,api,req_3ad0e75ca3
e000072,28700,184,POST,api.shop.example,/v1/cart/sync,503,83,application/json,api,req_67572df288
e000073,33900,234,POST,api.shop.example,/v1/cart/sync,503,83,application/json,api,req_44df04452c
e000076,42100,192,POST,api.shop.example,/v1/cart/sync,500,69,application/json,api,req_1c776b9fff
```

A field that contains a comma, a quote or a line break is quoted, with quotes doubled. In CSV, `offset_ms` and
`duration_ms` are whole numbers, and in NDJSON they carry a decimal. The CSV output ends with one empty line.

## Put it to work

`jq` turns the rows into answers. Total time by host:

```bash
wiretrail checkout.har export | jq -s 'group_by(.host) | map({host: .[0].host, requests: length, ms: (map(.duration_ms) | add)}) | sort_by(-.ms) | .[0:3]'
```

<!-- output: exp-jq.txt -->
```text
[
  {
    "host": "api.shop.example",
    "requests": 63,
    "ms": 11797
  },
  {
    "host": "cdn.shop.example",
    "requests": 15,
    "ms": 4585
  },
  {
    "host": "payments.shop.example",
    "requests": 3,
    "ms": 1333
  }
]
```

[Work with exported data](../automate/export-data.md) has more.

## `--top` and `--json` are ignored

`export` always writes every entry that matches the filter, and it always writes raw rows. Neither `--top`
nor `--json` changes that:

```bash
wiretrail checkout.har export --top 3 | wc -l | tr -d ' '
```

<!-- output: exp-top.txt -->
```text
99
```

`--json` prints the same NDJSON rows, with no envelope:

```bash
wiretrail checkout.har export --json --filter "status:>=500" | head -1 | jq 'keys'
```

<!-- output: exp-json.txt -->
```text
[
  "bytes",
  "content_type",
  "correlation",
  "duration_ms",
  "host",
  "id",
  "method",
  "norm_path",
  "offset_ms",
  "resource_type",
  "status"
]
```

To limit the rows, use `--filter`, or `head` on the output.

## Exit code

`export` always exits `0`.

## Related

- [Work with exported data](../automate/export-data.md) for recipes with `jq`.
- [Filter entries](../analyze/filters.md) selects the rows.
- [JSON schemas](../reference/json-schemas.md) lists the fields of every other command.

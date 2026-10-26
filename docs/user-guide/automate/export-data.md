# Work with exported data

`export` turns a capture into flat rows that any tool can read. This page shows what to do with them in `jq` and in a
spreadsheet. The same rows load into any tool that reads NDJSON or CSV.

```bash
wiretrail checkout.har export | jq -s 'map(select(.duration_ms > 1000)) | map({id, host, norm_path, duration_ms})'
```

<!-- output: ed-slow.txt -->
```text
[
  {
    "id": "e000009",
    "host": "cdn.shop.example",
    "norm_path": "/img/hero.webp",
    "duration_ms": 1921.0
  },
  {
    "id": "e000032",
    "host": "api.shop.example",
    "norm_path": "/v1/products/{id}/price",
    "duration_ms": 1009.0
  },
  {
    "id": "e000059",
    "host": "cdn.shop.example",
    "norm_path": "/img/banner.webp",
    "duration_ms": 1900.0
  },
  {
    "id": "e000090",
    "host": "api.shop.example",
    "norm_path": "/v1/orders",
    "duration_ms": 2409.0
  },
  {
    "id": "e000092",
    "host": "api.shop.example",
    "norm_path": "/v1/uploads/avatar",
    "duration_ms": 1104.0
  }
]
```

`jq -s` reads the whole stream as one array. That is how a group-by or a sort works across entries.

## Count by status

```bash
wiretrail checkout.har export | jq -s 'group_by(.status) | map({status: .[0].status, requests: length}) | sort_by(-.requests) | .[0:4]'
```

<!-- output: ed-status.txt -->
```text
[
  {
    "status": 200,
    "requests": 72
  },
  {
    "status": 302,
    "requests": 6
  },
  {
    "status": 401,
    "requests": 4
  },
  {
    "status": 429,
    "requests": 4
  }
]
```

## Bytes by host

```bash
wiretrail checkout.har export | jq -s 'group_by(.host) | map({host: .[0].host, bytes: (map(.bytes) | add)}) | sort_by(-.bytes) | .[0:3]'
```

<!-- output: ed-bytes.txt -->
```text
[
  {
    "host": "cdn.shop.example",
    "bytes": 3015095
  },
  {
    "host": "api.shop.example",
    "bytes": 61335
  },
  {
    "host": "shop.example",
    "bytes": 19840
  }
]
```

Nearly all of the bytes (97%) come from the CDN, and most of those from one 2.1 MiB image. `bytes` is the response size, so this is download weight and
not upload.

## Median and maximum for one host

```bash
wiretrail checkout.har export --filter "host:api.shop.example" | jq -s 'map(.duration_ms) | sort | {requests: length, median: .[length / 2 | floor], max: .[-1]}'
```

<!-- output: ed-percentile.txt -->
```text
{
  "requests": 63,
  "median": 85.0,
  "max": 2409.0
}
```

`jq` has no percentile function, so this indexes a sorted list. The `hosts` command prints p50 and p95 itself, with
a method that is documented on [Hosts, subsystems and endpoints](../analyze/inventory.md#hosts).

## A spreadsheet

`--format csv` opens in any spreadsheet. Filter first, so the sheet holds only what you need:

```bash
wiretrail checkout.har export --format csv --filter "host:payments.shop.example"
```

<!-- output: ed-csv.txt from="id,offset_ms" -->
```text
id,offset_ms,duration_ms,method,host,norm_path,status,bytes,content_type,resource_type,correlation
e000086,63800,887,OPTIONS,payments.shop.example,/v1/charges,204,0,,other,
e000087,64000,201,POST,payments.shop.example,/v1/charges,402,70,application/json,api,req_e4cd9f9aab
e000088,66500,245,POST,payments.shop.example,/v1/charges,200,34,application/json,api,req_466e1b8780
```

The file ends with an empty line, which some tools read as a row with no fields. Remove it if yours does: `wiretrail capture.har export --format csv | sed '$d'`.

## Join with other data

`id`, `offset_ms` and `correlation` are the keys. A correlation id joins a request to the server's own log line, and `offset_ms`
plus the capture start time in `capture.start_ms` gives a wall-clock time to join with other timelines. `capture.start_ms` is in the
envelope of any command's `--json` output.

## What is not in the rows

`export` leaves out headers, cookies, bodies and query values. It also leaves out the status text, the HTTP version, the
server address and the timing phases. For those, use [`show-entry`](../inspect/show-entry.md) on one entry, or the JSON of
[`slowest`](../failures/slowest.md).

## Related

- [Export](../inspect/export.md) lists the fields.
- [JSON output](json-output.md) for the richer per-command results.
- [Handle captures safely](../capture/handle-captures-safely.md) for what is safe to put in a shared sheet.

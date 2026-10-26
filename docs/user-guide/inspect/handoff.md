# Handoff

`handoff` writes a block for each failing and each slowest request with what a backend engineer needs to find it
in their own logs: the time in UTC, the correlation ids, the server address and a masked `curl` command. Send them the
blocks and not the capture.

```bash
wiretrail checkout.har handoff --filter "path:/v1/cart/sync" --top 1
```

<!-- output: hand-cart.txt from="# e000069" until="# e000070" -->
```text
# e000069 [503] POST api.shop.example/v1/cart/sync
  time: 2026-10-20T09:14:27+00:00 (+25000ms)
  correlation: req_fa6f840121, e1e598c1a6e1ae1d-AMS
  server ip: 203.0.113.30
  curl -X POST 'https://api.shop.example/v1/cart/sync' \
    -H 'accept: application/json, text/plain, */*' \
    -H 'accept-encoding: gzip, deflate, br' \
    -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36' \
    -H 'origin: https://shop.example' \
    -H 'referer: https://shop.example/' \
    -H 'authorization: <redacted>' \
    -H 'x-app-version: 4.12.0' \
    -H 'content-type: application/json' \
    -H 'content-length: 48' \
    --data '{"cart":"c_55d1","items":[{"qty":1,"sku":1042}]}'
```

## Which requests get a block

Every request that is an **error** (4xx, 5xx or no status), **plus** the `--top` slowest requests, in time order. `--top` counts the
slowest requests to add. It does not cap the number of errors. In this capture 16 requests failed, and `--top 3` gives
18 blocks, because two of the three slowest did not fail:

```bash
wiretrail checkout.har handoff --top 3 | grep -c '^# e'
```

<!-- output: hand-count.txt -->
```text
18
```

`--filter` narrows both sets, so `--filter "path:/v1/cart/sync" --top 1` gives every failure on that route and the slowest one among them. Without `--top 1` it adds the ten slowest requests of the filtered set.

## Read a block

| Line | Meaning |
|---|---|
| `# e000069 [503] POST api.shop.example/v1/cart/sync` | The entry id, status, method and route. |
| `time:` | The start of the request in UTC, and its offset in milliseconds from the start of the capture. |
| `correlation:` | The correlation ids from the response, the same as [`errors`](../failures/errors.md) shows. |
| `server ip:` | The address that answered, when the capture has it. |
| the `curl` command | A [masked replay](curl.md), indented. |

The time is exact to the millisecond when the request started on a fractional second and is otherwise shown to the second.
`correlation` and `server ip` are what let the other team look in the right log and find the right machine.

## JSON

```bash
wiretrail checkout.har handoff --filter "path:/v1/cart/sync" --top 1 --json | jq '.result.items[0] | del(.curl)'
```

<!-- output: hand-json.txt -->
```text
{
  "id": "e000069",
  "method": "POST",
  "host": "api.shop.example",
  "norm_path": "/v1/cart/sync",
  "status": 503,
  "timestamp": "2026-10-20T09:14:27+00:00",
  "offset_ms": 25000.0,
  "correlation_ids": [
    "req_fa6f840121",
    "e1e598c1a6e1ae1d-AMS"
  ],
  "server_ip": "203.0.113.30"
}
```

The `curl` field, left out here, holds the command as one string.

## Exit code

`handoff` exits `1` when it writes at least one block and `0` when it writes none.

## Related

- [curl](curl.md) explains the command in each block.
- [Hand a failure to the backend team](recipes/hand-a-failure-to-the-backend-team.md) puts it together.
- [Report](report.md) gives the overview to go with it.

# Pagination and N+1

`pagination` looks for two shapes in a capture: a walk through pages of a list, and a fan-out that
asks for one request per item (an **N+1**). Both are normal in small amounts and expensive in large ones.

```bash
wiretrail checkout.har pagination
```

<!-- output: pg-default.txt from="pagination sequences" until="next useful" -->
```text
pagination sequences:
  4 pages  GET api.shop.example/v1/products  (by limit,page) [repeated-cursor]

N+1 fan-out:
  12x  GET api.shop.example/v1/products/{id}/price  (after e000025)
```

## Pagination sequences

A sequence is a group of two or more calls to the same method, host and normalized path that carry a
pagination parameter, where **nothing else changes** between them. WireTrail looks for these parameter
names: `page`, `offset`, `cursor`, `page_token`, `after`, `before`, `start`, `limit`, `p`, `pagenumber` and
`page_number`.

`(by limit,page)` lists the pagination parameters found. The tags say what looks odd:

| Tag | Meaning |
|---|---|
| `[repeated-cursor]` | A value of one of those parameters appears more than once across the calls. |
| `[excessive]` | There are more pages than `--max-pages`, which defaults to `20`. |

`--max-pages 3` makes the four-page walk excessive:

```bash
wiretrail checkout.har pagination --max-pages 3
```

<!-- output: pg-max.txt from="4 pages" until="N+1 fan-out" -->
```text
  4 pages  GET api.shop.example/v1/products  (by limit,page) [repeated-cursor, excessive]
```

!!! warning "`[repeated-cursor]` is easy to trigger"
    `limit` is one of the pagination parameters, and a page size that stays the same repeats on every
    call. A healthy walk of three different pages with `limit=24` is tagged `[repeated-cursor]`, as it is
    in the known-good capture:

    ```bash
    wiretrail baseline.har pagination
    ```

    <!-- output: pg-baseline.txt from="pagination sequences" until="next useful" -->
    ```text
    pagination sequences:
      3 pages  GET api.shop.example/v1/products  (by limit,page) [repeated-cursor]
    ```

    The small capture `quirks/constant-limit.har` has three different pages and one fixed page size,
    and it gets the same tag. Read the `page` values yourself with `timeline` before you decide that a
    loop exists. [Limitations](../reference/limitations.md#pagination-flags-a-constant-page-size) lists
    this with the others.

## N+1 fan-out

An N+1 is a request for a list followed by one request for each item in it, when one request for all
of them would do. WireTrail reports a route as an N+1 when:

- its normalized path contains `{id}` or `{blob}`, so it is a per-item route, and
- at least `--fanout-min` calls (default `5`) fall within `--window-ms` (default `2000`).

`(after e000025)` names the closest earlier call to the same host that was **not** a per-item route.
It is the likely list request, but it is the nearest call, not a proven parent. Here it is the last
of four product-list calls, and the twelve price calls follow it:

```bash
wiretrail checkout.har timeline --filter "path:/v1/products*" --top 20
```

<!-- output: pg-timeline.txt from="4.1s" through="4.3s" -->
```text
    4.1s    558ms  e000025 GET api.shop.example/v1/products  [200] 30 B DUP
    4.2s     97ms  e000026 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
    4.3s    133ms  e000027 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
```

`--fanout-min 20` raises the bar, and the N+1 no longer qualifies:

```bash
wiretrail checkout.har pagination --fanout-min 20
```

<!-- output: pg-fanout.txt from="pagination sequences" until="next useful" -->
```text
pagination sequences:
  4 pages  GET api.shop.example/v1/products  (by limit,page) [repeated-cursor]
```

## What to do with an N+1

Count the calls and compare with the time they took. [Spot an N+1](recipes/spot-an-n-plus-one.md) goes
through it: twelve calls in 660 ms, fired in parallel, one of them waiting 880 ms in a queue. The fix is a batch endpoint, as
the known-good capture uses.

## JSON

```bash
wiretrail checkout.har pagination --json | jq '.result.nplus1[0] | del(.entry_ids)'
```

<!-- output: pg-json.txt -->
```text
{
  "host": "api.shop.example",
  "method": "GET",
  "norm_path": "/v1/products/{id}/price",
  "fanout": 12,
  "parent_id": "e000025",
  "first_offset_ms": 4200.0,
  "last_offset_ms": 4860.0
}
```

The result has two lists, `pages` and `nplus1`.

## Exit code

`pagination` exits `1` when it reports any sequence or any N+1, and `0` otherwise. A normal paged
list therefore makes it exit `1`.

## Related

- [Spot an N+1](recipes/spot-an-n-plus-one.md) works through the burst.
- [Storms](storms.md) finds the same burst by timing.
- [Duplicates](duplicates.md) shows a per-item route as one repeated group.

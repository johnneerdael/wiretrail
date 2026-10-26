# Slowest calls

`slowest` lists the longest requests and, for each one, where the time went: waiting on the server,
connecting, uploading, downloading, or queueing in the client. The label tells you what to look at
next.

```bash
wiretrail checkout.har slowest --top 7
```

<!-- output: slow-default.txt from="2.4s" until="next useful" -->
```text
    2.4s  e000090 POST api.shop.example/v1/orders  [201]
  bottleneck: server wait/TTFB
  phases: wait 2.4s / receive 8ms / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms

    1.9s  e000009 GET cdn.shop.example/img/hero.webp  [200]
  bottleneck: download/receive
  phases: wait 120ms / receive 1.8s / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms

    1.9s  e000059 GET cdn.shop.example/img/banner.webp  [0]
  bottleneck: TCP connect
  phases: wait 0ms / receive 0ms / send 0ms / connect 1.9s / dns 30ms / ssl 0ms / blocked 0ms

    1.1s  e000092 POST api.shop.example/v1/uploads/avatar  [201]
  bottleneck: request upload
  phases: wait 150ms / receive 4ms / send 950ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms

    1.0s  e000032 GET api.shop.example/v1/products/{id}/price  [200]
  bottleneck: queueing/blocked
  phases: wait 124ms / receive 4ms / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 880ms

   887ms  e000086 OPTIONS payments.shop.example/v1/charges  [204]
  bottleneck: DNS
  phases: wait 25ms / receive 1ms / send 1ms / connect 220ms / dns 640ms / ssl 140ms / blocked 0ms

   803ms  e000021 POST o0.ingest.sentry.io/api/{id}/envelope/  [200]
  bottleneck: TCP connect
  phases: wait 60ms / receive 2ms / send 1ms / connect 700ms / dns 40ms / ssl 520ms / blocked 0ms
```

## Read a row

```text
    2.4s  e000090 POST api.shop.example/v1/orders  [201]
  bottleneck: server wait/TTFB
  phases: wait 2.4s / receive 8ms / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms
```

The first line is the total time, the entry id, the method, host and path, and the status. `bottleneck`
names the **largest single phase**. `phases` lists all seven. The phases come from the HAR `timings`
object; [Timing phases](../concepts/timing-phases.md) defines each.

| Label | The largest phase | Usually means |
|---|---|---|
| `queueing/blocked` | `blocked` | The client waited before sending, often because too many requests were in flight. |
| `DNS` | `dns` | Looking up the host name was slow. |
| `TCP connect` | `connect` | Setting up the connection was slow. See below. |
| `TLS handshake` | `ssl` | The encryption setup was slow. See below. |
| `request upload` | `send` | Sending the request body took long. |
| `server wait/TTFB` | `wait` | The server took long to start answering. |
| `download/receive` | `receive` | Reading the response took long. |
| `unknown` | none | Every phase is zero. |

The list is sorted by total time and then by id. `--top` bounds it.

The seven rows above show six different labels. The order waits 2.4 seconds on the server, the hero image takes 1.8
seconds to download, the avatar upload spends 950 ms sending, a price call queues 880 ms in the client, the payment
preflight spends 640 ms on DNS, and the failed banner and the error-report call lose their time connecting. Each label
sends you somewhere different: the server team, the CDN, the upload size, the number of parallel requests, the resolver,
the network path.

!!! warning "A slow TLS handshake is labelled TCP connect"
    A HAR counts the TLS time **inside** `connect`, as its specification asks. `connect` should therefore never be
    smaller than `ssl`, and WireTrail picks `connect` whenever the two tie or `connect` is larger. So a call that spent 600 ms on
    TLS is labelled `TCP connect`. Look at the `ssl` figure in `phases`:

    ```bash
    wiretrail quirks/tls-in-connect.har slowest
    ```

    <!-- output: slow-tls.txt from="731ms" until="next useful" -->
    ```text
       731ms  e000000 GET tls.test.example/  [200]
      bottleneck: TCP connect
      phases: wait 20ms / receive 5ms / send 1ms / connect 700ms / dns 5ms / ssl 600ms / blocked 0ms
    ```

    Here 600 ms of the 700 ms of `connect` is `ssl`. `TLS handshake` appears only for a HAR that records `ssl` separately and larger.

## Failed requests

A request with no status has a total time and often only a `connect` or `dns` phase. In the list above, `GET
/img/banner.webp [0]` spent 1.9 seconds connecting and then failed. The label points at the connection, which is
where the time went, and not at the reason it failed.

## Ask for one host

```bash
wiretrail checkout.har slowest --filter "host:api.shop.example" --top 3
```

<!-- output: slow-api.txt from="2.4s" until="next useful" -->
```text
    2.4s  e000090 POST api.shop.example/v1/orders  [201]
  bottleneck: server wait/TTFB
  phases: wait 2.4s / receive 8ms / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms

    1.1s  e000092 POST api.shop.example/v1/uploads/avatar  [201]
  bottleneck: request upload
  phases: wait 150ms / receive 4ms / send 950ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms

    1.0s  e000032 GET api.shop.example/v1/products/{id}/price  [200]
  bottleneck: queueing/blocked
  phases: wait 124ms / receive 4ms / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 880ms
```

## The ranked finding

A **LOW** `slow-backend` finding appears when the single slowest call took more than a second **and** its label is
`server wait/TTFB`. It looks at that one call only, so a slow call that loses its time elsewhere raises none.

## JSON

```bash
wiretrail checkout.har slowest --top 1 --json | jq '.result.entries[0]'
```

<!-- output: slow-json.txt -->
```text
{
  "id": "e000090",
  "method": "POST",
  "host": "api.shop.example",
  "norm_path": "/v1/orders",
  "status": 201,
  "duration_ms": 2409.0,
  "phases": {
    "blocked": null,
    "dns": null,
    "connect": null,
    "ssl": null,
    "send": 1.0,
    "wait": 2400.0,
    "receive": 8.0
  },
  "bottleneck": "server wait/TTFB"
}
```

A phase that the HAR marks `-1`, meaning "does not apply", is `null` in `blocked`, `dns`, `connect` and `ssl`.

## Exit code

`slowest` always exits `0`.

```bash
wiretrail checkout.har slowest --top 1 > /dev/null; echo "exit code: $?"
```

<!-- output: slow-exit.txt -->
```text
exit code: 0
```

## Related

- [Timing phases](../concepts/timing-phases.md) defines the seven phases.
- [Why a call is slow](recipes/why-a-call-is-slow.md) follows one slow call to its cause.
- [Startup profile](startup.md) looks at the slow calls during the first seconds.

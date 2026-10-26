# Why a call is slow

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `slowest`, `show-entry`, `compare`, `handoff`</span>
<span><strong>Works on</strong> any capture with timings</span>
<span><strong>Time</strong> a few minutes</span>
<span><strong>Undo</strong> nothing to undo: every command only reads the capture</span>
</div>

## The problem

An order takes more than two seconds to place. You need to know whether the time is in the network, in the
client, or in the server, because the answer decides who gets the ticket.

## The solution

1. **List the slowest calls on the host.**

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

    `POST /v1/orders` took 2.4 seconds and almost all of it is `wait`, the time between sending the request and
    the first byte of the answer. Its `connect`, `dns` and `blocked` are absent from the capture (`slowest` prints them as 0 ms), so a connection was already open
    and the client sent the request at once. `bottleneck: server wait/TTFB` says the server was the slow party.

2. **Rule out the capture.** Time in the `wait` phase is the server's, but only if the capture measured it. Check that the
   phases are present:

    ```bash
    wiretrail checkout.har validate
    ```

    99% of requests have timings, so the figures can be trusted.

3. **Open the request.**

    ```bash
    wiretrail checkout.har show-entry e000090
    ```

    <!-- output: slow-entry.txt from="offset:" until="request headers" -->
    ```text
    offset: 67200ms  duration: 2409ms
    ```

    The offset places the call at 67.2 seconds into the capture, and the correlation ids in its response headers let the
    backend team find the request in their logs.

4. **Compare with the earlier build.** Was it always slow?

    ```bash
    wiretrail checkout.har compare baseline.har --filter "path:/v1/orders"
    ```

    <!-- output: slow-compare.txt from="latency regressions" until="next useful" -->
    ```text
    latency regressions:
      [medium] POST api.shop.example/v1/orders p50 319ms -> 2409ms
    ```

    The median time of this endpoint went from 319 ms to 2,409 ms. The regression is in this build, and not in the network.

5. **Hand it on.** [`handoff`](../../inspect/handoff.md) packages the id, the time, the correlation ids and a safe `curl` for
   the backend team. See [Hand a failure to the backend team](../../inspect/recipes/hand-a-failure-to-the-backend-team.md).

## How it works

The HAR splits each request into phases. If the largest is `wait`, the client has sent the request and is waiting on the
server. If it is `blocked`, the client held the request back. If it is `dns` or `connect`, the path to the server was
slow. The label in `slowest` is that largest phase, and `compare` tells you whether it changed.

## Variations

- **The time is in `blocked`.** The client queued the request. Look for too many parallel calls, as in [Spot an N+1](../../repeats/recipes/spot-an-n-plus-one.md).
- **The time is in `connect`.** The path to the server was slow, and part of it may be the TLS handshake. See the warning in [Slowest calls](../slowest.md).
- **The time is in `receive`.** The response was large or the link was slow. Check `biggest payloads` in [Summary](../../analyze/summary.md).
- **You have no baseline.** Use the figures in `phases` and the p95 from [`hosts`](../../analyze/inventory.md#hosts) as the yardstick.

## Related

- [Timing phases](../../concepts/timing-phases.md) defines each phase.
- [Slowest calls](../slowest.md) lists the labels.
- [Compare against a baseline](../../compare/compare.md) explains the latency rule.

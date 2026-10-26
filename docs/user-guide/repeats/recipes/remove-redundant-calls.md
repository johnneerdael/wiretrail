# Remove redundant calls

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `duplicates`, `diff`, `timeline`, `extract`</span>
<span><strong>Works on</strong> any capture with a repeated call</span>
<span><strong>Time</strong> a few minutes</span>
<span><strong>Undo</strong> nothing to undo: every command only reads the capture</span>
</div>

## The problem

A web app sends the same request every 1.5 seconds, and the capture is full of them. Each one costs
a request on the server, a wake-up on a phone's radio, and a line in every log. You want to know whether
they carry any information, and what to change.

## The solution

1. **Find the repeat.** `summary` showed `12x POST api.shop.example /v1/session/ping` in its duplicate list.

    ```bash
    wiretrail checkout.har duplicates --filter "path:/v1/session/ping"
    ```

    The group has twelve calls, all `200`, so none of them failed and none were retries.

2. **Look at the rhythm.** Are they a burst or a steady pulse?

    ```bash
    wiretrail checkout.har timeline --filter "path:/v1/session/ping" --top 5
    ```

    <!-- output: rr-ping-timeline.txt from="6.0s" until="next useful" -->
    ```text
        6.0s     52ms  e000038 POST api.shop.example/v1/session/ping  [200] 11 B DUP
        7.5s     33ms  e000041 POST api.shop.example/v1/session/ping  [200] 11 B DUP
        9.0s     30ms  e000042 POST api.shop.example/v1/session/ping  [200] 11 B DUP
       10.5s     50ms  e000050 POST api.shop.example/v1/session/ping  [200] 11 B DUP
       12.0s     42ms  e000052 POST api.shop.example/v1/session/ping  [200] 11 B DUP
    ```

    One call every 1.5 seconds, each answered in about 30 to 50 ms. A heartbeat.

3. **Ask what differs.** If the calls carry no new information, they are redundant.

    ```bash
    wiretrail checkout.har diff --filter "path:/v1/session/ping"
    ```

    <!-- output: diff-ping.txt from="POST api.shop.example/v1/session/ping" until="next useful" -->
    ```text
    POST api.shop.example/v1/session/ping  (12 calls, body: volatile-only)
    ```

    `volatile-only` means the bodies differ only in keys that look like clocks. No query parameter and no
    header changes. Check which key, because the verdict is broad (see [What varies between calls](../diff.md#body-verdicts)):

    ```bash
    wiretrail checkout.har extract '$.ts' --target req --filter "path:/v1/session/ping" --top 3
    ```

    <!-- output: diff-ping-ts.txt from="e000038" until="next useful" -->
    ```text
    e000038  1792487648000
    e000041  1792487649500
    e000042  1792487651000
    ```

    The only thing that changes is `ts`, a timestamp that moves by 1.5 seconds each time.

4. **Check the second kind of duplicate.** `/v1/flags` also repeats six times, with a different `cb` on
   each call:

    ```bash
    wiretrail checkout.har duplicates --filter "path:/v1/flags"
    ```

    <!-- output: rr-flags.txt from="6x" until="next useful" -->
    ```text
       6x  GET api.shop.example/v1/flags
      statuses: 200:6
      entries: e000019, e000040, e000051, e000061, e000066, e000071
    ```

    `cb` is a cache-buster, so these requests are asked as new ones each time and the server cannot answer from a
    cache. WireTrail ignores `cb` when it groups, which is how six different URLs became one group.

5. **Decide what to change.** The heartbeat carries no information beyond "still here". Send it less often, or send it only when
   the connection needs it. The flags call needs a stable URL and a cache header, so that the repeats are answered
   from cache. Then capture again.

6. **Check the result.** The earlier build did this better. Its capture has three pings, not twelve:

    ```bash
    wiretrail baseline.har duplicates --filter "path:/v1/session/ping"
    ```

    <!-- output: rr-baseline-ping.txt from="3x" until="next useful" -->
    ```text
       3x  POST api.shop.example/v1/session/ping
      statuses: 200:3
      entries: e000018, e000020, e000021
    ```

## How it works

`duplicates` finds the repeat and `timeline` shows whether it is a burst or a rhythm. `diff` separates a call that
changes from one that does not, and `extract` names the field. The `volatile-only` verdict is the key evidence:
with no query, header or body change that matters, the calls cannot be asking for different things.

## Variations

- **The calls are retries.** A group tagged `[retry pattern]` follows failures. See [Retries](../retries.md).
- **The calls differ in the path.** Twelve calls to `/v1/products/{id}/price` are one group after normalization, but they ask for
  twelve things. That is an [N+1](spot-an-n-plus-one.md), and `diff` shows nothing for it.
- **You cannot change the client.** Add `Cache-Control` on the server for the idempotent GETs, and measure with [`compare`](../../compare/compare.md).

## Related

- [Duplicates](../duplicates.md) explains the fingerprint.
- [What varies between calls](../diff.md) explains the verdicts.
- [Catch a regression before it ships](../../compare/recipes/catch-a-regression.md) keeps the improvement.

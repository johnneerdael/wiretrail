# Spot an N+1

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `pagination`, `storms`, `timeline`, `compare`</span>
<span><strong>Works on</strong> any capture with a list and per-item requests</span>
<span><strong>Time</strong> a few minutes</span>
<span><strong>Undo</strong> nothing to undo: every command only reads the capture</span>
</div>

## The problem

A product list screen feels slow. The list itself comes back quickly, but the prices appear one at a time. An **N+1**
is a request for the list followed by one more request per item. It is invisible when the list has three items and
painful when it has three hundred.

## The solution

1. **Ask `pagination` for fan-out.** It reports routes with an id in the path that were called many times in a short window.

    ```bash
    wiretrail checkout.har pagination
    ```

    <!-- output: pg-default.txt from="N+1 fan-out" until="next useful" -->
    ```text
    N+1 fan-out:
      12x  GET api.shop.example/v1/products/{id}/price  (after e000025)
    ```

    Twelve calls to one per-item route, and the call before them was a list request.

2. **See how tight the burst is.** `storms` looks at timing and not at shape:

    ```bash
    wiretrail checkout.har storms --filter "path:/v1/products/*/price"
    ```

    <!-- output: storm-endpoint.txt from="endpoint api.shop.example" until="next useful" -->
    ```text
    endpoint api.shop.example/v1/products/{id}/price  12 calls in 1.0s (12.0/s)
      window: 4.2s - 4.9s
      entries: e000026, e000027, e000028, e000029, e000030, e000031, e000032, e000033, e000034, e000035, e000036, e000037
    ```

    All twelve calls started within 0.7 seconds, so the app fired them in parallel.

3. **Read the sequence.** The timeline shows the list calls and then the burst:

    ```bash
    wiretrail checkout.har timeline --filter "path:/v1/products*" --top 20
    ```

    <!-- output: pg-timeline.txt from="4.1s" through="4.7s" -->
    ```text
        4.1s    558ms  e000025 GET api.shop.example/v1/products  [200] 30 B DUP
        4.2s     97ms  e000026 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.3s    133ms  e000027 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.3s    115ms  e000028 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.4s    102ms  e000029 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.4s     98ms  e000030 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.5s     96ms  e000031 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.6s     1.0s  e000032 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.6s    101ms  e000033 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
        4.7s    100ms  e000034 GET api.shop.example/v1/products/{id}/price  [200] 20 B DUP
    ```

    One of the twelve took 1.0 second while the others took about 100 ms. [Slowest calls](../../failures/slowest.md)
    explains why: that request spent 880 ms queued before it was sent, which is what a browser does when it has too many requests in flight.
    More items would make that worse.

4. **Compare with the known-good build.** The earlier capture asked for all twelve prices in one call:

    ```bash
    wiretrail baseline.har timeline --filter "path:/v1/prices"
    ```

    <!-- output: n1-baseline.txt from="4.0s" until="next useful" -->
    ```text
        4.0s    152ms  e000017 POST api.shop.example/v1/prices  [200] 132 B
    ```

    One `POST /v1/prices` with the ids in the body, answered in 152 ms. The change from one batched call to
    twelve single calls is the regression.

5. **Fix and confirm.** Ask for the prices in one request, then capture again. The burst should disappear:

    ```bash
    wiretrail baseline.har storms --min-count 10
    ```

    <!-- output: n1-storms-baseline.txt from="== wiretrail storms" until="next useful" -->
    ```text
    == wiretrail storms ==
    ```

## How it works

`pagination` recognizes an N+1 by its shape: a per-item route called many times close together, after a
list request. `storms` confirms by timing, and `timeline` shows the order. Normalization is what makes it
visible: the twelve prices have twelve different URLs, and WireTrail counts them as one endpoint.

## Variations

- **The items come in pages.** An N+1 per page multiplies. Check `pagination` for [excessive pages](../pagination.md) as well.
- **The burst is smaller than the threshold.** `--fanout-min` is 5 by default. Lower it to see groups of fewer than five calls.
- **The ids are not in the path.** A per-item call such as `GET /price?id=1042` has a query value and no path id, so the calls differ and
  [`duplicates`](../duplicates.md) does not group them. [`diff`](../diff.md) does group them by route, and lists the varying values as `query id varies`.

## Related

- [Pagination and N+1](../pagination.md) lists the flags and thresholds.
- [Storms](../storms.md) finds bursts of any kind.
- [Compare against a baseline](../../compare/compare.md) shows what a change did to a whole capture.

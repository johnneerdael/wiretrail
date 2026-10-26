# Catch a regression before it ships

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `compare`, `diagnose`, `--fail-on`</span>
<span><strong>Works on</strong> two captures of the same flow</span>
<span><strong>Time</strong> ten minutes to set up, seconds to run</span>
<span><strong>Undo</strong> nothing to undo: every command only reads the captures</span>
</div>

## The problem

A new build passes its tests, and it is slower and chattier than the one before. Nobody notices until users do. You want a check
that runs on a capture of a standard flow and fails when the flow got worse.

## The solution

1. **Keep a baseline.** Save a capture of the flow from a build you accept. This guide uses `baseline.har`.

2. **Capture the new build doing the same flow**, and compare.

    ```bash
    wiretrail checkout.har compare baseline.har
    ```

    <!-- output: cmp-default.txt from="max severity" until="new errors" -->
    ```text
    max severity: high
    new hosts: auth.shop.example, www.shop.example
    new endpoints: 4
    removed endpoints: 1
    ```

    The result is `high` because of the new 5xx errors further down. The new hosts and endpoints are listed, and they do not raise the severity.

3. **Read what the severity is made of.** The lists say what, and in which endpoint:

    <!-- output: cmp-default.txt from="new errors:" until="payload growth:" -->
    ```text
    new errors:
      [high] POST api.shop.example/v1/cart/sync -> 503 (5x)
      [medium] GET search.shop.example/v1/suggest -> 429 (4x)
      [medium] GET api.shop.example/v1/cart -> 401 (2x)
      [medium] GET api.shop.example/v1/auth/session -> 401 (1x)
      [medium] GET api.shop.example/v1/notifications -> 401 (1x)
      [medium] POST payments.shop.example/v1/charges -> 402 (1x)

    latency regressions:
      [medium] POST api.shop.example/v1/orders p50 319ms -> 2409ms
      [medium] GET cdn.shop.example/img/hero.webp p50 541ms -> 1921ms
      [medium] POST api.shop.example/v1/uploads/avatar p50 294ms -> 1104ms
      [medium] OPTIONS payments.shop.example/v1/charges p50 277ms -> 887ms
      [medium] GET api.shop.example/v1/products p50 128ms -> 558ms
    ```

    A new `503` on the cart sync, new `429`s from search, and an order that went from 319 ms to 2,409 ms. Each is an endpoint to
    open with [`slowest`](../../failures/slowest.md) or [`errors`](../../failures/errors.md).

4. **Add what `compare` cannot see.** It misses the number of calls. Run `diagnose` on the **new** capture, so a burst or a heartbeat
   is not lost:

    ```bash
    wiretrail checkout.har diagnose --top 20
    ```

    The ranked list holds the N+1, the identical heartbeat calls and the ignored rate limit, none of which moved `compare`.

5. **Choose the line.** Decide which finding fails the build:

    ```bash
    wiretrail checkout.har compare baseline.har --fail-on high > /dev/null; echo "exit code: $?"
    ```

    <!-- output: cmp-exit-high.txt -->
    ```text
    exit code: 1
    ```

    `--fail-on high` fails on a new 5xx and passes a slow endpoint. `--fail-on medium` fails on both. The exit code is the
    gate. See [Gate a build](../../automate/ci-gates.md) for the pipeline.

6. **Replace the baseline on purpose.** When a slower endpoint is the price of a feature, capture again and commit the new
   baseline in the same change, so the review sees it.

## How it works

`compare` compares medians, so one slow request does not trip it, and a change has to be consistent. It counts an endpoint's
errors as "new" only when the baseline had none for that endpoint, so a long-standing error does not fail every build.
Those two choices make the gate quiet, and the price is that it is blind to counts and tails.

## Variations

- **Two checks.** Run `compare` for what got worse and `diagnose` for what is wrong now. Both exit `1` on findings.
- **Only part of the flow.** `--filter "host:api.shop.example"` compares one host.
- **A noisy endpoint.** A filter keeps what matches and cannot exclude, so compare only the part you trust, for example `--filter "host:api.shop.example"` on both captures, or raise the bar with `--fail-on high`.

## Related

- [Compare against a baseline](../compare.md) has every rule and threshold.
- [Gate a build](../../automate/ci-gates.md) shows the pipeline step.
- [Diagnose](../../analyze/diagnose.md) is the other half of the check.

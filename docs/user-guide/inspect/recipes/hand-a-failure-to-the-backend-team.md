# Hand a failure to the backend team

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `errors`, `handoff`, `report`, `show-entry`</span>
<span><strong>Works on</strong> any capture with a failing request</span>
<span><strong>Time</strong> ten minutes</span>
<span><strong>Undo</strong> nothing to undo, but check what you paste before you send it</span>
</div>

## The problem

The cart does not save, and the cause looks like the server. The backend team does not want a 150 MB file. They want a time, an
id they can search for, and a request they can repeat. And you cannot attach the capture, because it holds credentials.

## The solution

1. **Describe the failure in the server's words.**

    ```bash
    wiretrail checkout.har errors --filter "path:/v1/cart/sync"
    ```

    <!-- output: first-errors.txt from="4x" until="next useful" -->
    ```text
       4x  [503] POST api.shop.example/v1/cart/sync
      message: inventory service unavailable
      code: upstream_unavailable
      correlation: req_fa6f840121, e1e598c1a6e1ae1d-AMS
      body: {"error":{"code":"upstream_unavailable","message":"inventory service unavailable"}}
      entries: e000069, e000070, e000072, e000073

       1x  [500] POST api.shop.example/v1/cart/sync
      message: internal server error
      code: internal_error
      correlation: req_1c776b9fff, 775f0aeadf5f3512-AMS
      body: {"error":{"code":"internal_error","message":"internal server error"}}
      entries: e000076
    ```

    Four 503s, `inventory service unavailable`, then a 500. The `correlation` ids are the first thing the team will search for.

2. **Write the blocks.**

    ```bash
    wiretrail checkout.har handoff --filter "path:/v1/cart/sync" --top 1
    ```

    Each block has a UTC time, the ids, the server address and a masked `curl`. Take the first and the last, the first 503 and the final 500:

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

3. **Add the overview.**

    ```bash
    wiretrail checkout.har report --filter "path:/v1/cart/sync" > cart-sync.md
    ```

    The report summarizes the filtered capture: how many calls, how many failed, how slow. The four lines at the top (creator, HAR version, entries, window) still describe the whole file.

4. **Check what you are about to send.** Read the blocks once. WireTrail masks what it recognizes, and a credential in an unusual header or a
   short secret in a body can get through. See [How redaction works](../../concepts/redaction.md#what-each-command-masks).

    ```bash
    wiretrail checkout.har handoff --filter "path:/v1/cart/sync" --top 1 | grep -i -E 'bearer|token|secret|password'
    ```

    No output means that none of those words appears. It does not prove there is nothing sensitive in the text.

5. **Write the ticket.** Use the output as the evidence:

    - **What**: `POST /v1/cart/sync` returned 503 four times, then 500. The client backed off from 1.2 s to 8.2 s.
    - **When**: the first failure at `2026-10-20T09:14:27Z`, the last 17 seconds later.
    - **Where**: server `203.0.113.30`, correlation ids from the blocks.
    - **Repeat it**: the masked `curl`. Replace the `authorization` value yourself.
    - **Not known**: why the server returned 503. The capture shows the status, not the cause.

## How it works

`handoff` takes every failing request and the slowest few, and for each writes only what another person needs to find the
call: the time, the ids and the address. It leaves out the body of the capture, and masks the credentials in the replay.

## Variations

- **The problem is speed, not failure.** Use [`slowest`](../../failures/slowest.md) first. `handoff` also includes the slowest
  requests, so the blocks for them are already there.
- **Several routes fail.** Run `handoff` once for each and send them together, or filter on the host.
- **They ask for the HAR.** Strip it first. See [Handle captures safely](../../capture/handle-captures-safely.md).

## Related

- [Handoff](../handoff.md) and [Report](../report.md) explain each output.
- [curl](../curl.md) explains the safety labels.
- [Why a call is slow](../../failures/recipes/why-a-call-is-slow.md) is the same idea for a slow call.

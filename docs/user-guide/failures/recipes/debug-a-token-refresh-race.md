# Debug a token refresh race

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `diagnose`, `auth`, `timeline`, `jwt`, `show-entry`</span>
<span><strong>Works on</strong> captures with an `Authorization` header and a refresh call</span>
<span><strong>Time</strong> ten minutes</span>
<span><strong>Undo</strong> nothing to undo: every command only reads the capture</span>
</div>

## The problem

An app signs in again after a token expires, and a few requests are rejected right after the refresh succeeds. This
is a **race**: calls that started before the refresh finished still carry the old token. Users see a flash of errors
that fixes itself, and it never shows up in a test that runs one request at a time.

## The solution

1. **See what WireTrail suspects.**

    ```bash
    wiretrail checkout.har diagnose --filter "host:*shop.example" --top 4
    ```

    <!-- output: race-diagnose.txt from="[HIGH] token-refresh-race" until="next useful" -->
    ```text
    [HIGH] token-refresh-race — suspicious token refresh on auth.shop.example
      refresh succeeded but later calls reused the old token
      evidence: e000014, e000015
      -> wiretrail <file> auth
    ```

    The finding says the refresh succeeded but later calls reused the old token, and names `e000014` and `e000015` as the
    evidence.

2. **Read the `auth` result.**

    ```bash
    wiretrail checkout.har auth
    ```

    <!-- output: auth-default.txt from="token refreshes" until="next useful" -->
    ```text
    token refreshes:
      e000014 auth.shop.example [200] [old-token-reused]
    ```

    `e000014` is the refresh, and `[old-token-reused]` says a later request still sent a token seen before it.

3. **Put the calls in order.** Show the first nine calls that carry an `x-request-id` response header:

    ```bash
    wiretrail checkout.har timeline --filter "has:resp.header.x-request-id" --top 9
    ```

    <!-- output: auth-race-timeline.txt from="900ms" until="next useful" -->
    ```text
       900ms    214ms  e000010 GET api.shop.example/v1/config  [200] 11.0 KiB
        1.0s     94ms  e000011 GET api.shop.example/v1/auth/session  [401] 58 B DUP
        1.1s     56ms  e000012 GET api.shop.example/v1/cart  [401] 58 B DUP
        1.2s     35ms  e000013 GET api.shop.example/v1/cart  [401] 58 B DUP
        1.3s    407ms  e000014 POST auth.shop.example/oauth/token  [200] 342 B
        1.3s     51ms  e000015 GET api.shop.example/v1/notifications  [401] 58 B DUP
        1.7s     83ms  e000016 GET api.shop.example/v1/auth/session  [200] 36 B DUP
        1.7s     53ms  e000017 GET api.shop.example/v1/cart  [200] 45 B DUP
        1.7s     84ms  e000018 GET api.shop.example/v1/notifications  [200] 12 B DUP
    ```

    The refresh `e000014` starts at 1.3 s and takes 407 ms. `e000015`, a call to `/v1/notifications`, starts in the same
    second, while the refresh is still running, and gets a 401. After the refresh completes, the same three
    calls succeed at 1.7 s.

4. **Confirm with the tokens.** The expired token should have a last use **after** the refresh began:

    ```bash
    wiretrail checkout.har jwt --json | jq '.result.tokens[] | {token_hash, source, occurrences, first_entry_id, last_entry_id, expired: .summary.expired}'
    ```

    <!-- output: jwt-json.txt from="8e37a634a25062a2" until="4bbf025fbfd0c0a4" -->
    ```text
      "token_hash": "8e37a634a25062a2",
      "source": "req.header.authorization",
      "occurrences": 4,
      "first_entry_id": "e000011",
      "last_entry_id": "e000015",
      "expired": true
    }
    {
      "token_hash": "fd2ef4823d488917",
      "source": "resp.body",
      "occurrences": 56,
      "first_entry_id": "e000014",
      "last_entry_id": "e000098",
      "expired": false
    }
    {
    ```

    The expired token appears four times, from `e000011` to `e000015`. The refresh is `e000014`, so `e000015` sent the old
    token after the refresh began. Together with the refresh response in `e000014`, which carries the new one, that
    confirms the race.

5. **Read the rejected call.**

    ```bash
    wiretrail checkout.har show-entry e000015
    ```

    <!-- output: auth-race-entry.txt from="response body" until="next useful" -->
    ```text
    response body: {"error":{"code":"token_expired","message":"JWT expired"}}
    ```

    The server said `token_expired`. The `authorization` header in the output is masked, which is why step 4 matters:
    `show-entry` cannot tell you which token a request used.

6. **Fix and check.** Make the client hold requests while a refresh is in flight, then send them with the new token. Capture
   again, and run `auth`: there should be no `[old-token-reused]`.

## How it works

WireTrail finds the refresh by its path, then asks two questions. Did a new token appear after it? Did a request after
the refresh began still use a token seen before it? Yes to both is a suspect refresh. `jwt` confirms by following each
token from its first use to its last.

## Variations

- **The refresh is not found.** WireTrail recognizes `/token`, `/oauth`, `/auth/refresh` or a `grant_type=refresh_token` query. A
  refresh in a form body, or on another path, is invisible to `auth`, but `jwt` and the timeline still work. See [Auth failures and refresh](../auth.md#what-it-does-not-see).
- **The flag is a false alarm.** If the later request went to a different host that has its own token, nothing is wrong. Check
  the host of `reusing_ids` in `auth --json`.
- **`concurrent` is set.** Another refresh started within this one's duration of its start, shortly before it or shortly after it, so the two overlapped or nearly did. The second may invalidate the first token. Serialize refreshes in the client.

## Related

- [Auth failures and refresh](../auth.md) explains the tags.
- [JWTs](../jwt.md) explains how expiry is judged.
- [How redaction works](../../concepts/redaction.md) says why `show-entry` masks the header.

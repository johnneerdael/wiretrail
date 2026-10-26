# Find the first failure

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `cascade`, `timeline`, `show-entry`, `transitions`, `jwt`</span>
<span><strong>Works on</strong> any capture that contains errors</span>
<span><strong>Time</strong> a few minutes</span>
<span><strong>Undo</strong> nothing to undo: every command only reads the capture</span>
</div>

## The problem

A capture holds sixteen errors, and they look unrelated: some 401s, a 502, several 503s, 429s, a failed
image. Reading them in the order of the error list tells you which are common. It does not tell you which came first,
and the first failure is often the one that makes the others make sense.

## The solution

1. **Ask which failed first.**

    ```bash
    wiretrail checkout.har cascade
    ```

    <!-- output: casc-default.txt from="first failure" until="next useful" -->
    ```text
    first failure: e000011 [401] api.shop.example/v1/auth/session
      before: e000008, e000009, e000010
      after:  e000012, e000013, e000014

    cascades:
      e000011 [auth] -> 3 downstream failures
        e000012, e000013, e000015
    ```

    The first failure is `e000011`, a 401 on `/v1/auth/session`, and it is followed by three more failures within
    five seconds. The trigger is tagged `[auth]` because of the path.

2. **See the neighbours.** The calls before it were a page load and a config request. Look at the host's timeline:

    ```bash
    wiretrail checkout.har timeline --filter "host:api.shop.example" --top 8
    ```

    <!-- output: casc-before.txt from="900ms" until="next useful" -->
    ```text
       900ms    214ms  e000010 GET api.shop.example/v1/config  [200] 11.0 KiB
        1.0s     94ms  e000011 GET api.shop.example/v1/auth/session  [401] 58 B DUP
        1.1s     56ms  e000012 GET api.shop.example/v1/cart  [401] 58 B DUP
        1.2s     35ms  e000013 GET api.shop.example/v1/cart  [401] 58 B DUP
        1.3s     51ms  e000015 GET api.shop.example/v1/notifications  [401] 58 B DUP
        1.7s     83ms  e000016 GET api.shop.example/v1/auth/session  [200] 36 B DUP
        1.7s     53ms  e000017 GET api.shop.example/v1/cart  [200] 45 B DUP
        1.7s     84ms  e000018 GET api.shop.example/v1/notifications  [200] 12 B DUP
    ```

    The config call at 900 ms is a 200, and then everything the app asks for after it fails with a 401. The app
    started and its first authenticated calls were rejected.

3. **Read the first failure in full.**

    ```bash
    wiretrail checkout.har show-entry e000011
    ```

    <!-- output: ff-entry.txt from="response body" until="next useful" -->
    ```text
    response body: {"error":{"code":"token_expired","message":"JWT expired"}}
    ```

    The server said `token_expired`, `JWT expired`. That explains the 401s as an expired credential. It does not yet say why
    the app used an expired token, but it narrows the question.

4. **Ask whether it recovered.** `transitions` shows what each failed call turned into:

    ```bash
    wiretrail checkout.har transitions --filter "path:/v1/cart"
    ```

    <!-- output: trans-cart.txt from="401 -> 200" until="next useful" -->
    ```text
    401 -> 200  [auth-recovered]  GET api.shop.example/v1/cart
      e000013 -> e000017  (gap 520ms)
    ```

    The cart call failed twice and then succeeded, so the failure was temporary and ended when the app got a new token.

5. **Check the token.** `jwt` decodes it:

    ```bash
    wiretrail checkout.har jwt
    ```

    <!-- output: jwt-default.txt from="8e37a634a25062a2" until="fd2ef4823d488917" -->
    ```text
    8e37a634a25062a2 (4x, req.header.authorization) [EXPIRED]
      iss: https://auth.shop.example/
      aud: shop-web
      sub (hashed): b1ca0ff720b2652d
      exp: 1792487342 (expired 301s ago)
    ```

    The first token had expired 301 seconds before its first use. The app held a token from an earlier session and
    refreshed it only after the first calls failed.

6. **Say what you know.** The first failure was an expired token at start-up, and it cleared once the app refreshed.
   That is a different problem from the 503s later, which `cascade` did not connect to it. They are separate causes with separate
   owners.

## How it works

`cascade` orders failures by time and reports the earliest. A cause tends to come before its effects, so the first failure is a
good place to start, though it is not always the cause: it can be the first symptom of something that began before the capture did.

## Variations

- **The first failure is a status 0.** Run `wiretrail checkout.har cascade` as usual: a request that failed with no status counts. Check
  `validate` for the capture quality.
- **No cascade, but many failures.** They are not clustered tightly enough to form a cascade at the defaults, which ask for 3 failures after the first within 5,000 ms. They may still share a cause: group them with [`errors`](../errors.md).
- **You want a wider net.** Lower `--min-downstream` and shorten `--window-ms` to see failures that cluster tightly.

## Related

- [Cascade](../cascade.md) explains the trigger kinds.
- [Debug a token refresh race](debug-a-token-refresh-race.md) follows the expired token to its replacement.
- [Errors](../errors.md) groups the failures by route.

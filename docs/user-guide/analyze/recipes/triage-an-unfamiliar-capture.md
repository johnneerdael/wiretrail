# Triage an unfamiliar capture

<div class="recipe-meta" markdown>
<span><strong>Uses</strong> `validate`, `diagnose`, `hosts`, `timeline`, `show-entry`</span>
<span><strong>Works on</strong> any HAR 1.2 or 1.3 capture</span>
<span><strong>Time</strong> a few minutes</span>
<span><strong>Undo</strong> nothing to undo: every command only reads the capture</span>
</div>

## The problem

Someone hands you a `.har` file and a complaint such as "checkout is flaky". You did not capture it,
you do not know the app, and you have no idea where to start. You want a method that does not depend
on guessing which command to run.

## The solution

1. **Check that the capture can answer the question.** An empty result from a capture with no bodies
   means nothing.

    ```bash
    wiretrail checkout.har validate
    ```

    Read the `sufficiency` notes if there are any. See [Validate a capture](../validate.md). If bodies
    or headers are missing, ask for a fresh capture before you read anything into the absence of findings.

2. **Get the ranked list.** `diagnose` prints the findings, most serious first.

    ```bash
    wiretrail checkout.har diagnose
    ```

    Take the first HIGH finding. Its `->` line is the command that investigates it. If you would rather
    not run the commands yourself, [`auto`](../auto.md) runs them.

3. **Find out who the findings are about.** Count requests and errors per host:

    ```bash
    wiretrail checkout.har hosts --top 3
    ```

    <!-- output: triage-hosts.txt from="api.shop.example" until="search.shop.example" -->
    ```text
    api.shop.example  (63 req, 10 err, 53 dup)
      latency p50/p95/max: 85ms / 560ms / 2.4s
      bytes sent/received: 980 B / 59.9 KiB
      methods: GET:36 OPTIONS:1 POST:25 PUT:1
      status: 2xx:53 4xx:4 5xx:6

    cdn.shop.example  (15 req, 1 err, 8 dup)
      latency p50/p95/max: 38ms / 1.9s / 1.9s
      bytes sent/received: 0 B / 2.9 MiB
      methods: GET:15
      status: 2xx:8 3xx:6 other:1
    ```

    One host carries most of the traffic and most of the errors. That tells you which team to ask.

4. **Put the failures in order.** A filtered timeline shows only the errors, in time order. The calls around them are left out:

    ```bash
    wiretrail checkout.har timeline --filter "status:>=400" --top 20
    ```

    <!-- output: triage-errors-timeline.txt from="1.0s" through="RETRY" head="10" -->
    ```text
        1.0s     94ms  e000011 GET api.shop.example/v1/auth/session  [401] 58 B
        1.1s     56ms  e000012 GET api.shop.example/v1/cart  [401] 58 B DUP
        1.2s     35ms  e000013 GET api.shop.example/v1/cart  [401] 58 B DUP
        1.3s     51ms  e000015 GET api.shop.example/v1/notifications  [401] 58 B
       13.0s    221ms  e000054 GET api.shop.example/v1/inventory/{id}  [502] 56 B
       25.0s    243ms  e000069 POST api.shop.example/v1/cart/sync  [503] 83 B DUP
       26.2s    210ms  e000070 POST api.shop.example/v1/cart/sync  [503] 83 B RETRY
    ```

    The 401s come first, at the start. The 503s that are retried come in the middle. The 429s come
    later. Three different problems happened at three different times, so they are probably three
    different causes. A fourth kind of failure has no status, and `status:>=400` misses it: see
    [A request with no status has status 0](../filters.md#a-request-with-no-status-has-status-0).

5. **Open one request from each finding.** Take an id from the `evidence` line:

    ```bash
    wiretrail checkout.har show-entry e000076
    ```

    You see the request and the response, with each body cut at 2,000 characters, the offset and the total duration of the call, and the correlation ids. The timing phases are in `--json`. See [Show an entry](../../inspect/show-entry.md).

6. **Write down what you know and what you do not.** For each finding, note the evidence ids, what
   the capture shows, and what it cannot show. A 503 shows that a server answered with an error. It does not
   show why.

## How it works

The order matters. `validate` stops you misreading silence. `diagnose` gives you a ranked list so you
look at the worst thing first, and `hosts` tells you where in the system it is. The timeline then
separates one problem from another, and `show-entry` gives you the raw request that proves it.

## Variations

- **The capture is large.** `diagnose` and `summary` take longer than the other commands on a big file.
  See [Performance](../../reference/performance.md). Filter first if you already know the host.
- **You have an earlier good capture.** Run [`compare`](../../compare/compare.md) first. It names what got worse.
- **The capture was stripped.** `validate` says so, and `auth`, `jwt`, `search` and `extract` will be limited.
- **You want to hand the result on.** [`report`](../../inspect/report.md) writes a markdown summary, and
  [`handoff`](../../inspect/handoff.md) writes what a backend team needs.

## Related

- [Summary](../summary.md) and [Diagnose](../diagnose.md) explain the output you read in steps 1 and 2.
- [Find the first failure](../../failures/recipes/find-the-first-failure.md) goes deeper on step 4.
- [Handle captures safely](../../capture/handle-captures-safely.md) before you pass the capture on.

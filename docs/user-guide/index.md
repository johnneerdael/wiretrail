---
hide:
  - navigation
  - toc
---

# WireTrail user guide { .md-visually-hidden }

<div class="wt-hero" markdown>

<div markdown>

<p class="wt-eyebrow">Command-line HAR analysis</p>

## Follow the request. Find the cause.

WireTrail reads HAR network captures from the command line. It groups repeated calls,
finds the failures and the slow requests, ranks the likely causes, and compares one
capture with another. Output is plain text, with JSON for scripts and CI. Most commands
take about half a second on a 150 MB capture.

[Start your first triage](getting-started/first-triage.md){ .md-button .md-button--primary }
[Install WireTrail](getting-started/install.md){ .md-button }

</div>

<!-- hero-capture: home-hero.txt -->
<div class="wt-terminal">
<div class="wt-terminal__bar"><i></i><i></i><i></i><b>HAR / Investigation</b></div>
<pre><code><span class="d"># Start with one question</span>
<span class="p">$</span> wiretrail checkout.har diagnose --top 3

<span class="hl">[HIGH] retry-exhaustion — 4 retries, final 500 on POST /v1/cart/sync</span>
  repeated retries did not recover
  evidence: e000069, e000070, e000072, e000073, e000076
  -&gt; wiretrail &lt;file&gt; retries
<span class="d">...</span>
<span class="hl">[HIGH] rate-limit-no-backoff — calls during 429 cooldown on search.shop.example /v1/suggest</span>
  4 429s, follow-ups before Retry-After elapsed
  evidence: e000077, e000078, e000079, e000080
  -&gt; wiretrail &lt;file&gt; rate-limit</code></pre>
<p class="wt-terminal__note">Real output from a demo capture</p>
</div>

</div>

!!! note "A command-line tool"
    WireTrail has no window to click through. Each investigation is one command that prints a
    report you can read in a terminal, paste into a ticket, or parse with `jq`. To watch traffic
    live or edit and resend a request, [use your browser's DevTools or a proxy](help/faq.md#when-should-i-use-devtools-or-a-proxy-instead).

## What do you want to do?

<div class="grid cards" markdown>

-   :wiretrail-file:{ .lg .middle } **Get a HAR**

    ---

    Save a capture from Chrome, Edge, Firefox or Safari, or export one from a proxy or a
    phone. Check that it holds what the analysis needs.

    [:octicons-arrow-right-24: Capture](capture/index.md)

    Popular: [From a browser](capture/browsers.md) · [Handle captures safely](capture/handle-captures-safely.md)

-   :wiretrail-search:{ .lg .middle } **See what happened**

    ---

    Start with a summary, let `auto` run the follow-ups, or ask `diagnose` for ranked
    findings with the evidence behind each.

    [:octicons-arrow-right-24: Analyze](analyze/index.md)

    Popular: [Summary](analyze/summary.md) · [Auto](analyze/auto.md) · [Diagnose](analyze/diagnose.md)

-   :wiretrail-clock:{ .lg .middle } **Find repeated calls**

    ---

    Duplicates, retries, bursts, pagination loops, N+1 fan-out and ignored rate limits,
    and what differs between calls that look the same.

    [:octicons-arrow-right-24: Repeats](repeats/index.md)

    Popular: [Duplicates](repeats/duplicates.md) · [Retries](repeats/retries.md) · [Rate limits](repeats/rate-limits.md)

-   :wiretrail-warning:{ .lg .middle } **Find what failed or was slow**

    ---

    Errors with their messages, redirects, status changes, the slowest calls and what
    they were waiting for, expired tokens and token refreshes.

    [:octicons-arrow-right-24: Failures](failures/index.md)

    Popular: [Errors](failures/errors.md) · [Slowest calls](failures/slowest.md) · [Auth failures and refresh](failures/auth.md)

-   :wiretrail-network:{ .lg .middle } **Look inside one request**

    ---

    Read one entry with secrets masked, search or extract from bodies, replay a call as
    `curl`, and write a report or a hand-off for another team.

    [:octicons-arrow-right-24: Inspect](inspect/index.md)

    Popular: [Show an entry](inspect/show-entry.md) · [curl](inspect/curl.md) · [Handoff](inspect/handoff.md)

-   :wiretrail-compare:{ .lg .middle } **Catch a regression**

    ---

    Compare a capture with a known-good one, enforce your own rules, and fail a build
    when the change is serious enough.

    [:octicons-arrow-right-24: Compare](compare/index.md)

    Popular: [Compare against a baseline](compare/compare.md) · [Rules](compare/rules.md)

-   :wiretrail-json:{ .lg .middle } **Script it or hand it to an agent**

    ---

    A stable JSON envelope, exit codes you can gate on, flat NDJSON and CSV exports,
    and a Claude Code plugin that runs the whole triage.

    [:octicons-arrow-right-24: Automate](automate/index.md)

    Popular: [JSON output](automate/json-output.md) · [Gate a build](automate/ci-gates.md)

-   :wiretrail-root:{ .lg .middle } **Understand the findings**

    ---

    How a HAR is read, how endpoints are grouped, what redaction does and does not do,
    and how findings are ranked.

    [:octicons-arrow-right-24: Concepts](concepts/index.md)

    Popular: [How redaction works](concepts/redaction.md) · [How findings are ranked](concepts/how-findings-are-ranked.md)

-   :wiretrail-terminal:{ .lg .middle } **Look something up**

    ---

    Every command and flag, the JSON fields, error messages and exit codes, measured
    speed, and what WireTrail cannot tell you.

    [:octicons-arrow-right-24: Reference](reference/index.md)

    Popular: [Command line](reference/command-line.md) · [Limitations](reference/limitations.md)

</div>

## New here?

1. [Install WireTrail](getting-started/install.md) with `cargo install wiretrail` or a
   prebuilt binary.
2. Work through [Your first triage](getting-started/first-triage.md), a real investigation
   from the first summary to the call that causes the trouble.
3. Keep [Choose the right command](getting-started/choose-a-command.md) open: it maps the
   question you have to the command that answers it.

No capture of your own yet? [Follow along with the demo captures](getting-started/follow-along.md),
which you can download and run every command on.

## When to use WireTrail, and when not to

| Reach for WireTrail when… | Reach for DevTools or a proxy when… |
|---|---|
| You have a finished capture and a question about it. | You want to watch traffic as it happens. |
| The capture is large, or the machine is a CI runner or a remote box. | You want to edit a request and send it again. |
| An agent or a pipeline should read the result. | You are exploring with no question yet. |

The two work well together. Capture in the browser or a proxy, then run WireTrail on the
exported `.har` file. Use the search box (press ++s++ or ++slash++) to find a command, a flag
or an error message.

## Get help and follow changes

Read what changed in each release in [What's new](getting-started/whats-new.md), or on the
[GitHub releases page](https://github.com/johnneerdael/wiretrail/releases). If a step in this
guide differs from your build, [report it on GitHub](https://github.com/johnneerdael/wiretrail/issues)
with your WireTrail version (`wiretrail --version`), your operating system and the command you ran.

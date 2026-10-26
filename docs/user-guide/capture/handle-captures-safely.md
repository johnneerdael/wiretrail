# Handle captures safely

A HAR is a recording of everything a session sent and received. In a real capture that includes session cookies, bearer tokens,
passwords in login bodies, API keys in URLs, and personal data in responses. Anyone who holds the file can use the credentials in it
until they expire, and can read everything else in it for as long as they keep it.

## Keep the file, share the findings

For most conversations you do not need to hand over the capture. You need to hand over what you found.

| Instead of the HAR, send | Command | What it holds |
|---|---|---|
| A summary of the problem | [`report`](../inspect/report.md) | Counts, hosts, errors with the server's message, slowest calls |
| What a backend team needs | [`handoff`](../inspect/handoff.md) | Times, correlation ids, server address and a masked `curl` for each failing call |
| One request in full | [`show-entry`](../inspect/show-entry.md) | Headers and bodies with known secrets masked |
| The ranked findings | [`diagnose`](../analyze/diagnose.md) | Findings with entry ids |

## What WireTrail masks, and what it does not

WireTrail masks a fixed set of headers, a fixed set of JSON keys, secret-looking query parameters, and anything that looks like a long random
token. It does not recognize every secret. `search` and `extract` mask the least, `errors`, `report`, `diagnose` and `summary` print the server's messages unmasked, and a JWT in an
unusual header can pass through. [How redaction works](../concepts/redaction.md) has the full list, command by command.

**Read the output once before you send it.** A search for the words that should not be there is a fast check:

```bash
wiretrail capture.har handoff | grep -i -E 'bearer|token|secret|password|key'
```

No output is encouraging and not a guarantee.

## If you must send a HAR

WireTrail does not write a sanitized HAR. It masks what it **prints**, and the file on disk is untouched. To send the capture itself, strip it
first. This `jq` program removes the headers that carry credentials, the cookies, and every request and response body:

```bash
jq '(.log.entries[] | .request.headers, .response.headers) |= map(select(.name | ascii_downcase | IN("authorization","cookie","set-cookie","proxy-authorization","x-api-key","x-device-token") | not)) | del(.log.entries[].request.cookies, .log.entries[].response.cookies, .log.entries[].request.postData, .log.entries[].response.content.text)' checkout.har > stripped.har && wiretrail stripped.har validate
```

<!-- output: safe-strip.txt from="HAR 1.2" until="anomalies" -->
```text
HAR 1.2 via WireTrail demo generator (synthetic)  (99 entries)
with timings: 99% · response bodies: 0% · POST req bodies: 0%
auth headers: false · cookies: false
sanitized: yes (no auth/cookies and few response bodies)
```

`validate` now says `sanitized: yes`, and a search for the header name finds nothing:

```bash
grep -c -i 'authorization' checkout.har stripped.har
```

<!-- output: safe-grep.txt -->
```text
checkout.har:59
stripped.har:0
```

What the program does **not** remove is as important:

- **Secrets in URLs.** A token in a query string stays:

    ```bash
    grep -o 'token=[a-z_0-9]*' stripped.har | head -1
    ```

    <!-- output: safe-query.txt -->
    ```text
    token=rcpt_demo_8f21
    ```

- **Credentials in headers you did not name.** The list above is a start. Look at the header names in your own capture and add the ones that carry tokens.
- **Personal data in URLs and paths**, such as an email address in a path.
- **Host names, paths and timing**, which can describe your system to an outsider.

A stripped capture is also a weaker one: with no bodies and no `Authorization` headers, `errors` shows no messages and `auth` shows nothing. Strip for
the reader, and keep the full capture for yourself.

## Where captures should live

- **Not in the repository** unless it is a deliberately sanitized fixture. Add `*.har` to `.gitignore`.
- **Not in a ticket attachment** that many people can open.
- **Not in a shared chat** where it will stay in the history.
- **Delete it** when the investigation is over.

If a capture with live credentials has left your control, treat those credentials as exposed and rotate them.

## Agents and CI

An AI agent that reads a capture has read all of it, and its replies can quote any of it. Run the agent on a stripped capture when you can. Keep
captures out of build logs: gate on exit codes, and do not print a capture's contents. See [The Claude Code plugin](../automate/claude-code-plugin.md) and
[Gate a build](../automate/ci-gates.md).

## Related

- [How redaction works](../concepts/redaction.md) lists exactly what is masked.
- [Handoff](../inspect/handoff.md) and [Report](../inspect/report.md) are the shareable outputs.
- [What a useful capture holds](what-a-capture-needs.md) for what you lose by stripping.

# Get started

WireTrail answers questions about a network capture. You give it a HAR file, which is what a browser,
a proxy or an app inspector exports, and it tells you what happened: which calls repeated, which
failed, which were slow, how a sign-in went, and what changed since the last capture. Each answer is
one command.

## What you need

- A capture in HAR format, version 1.2 or 1.3. [Capture](../capture/index.md) shows how to get one. If you do not
  have one, [use the demo captures](follow-along.md).
- WireTrail on your `PATH`. See [Download and install](install.md).

## The path through this section

1. [Download and install](install.md) takes a minute.
2. [Your first triage](first-triage.md) is a guided investigation of a real capture, from the first
   summary to the one request that matters.
3. [Follow along with the demo captures](follow-along.md) gives you the files, and an answer key for what is in them.
4. [Choose the right command](choose-a-command.md) maps the question you have to the command that answers it.
5. [What's new](whats-new.md) lists what each release added.

## How WireTrail works, in four sentences

WireTrail reads the capture once and groups its requests by host and by a normalized path, so
`/users/123` and `/users/456` count as one endpoint. Every command is a different question asked of
those groups. Output is plain text for people, and `--json` gives the same result in a stable
document for scripts. Credentials and secret-looking values are masked in most output, with
[limits you should know](../concepts/redaction.md).

## Related

- [Analyze](../analyze/index.md) is where most investigations start.
- [Concepts](../concepts/index.md) explains how a HAR is read and how findings are ranked.

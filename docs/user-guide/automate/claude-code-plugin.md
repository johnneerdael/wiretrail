# The Claude Code plugin

The repository doubles as a [Claude Code](https://claude.com/claude-code) plugin marketplace. Installing the plugin gives Claude a skill,
`analysing-har`, that switches on when you talk about a HAR capture, and a slash command, `/analysing-har`. Claude then runs WireTrail
for you, installs it if it is missing, and works through the same steps you would.

## Install it

In Claude Code, add the marketplace once, then install the plugin:

```text
/plugin marketplace add johnneerdael/wiretrail
/plugin install analysing-har@analysing-har
```

The plugin name and the marketplace name are both `analysing-har`, which is why the identifier reads `analysing-har@analysing-har`. Check
that it is installed:

```text
/plugin
```

## Use it

Describe the problem in plain words, or point at a file:

```text
/analysing-har path/to/capture.har
```

```text
Here is a HAR from the checkout flow. The cart does not save. What's going on?
```

The slash command takes the path and, optionally, a focus such as `errors`, `auth` or `duplicates`. The skill also switches on by
itself when the conversation mentions "analyse har", "debug har" or `wiretrail`, when a `.har` file appears in the conversation or the working
folder, and when you ask about request storms, duplicate calls, retries, errors, slow requests, token refresh, rate limiting, redirects,
searching bodies or comparing with a baseline.

## What the plugin tells Claude to do

The skill is a written set of defaults, so the agent behaves like an experienced user:

1. **Make sure WireTrail is installed**, and run `cargo install wiretrail` if it is not.
2. **Run `auto` first.** It prints the summary, ranks the problems and drills into the serious ones, which usually answers the whole
   question. `auto --all` adds the low-severity findings.
3. **Go deeper only where `auto` points,** or where you named a focus, using the `--filter` the recommendation shows.
4. **Report the evidence**: entry ids, counts, statuses and excerpts, so you can check them.
5. **Prefer `--json` and `jq`** to pick out fields, and never parse the HAR JSON by hand.
6. **Use `--unsafe-include-secrets` only** when you need a replayable request, and say so, because the output then holds live credentials.

The full text is in [`SKILL.md`](https://github.com/johnneerdael/wiretrail/blob/main/plugins/analysing-har/skills/analysing-har/SKILL.md)
and [`analysing-har.md`](https://github.com/johnneerdael/wiretrail/blob/main/plugins/analysing-har/commands/analysing-har.md). You can read
them, fork them, or paste them into another agent's instructions.

## Update and uninstall

```text
/plugin marketplace update analysing-har
/plugin update analysing-har@analysing-har
```

```text
/plugin uninstall analysing-har@analysing-har
/plugin marketplace remove analysing-har
```

## Working with an agent

Whichever agent runs `wiretrail`, a few habits make the result more useful and safer:

- **Give it the capture path and the question.** "Why does the cart sync fail?" is answerable. "Fix my app" is not.
- **Check what it quotes.** WireTrail masks the secrets it recognizes, and an agent that quotes `search` or `extract` output, or
  a header it does not know is secret, can repeat a credential. See [How redaction works](../concepts/redaction.md).
- **Keep captures private.** A capture holds session tokens, cookies and personal data. An agent that reads one has read all of it. See
  [Handle captures safely](../capture/handle-captures-safely.md).
- **Ask for evidence.** Entry ids let you open the same request with `show-entry` and check a claim.
- **Treat a finding as a lead.** A ranked finding says a pattern was seen. Whether it is a bug is your decision.

## Related

- [Choose the right command](../getting-started/choose-a-command.md) is the same map the skill follows.
- [JSON output](json-output.md) for what an agent should read.
- [Download and install](../getting-started/install.md) if you want WireTrail on your `PATH` without the plugin.

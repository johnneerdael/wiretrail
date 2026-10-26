# Analyze a capture

Most investigations start here. These commands give you the shape of a capture, rank what looks
wrong, and let you narrow in. Once you know which kind of problem you have, move to
[Repeats](../repeats/index.md), [Failures](../failures/index.md) or [Inspect](../inspect/index.md).

## Start with one of three

| Command | Use it when | Page |
|---|---|---|
| `summary` | You want the shape of the capture and a short list of next steps. It runs when you give no command. | [Summary](summary.md) |
| `auto` | You want WireTrail to run the follow-up analysis for the serious findings too. | [Auto](auto.md) |
| `diagnose` | You want only the ranked findings, with their evidence. | [Diagnose](diagnose.md) |

All three rank findings with the same logic, so they never disagree. The difference is how much
they print.

## Look around

| Command | Answers | Page |
|---|---|---|
| `hosts` | Which servers, how many requests, how fast, how many errors | [Hosts, subsystems and endpoints](inventory.md#hosts) |
| `subsystems` | The same, grouped by integration or owning team | [Hosts, subsystems and endpoints](inventory.md#subsystems) |
| `endpoints` | Which routes were called and how they answered | [Hosts, subsystems and endpoints](inventory.md#endpoints) |
| `timeline` | What happened, in order | [Timeline](timeline.md) |
| `validate` | Whether the capture holds what the analysis needs | [Validate a capture](validate.md) |

## Narrow it

Every command takes `--filter` to look at a subset, and `--top` to bound the length of its lists.
`validate`, `show-entry` and `curl <id>` ignore `--filter`. [Filter entries](filters.md) covers the clauses.

## Recipes

- [Triage an unfamiliar capture](recipes/triage-an-unfamiliar-capture.md) is the order of commands to
  run when someone hands you a capture and a vague complaint.

## Related

- [Choose the right command](../getting-started/choose-a-command.md) starts from your question.
- [How findings are ranked](../concepts/how-findings-are-ranked.md) explains the severity levels.

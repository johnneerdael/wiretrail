# Compare

These pages are for the question "is this capture worse than the last one?" and for the second question, "does it
break our rules?". `compare` measures a capture against a known-good one. `rules` and `checks` hold a capture to
conventions you write down. All three can fail a build.

| Command | The question | Page |
|---|---|---|
| `compare` | What got worse since the baseline: new errors, slower endpoints, bigger responses? | [Compare against a baseline](compare.md) |
| `rules` | Does the capture break my rules, or a built-in rule pack? | [Rules and rule packs](rules.md) |
| `checks` | Are required headers missing, or content types wrong? | [Built-in checks](checks.md) |

The rules and the subsystem names live in one optional file, described in [The configuration file](config-file.md).

## A baseline is a capture you trust

`compare` needs a second capture to measure against: the same flow, from an earlier build, that you
believe was fine. Keep it with your tests, and replace it when you accept a change on purpose. The demo capture
`baseline.har` plays that role in this guide.

## What none of them see

`compare` looks at errors, response time and size. It does not look at **how many** calls were made. A heartbeat that
grew from 3 calls to 12, or an N+1, does not move it. Run [`diagnose`](../analyze/diagnose.md) on the new capture too. The recipe below
does both.

## Recipes

- [Catch a regression before it ships](recipes/catch-a-regression.md) combines `compare`, `diagnose` and an exit code.

## Related

- [Gate a build](../automate/ci-gates.md) puts these in a pipeline.
- [Analyze](../analyze/index.md) for the single-capture view.

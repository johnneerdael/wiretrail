# Inspect

The other sections find what is wrong. These commands open one request, read inside bodies, turn a capture
into data, and package a result for another person or another tool.

| Command | What it does | Page |
|---|---|---|
| `show-entry` | Prints one request and response in full, with secrets masked | [Show an entry](show-entry.md) |
| `search` | Finds text inside request and response bodies | [Search bodies](search.md) |
| `extract` | Pulls one JSON field out of every body | [Extract a field](extract.md) |
| `export` | Flattens every entry into one NDJSON or CSV row | [Export](export.md) |
| `report` | Writes a markdown summary for a ticket or a document | [Report](report.md) |
| `curl` | Writes a masked `curl` command that repeats a request | [curl](curl.md) |
| `handoff` | Writes what a backend team needs to find the failing calls | [Handoff](handoff.md) |

## Before you share anything

These commands exist to produce output you can hand to someone else, so what they mask matters. WireTrail masks a
fixed set of headers, a fixed set of JSON keys, and values that look like random tokens. It does **not** mask a
credential it does not recognize. Two of these commands, `search` and `extract`, mask less than the others.
[How redaction works](../concepts/redaction.md) lists what each command does, and
[Handle captures safely](../capture/handle-captures-safely.md) says how to check before you send.

## Recipes

- [Hand a failure to the backend team](recipes/hand-a-failure-to-the-backend-team.md) builds a ticket from a failing call.

## Related

- [Failures](../failures/index.md) finds the requests worth inspecting.
- [JSON output](../automate/json-output.md) for the structured form of each result.

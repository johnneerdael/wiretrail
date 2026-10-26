# Performance

WireTrail maps the capture into memory, parses it once, and answers each command from the parsed records. Most commands take about half a second on a 150 MB capture. A few
do more work and take longer, and one has a cost that grows faster than the capture does.

These figures are from one machine and one synthetic file. They show the shape of the cost. Do not read them as a promise for your captures.

## How it was measured

- **Machine:** Apple M4 Pro, 48 GiB of memory, macOS 15.8, a release build (`lto = "fat"`) with Rust 1.99.
- **Version:** 0.3.1. The analysis code in 0.3.2 is the same.
- **File:** the 95 entries of `checkout.har` without its four 429 responses, repeated 1,000 times. That is **95,000 entries and 150 MB**. It is repetitive, but it parses like any other HAR.
- **Method:** each command run once with `--top 5`, wall-clock time and peak memory from `/usr/bin/time`. The script is `scripts/docs-measure.py`. It rebuilds the file and prints this table.

| Command | Time | Peak memory |
|---|---|---|
| `hosts` | 0.44 s | 494 MiB |
| `errors` | 0.42 s | 492 MiB |
| `duplicates` | 0.44 s | 491 MiB |
| `timeline` | 0.48 s | 492 MiB |
| `export` | 0.48 s | 528 MiB |
| `summary` | 1.70 s | 585 MiB |
| `diagnose` | 1.64 s | 537 MiB |
| `auto` | 5.01 s | 605 MiB |

## Reading the table

- **Loading dominates the simple commands.** About 0.4 s of each is reading and parsing 150 MB. A command that only groups what is already loaded adds little.
- **`summary`, `diagnose`, `auto` and `report` run the ranking.** The ranking runs several analyses, so these take about four times as long. `auto` then runs each drill-down, and takes about three times as long as `summary`.
- **Memory is about three and a half times the file** for the simple commands, and about four times for `summary`, `diagnose` and `auto`. The whole capture is held in memory, bodies included. A 1 GB capture needs more than 3 GB.

## The cooldown check scales with the 429s

`rate-limit` compares every limited response with every other call to the same route, and so it, and everything that runs it (`summary`, `diagnose`, `auto` and `report`), slows down fast
when one route has many 429s. On a capture of about 2,000 other entries:

| 429 responses on one route | `rate-limit` |
|---|---|
| 250 | 0.07 s |
| 500 | 0.08 s |
| 1,000 | 0.24 s |
| 2,000 | 1.38 s |
| 4,000 | 9.93 s |

Doubling the 429s from 2,000 to 4,000 multiplied the time by more than seven. A real capture has few 429s on one route, and the cost does not matter for it. A load test that records thousands
of rate-limit responses does, and `rate-limit --filter` or a narrower capture avoids it.

## Making it faster

- **Filter first.** `--filter` does not skip parsing, but it shrinks every analysis after it.
- **Use the narrow command.** `errors` and `hosts` cost a tenth of `auto` on the same file.
- **Cut the capture.** A capture of the flow that matters is smaller, quicker and safer to handle than an hour of browsing.
- **Do not ask `auto` for everything.** `auto --all` drills every finding, so it costs more than `auto`.

## Related

- [How WireTrail reads a HAR](../concepts/how-wiretrail-reads-a-har.md) says why memory follows file size.
- [Limitations](limitations.md#the-ranking-commands-slow-down-on-a-capture-with-many-429s) lists this with the other limits.
- [Rate limits](../repeats/rate-limits.md) explains the check.

# Captured output

Every block of command output in the user guide is quoted from a file in this folder. Nothing is typed by hand, so a page cannot show output that wiretrail did not print.

Each `<name>.txt` is one real run:

```text
$ wiretrail checkout.har errors --top 1     <- the command, as written in captures.txt
# exit 1                                    <- its exit status
== wiretrail errors ==                      <- everything it printed (stdout and stderr)
```

These files are committed. `scripts/docs-check.py` reads them, so CI can verify the guide without running the tool. Unlike a guide built on large private files, everything the captures need is committed too, so a fresh clone can regenerate them.

## The loop

```bash
cargo build --release                                   # the binary the guide describes
python3 scripts/docs-make-demo-hars.py                  # the demo captures (deterministic)
scripts/docs-workdir.sh /tmp/wt-work                    # short names -> the committed files
scripts/docs-capture.sh /tmp/wt-work                    # run every command in captures.txt
python3 scripts/docs-embed.py                           # rewrite the output blocks in the pages and the README
python3 scripts/docs-check.py                           # quotes, flags, commands, nav, voice, the product name
.venv/bin/mkdocs build --strict                         # links, anchors
```

`scripts/docs-capture.sh <workdir> <filter>` runs only the captures whose name contains the filter, which is what you want while writing one page.

### `captures.txt`

One line per capture, `name|max-lines|command`:

```text
first-summary|0|wiretrail checkout.har --top 5
```

- `name` becomes `outputs/<name>.txt`.
- `max-lines` keeps the first N lines of output (`0` keeps all).
- `wiretrail` in a command is the binary you built, wherever it appears: the runner puts it first on the `PATH`. Any other tool (`jq`, `grep`) runs as written, so a pipeline can quote its own result.
- The command runs with the work directory as its current folder. Some captures write a file there (`stripped.har`, `bom.har`) that a later capture reads, so run the file in order.
- Comment lines start with `#`.

### In a page

````markdown
<!-- output: first-summary.txt from="entries:" until="top hosts" -->
```text
...written by scripts/docs-embed.py...
```
````

The marker is followed by a fenced block, and `docs-embed.py` rewrites that block. The options are listed at the top of `scripts/docs-embed.py`: `from`, `until`, `through`, `head`, `keep` (do not rewrite, only verify; for blocks with `...` elisions) and
`cmd` (start the block with the command, for a `console` fence). Option values are in double quotes, so a value that contains one cannot be used. `docs-embed.py` also fills the README.

`docs-check.py` then checks that every line in the block exists in the capture, that every `--flag` is real, and that the command that produced the capture is shown in a `bash`, `sh` or `console` block on the same page. The terminal on the home page is hand-written
HTML, and a `<!-- hero-capture: name.txt -->` marker names the capture its lines must come from.

## What the work directory holds

`scripts/docs-workdir.sh` links these names to committed files:

| Name in the guide | File |
|---|---|
| `checkout.har`, `baseline.har`, `sanitized.har` | `assets/downloads/`, written by `scripts/docs-make-demo-hars.py` |
| `quirks/*.har` | `assets/downloads/quirks/`, small files that each show one behavior |
| `config/*.yaml`, `proj/wiretrail.yaml` | `_meta/fixtures/config/` |
| `empty.har`, `truncated.har`, `not-a-har.json` | made there, for the error captures |

## The demo captures

The demo captures are **synthetic**. `scripts/docs-make-demo-hars.py` writes them and draws no random numbers, so it writes the same bytes each time, and CI regenerates them and fails if they differ from what is committed. The guide quotes ids, counts and timings, so do not change
a scenario without re-running the loop and reading the diff. Each file says that it is synthetic in its `log.comment`, the hosts are in the reserved `.example` domain, the addresses are from the documentation ranges, and every token and key is made up.

## Output that varies

WireTrail prints no timing line and no clock, so output does not vary between runs. Two things can differ:

- The hash `jwt` prints for a token and a subject depends on the build of WireTrail. If a capture changes after a compiler update, that is why.
- The order of rows with equal counts is fixed by a rule (by count, then by name or id), so it does not vary.

If a capture changes after a re-run and you did not change the tool, read the diff before you accept it. A changed number on a page is either a real change in WireTrail or a mistake in the page.

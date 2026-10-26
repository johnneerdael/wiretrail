# WireTrail user guide: style guide

For anyone who writes or edits the guide. This file is not published (`exclude_docs` in `mkdocs.yml`).

## Who the guide is for

Someone with a HAR capture and a question: "what happened in this session?", "why is this slow?", "what changed since the last good build?". They know their app or their stack. They may never have read a HAR,
and may not know what a timing phase is. Write for that person, and assume nothing about HTTP internals beyond what a developer meets.

The guide does these jobs, and each page does one of them:

| Page type | Job | Where | Shape |
|---|---|---|---|
| Tutorial | Get a first result, in order | `getting-started/` | Numbered steps, real output after each, ends with **What you learned**. |
| How-to | Do one task with one command | `capture/`, `analyze/`, `repeats/`, `failures/`, `inspect/`, `compare/`, `automate/` | One command up front, then how to read the result, variations, **Related**. |
| Recipe | Solve a named problem from end to end | `*/recipes/` | The `recipe-meta` box, **The problem**, **The solution**, **How it works**, **Variations**, **Related**. |
| Concept | Explain an idea, with no commands to run | `concepts/` | A definition first, a diagram where it earns its place, **Related**. |
| Reference | Be complete and exact | `reference/` | Tables. No story. Each row says what to do when it matters. |

If a page tries to be two of these, split it.

## Voice

- Plain, direct, second person: "you", "run", "check". Short sentences. One idea per paragraph.
- Say what happens, then why. Lead with the result, not the history.
- Write WireTrail in prose and `wiretrail` only where it is the command: in a code span, a fence, a path or a URL. "Run `wiretrail` on the capture" and "WireTrail ranks the findings" are both right. `scripts/docs-check.py` fails on a lowercase
  name in a sentence, in the README as well as in the pages.
- Name things as WireTrail prints them: `[HIGH]`, `old-token-reused`, `bottleneck`. Bold a term once, where it is defined. Link to the glossary after that.
- Be exact about limits. If a flag does nothing, or a rule is a heuristic, say so on the page that meets it, and put it on `reference/limitations.md`. Do not soften it and do not hide it in a footnote.
- A finding is a pattern, not a diagnosis. Say what the capture shows and what it cannot show. "A 503 shows that a server answered with an error, and not why."
- Do not sell. No "powerful", "seamless", "blazing", "robust", "easy", "simply", "just", "leverage", "utilize". `scripts/docs-check.py` warns on these.
- Do not compare WireTrail with another tool to win. Say what it is not, and when to use DevTools or a proxy instead.
- No emoji in prose. No exclamation marks.
- Spell out abbreviations once in `_snippets/abbreviations.md` and let the tooltip do the work.

## Numbers and claims

- Every number in prose comes from a capture in `_meta/outputs/` or from a measurement written down on `reference/performance.md` (which `scripts/docs-measure.py` reproduces). Do not round a figure up, and do not quote one from memory.
- Timings are for the machine that measured them. Say "on a laptop" or give the figure, never "fast".
- **Verify a claim about behavior before you write it.** Run the command, or read the code. If it surprised you, it will surprise a reader: reproduce it with a file in `assets/downloads/quirks/` and add it to `reference/limitations.md`.
- Never state that output is "safe to share". Say what is masked and what is not, and link to `concepts/redaction.md`.

## Examples

- **Real output only.** Output blocks are written by `scripts/docs-embed.py` from `_meta/outputs/`, so they cannot drift. Never paste output by hand. See `_meta/outputs/README.md`.
- Name the captures as the guide does: `checkout.har` (the working example), `baseline.har` (the known-good one), `sanitized.har` (stripped of credentials and bodies), and the small files in `quirks/`. All are **synthetic**, from
  `scripts/docs-make-demo-hars.py`, and the guide says so where a reader first meets them.
- Use only reserved names in examples: `*.example` hosts, addresses from the documentation ranges, and made-up tokens. Never use a real host, a real account or a token from a real capture.
- Use a `bash` fence for a command you want copied, a `text` fence for output, and a `console` fence with the `cmd` option for a session that shows both.
- Show a command before its output, and name the thing to look at after it. Do not paste output and leave the reader to find the point.
- Keep a block short enough to read. Use `from=`, `until=`, `through=` or `head=` on the embed marker, and `...` on its own line for a deliberate gap.

## Page layout

- One `#` title per page, in sentence case, matching the nav label. Sections are `##`. Go to `###` only when a section has real parts.
- Open with what the feature does and when to use it, in two or three sentences. Put the command next.
- End with **Related** (links), **Next steps** (a path through the guide) or **What you learned** (tutorials). The checker warns when a page has none.
- Link with relative paths to the `.md` file, and to an anchor when the point is in a section. `mkdocs build --strict` fails on a broken link or anchor. A heading that starts with a code span has a leading hyphen in its anchor.
- Admonitions are rare, and never two in a row. `!!! note` is for something that surprises, `!!! tip` for a shortcut, `!!! warning` for something that costs time or gives a misleading answer, and `!!! danger` for something that loses data.

## Diagrams

Diagrams are inline SVG in `_snippets/`, included with `--8<--`. They take their colors from the page (classes in `stylesheets/wiretrail.css`), so they work in light and dark. Draw one only where a picture explains the mechanism better than
text: the timing phases are the one so far. Give each an accessible `<title>` and `<desc>`, and test at 390 px wide.

## Brand

The identity is the WireTrail 1.0 brand kit. `brand.md` in this folder says where each asset came from and how to update it. The rules that matter when you write a page:

| Token | Value | Use |
|---|---|---|
| Night | `#101820` | Header, footer, dark theme background, text on cyan |
| Paper | `#F3F6F5` | Light theme background |
| Signature | `#28C5DC` | The mark, the header rule, filled controls in the dark theme |
| Highlight | `#96EAF4` | The link between the blocks of the mark, the active tab |
| Light foreground | `#006779` | Links and filled controls on light surfaces (white text on it) |
| Dark link | `#6FDEEC` | Links on dark surfaces |

- Type: Space Grotesk for headings, Inter for reading, IBM Plex Mono for commands and data. All three are self-hosted from `assets/fonts/`.
- Scale 12 / 14 / 16 / 20 / 28 / 40 px, a 4 px spacing base, 4 px controls, 8 px panels. Material's `1rem` is 20 px.
- Status colors (note, tip, warning, danger) always sit next to a label. The product color is never a severity scale: `[HIGH]` is not teal because it is high.
- The mark is `assets/brand/wiretrail-mark-on-dark.svg`. Do not recolor, rotate, stretch or add effects to it.
- Supporting glyphs are the kit's 16 icons, written `:wiretrail-network:` and so on. Use them at 24 px, beside a readable label.
- The tagline is "Follow the request. Find the cause." Say what the tool found, what it means and what to inspect next, and keep claims grounded.

## Before you commit

```bash
cargo build --release
python3 scripts/docs-make-demo-hars.py          # the demo captures, byte for byte
scripts/docs-workdir.sh /tmp/wt-work
scripts/docs-capture.sh /tmp/wt-work            # run every command in captures.txt
python3 scripts/docs-embed.py --check           # no output block is stale
python3 scripts/docs-check.py                   # quotes, flags, commands, nav, voice, the product name
.venv/bin/mkdocs build --strict                 # links, anchors, nav
```

All of them must pass. When a change touches a command, run it against the demo captures and read the diff of `_meta/outputs/` before you accept it: a changed number on a page is either a real change in WireTrail or a mistake in the page.

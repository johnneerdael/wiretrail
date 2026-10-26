# Search bodies

`search` looks for text inside request and response bodies and prints a short excerpt around each match. Use it
when you know a word, an error message or an id that should be in a body, and you want to know which requests
carry it.

```bash
wiretrail checkout.har search "internal server error"
```

<!-- output: search-substr.txt from="e000076" until="next useful" -->
```text
e000076 (resp.body)
  …or":{"code":"internal_error","message":"internal server error"}}…
```

## How it matches

By default the pattern is a plain piece of text. Two flags change that:

| Flag | Effect |
|---|---|
| `--regex` | The pattern is a regular expression, in the Rust `regex` syntax. |
| `--ignore-case` | The match ignores case. |

```bash
wiretrail checkout.har search "JWT EXPIRED" --ignore-case --top 2
```

<!-- output: search-ci.txt from="e000011" until="next useful" -->
```text
e000011 (resp.body)
  …ror":{"code":"token_expired","message":"JWT expired"}}…

e000012 (resp.body)
  …ror":{"code":"token_expired","message":"JWT expired"}}…
```

A regular expression can match a shape. This finds the error code in the server errors:

```bash
wiretrail checkout.har search '"code":"[a-z_]+"' --regex --filter "status:>=500" --top 3
```

<!-- output: search-regex.txt from="e000054" until="next useful" -->
```text
e000054 (resp.body)
  …{"error":{"code":"bad_gateway","message":"bad gate…

e000069 (resp.body)
  …{"error":{"code":"upstream_unavailable","message":…

e000070 (resp.body)
  …{"error":{"code":"upstream_unavailable","message":…
```

A pattern that is not a valid regular expression stops the command with exit code `2` and the parser's message:

```bash
wiretrail checkout.har search "(" --regex
```

<!-- output: search-badregex.txt from="wiretrail:" -->
```text
wiretrail: invalid regex: regex parse error:
    (
    ^
error: unclosed group
```

## What it searches

- **Bodies only.** Request bodies and response bodies. Headers, URLs and query strings are not searched. For
  those, use `--filter`.
- **One match per body.** For each entry, WireTrail reports the first match in the request body and the first in the
  response body. It does not list every occurrence.
- **`--top` limits the matches**, default 10. The search stops after that many.
- **`--filter` applies first.** It limits which entries are searched.

Each result shows the entry id, whether the match is in `req.body` or `resp.body`, and a window that runs from 40 bytes before the start of the match to 40 bytes after the start of the match. A match
longer than that is cut short. `…` marks the ends of the window.

When nothing matches, the output has only the header and the exit code is still `0`:

```bash
wiretrail checkout.har search "no such text anywhere"
```

<!-- output: search-none.txt from="== wiretrail search" until="next useful" -->
```text
== wiretrail search ==
```

## What it masks

!!! warning "Search masks less than `show-entry`"
    The excerpt is masked only where a piece of text **looks like a random token**. It does not look at JSON key names, so a
    short secret next to your match is printed. On a login whose password and token are short:

    ```bash
    wiretrail quirks/short-secrets.har search hunter2
    ```

    <!-- output: search-short.txt from="e000000" until="next useful" -->
    ```text
    e000000 (req.body)
      …{"user":"ann","password":"hunter2"}…

    e000000 (resp.body)
      …{"ok":true,"password":"hunter2","token":"abc123","note":"passwo…
    ```

    `show-entry` on the same entry masks `password` and `token`. If you search for a word near a credential, the credential can
    be in the excerpt. See [How redaction works](../concepts/redaction.md#what-each-command-masks).

## JSON

```bash
wiretrail checkout.har search "declined" --json | jq '.result.matches'
```

<!-- output: search-json.txt -->
```text
[
  {
    "id": "e000087",
    "location": "resp.body",
    "snippet": "{\"error\":{\"code\":\"card_declined\",\"message\":\"Your card was decli"
  }
]
```

## Exit code

`search` exits `0` whether or not it finds anything, and `2` for a pattern it cannot read.

## Related

- [Extract a field](extract.md) pulls one JSON value from every body.
- [Show an entry](show-entry.md) opens a match in full.
- [Filter entries](../analyze/filters.md) narrows what is searched.

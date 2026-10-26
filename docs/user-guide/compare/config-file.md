# The configuration file

An optional YAML file gives WireTrail three kinds of knowledge it cannot read from a capture: which team owns which host,
which headers every request must carry, and which rules to enforce.

## Where it is read from

`--config <path>` names a file. If you do not pass it, WireTrail reads `wiretrail.yaml` from the **current folder**, when there
is one. Nothing is read from anywhere else. If there is no file, the defaults apply.

Only four commands use the file: `subsystems`, `report`, `checks` and `rules`. The others never open it, so a bad path does not
matter to them:

```bash
wiretrail checkout.har summary --config config/nope.yaml --top 1
```

<!-- output: cfg-none-needed.txt from="entries:" until="status classes" -->
```text
entries: 99 total, 99 after filter
duration (first start to last response): 70.0s
```

An explicit `--config` wins over a `wiretrail.yaml` in the folder.

## A complete file

```yaml
ownership:
  - name: Storefront API
    host: "api.shop.example"
    owner: web-platform
    criticality: high

required_headers:
  - host: "api.shop.example"
    headers: ["X-App-Version"]

rules:
  - name: "Storefront calls are fast enough"
    host: "api.shop.example"
    max_latency_ms: 500
  - name: "No tracking hosts in a checkout capture"
    host: "*.ingest.sentry.io"
    forbid: true
```

All three sections are optional.

## `ownership`

Names the subsystem a host or path belongs to, for [`subsystems`](../analyze/inventory.md#subsystems) and
[`report`](../inspect/report.md). Each entry has a `name`, a `host` or a `path` (or both), and an optional `owner` and `criticality`.
The first entry that matches wins. An entry with neither `host` nor `path` never matches.

| Key | Meaning |
|---|---|
| `name` | The subsystem name that is shown. |
| `host`, `path` | Globs. A request must match each one that is given. |
| `owner` | A team or person, shown in square brackets. |
| `criticality` | Stored with the subsystem and kept in the JSON. The text output does not show it. |

A request that matches no entry gets a built-in vendor name if its host is one of GitHub, Supabase, Google, TMDB, Cloudflare, AWS or Sentry, and
otherwise its own host name.

## `required_headers` and `rules`

See [Built-in checks](checks.md#required-headers) and [Rules and rule packs](rules.md#your-own-rules).

## Globs

Matching uses `*` for any run of characters and ignores case. There is no `?`, no character class and no regular
expression. `host: "*.ingest.sentry.io"` matches `o0.ingest.sentry.io`. A pattern with no `*` must match the whole value.

## Mistakes

A file that cannot be read, or is not valid YAML, stops the command with exit code `2` and a short message:

```bash
wiretrail checkout.har subsystems --config config/broken.yaml
```

<!-- output: cfg-broken.txt from="wiretrail:" -->
```text
wiretrail: failed to parse config YAML
```

```bash
wiretrail checkout.har subsystems --config config/nope.yaml
```

<!-- output: cfg-missing.txt from="wiretrail:" -->
```text
wiretrail: failed to read config file
```

The messages do not say which line is wrong. Check the YAML with another tool.

!!! warning "A misspelled key is ignored"
    WireTrail reads only the keys it knows, and silently skips the rest. A file with `ownershp:` instead of `ownership:` is
    valid, and does nothing:

    ```bash
    wiretrail checkout.har subsystems --config config/typo.yaml --top 2
    ```

    <!-- output: cfg-typo.txt from="api.shop.example" until="cdn.shop.example" -->
    ```text
    api.shop.example  [-]  (63 req, 10 err, 53 dup)
      window: 900ms - 69.8s
      hosts: api.shop.example
    ```

    The hosts keep their own names and there is no error. If a section seems to have no effect, check the spelling of the key first.

## Related

- [Hosts, subsystems and endpoints](../analyze/inventory.md#subsystems) shows `ownership` at work.
- [Rules and rule packs](rules.md) and [Built-in checks](checks.md) use the other two sections.
- [Command line](../reference/command-line.md) lists `--config`.

# Cascade

`cascade` finds the first failure in a capture, shows what was happening around it, and reports
any failure that was followed closely by several more. It is for "what broke first?" and "did one failure
take others down with it?".

```bash
wiretrail checkout.har cascade
```

<!-- output: casc-default.txt from="first failure" until="next useful" -->
```text
first failure: e000011 [401] api.shop.example/v1/auth/session
  before: e000008, e000009, e000010
  after:  e000012, e000013, e000014

cascades:
  e000011 [auth] -> 3 downstream failures
    e000012, e000013, e000015
```

## First failure

The first failure is the earliest request in time that is an error: a 4xx, a 5xx or no status. `before`
and `after` are the ids of the three requests on each side of it in start order. They include successes, so you see
the neighbours and not only other failures.

## Cascades

A cascade is a failed request followed, within a window, by at least a minimum number of further failures.
The failures after it are called **downstream**.

| Flag | Default | Meaning |
|---|---|---|
| `--window-ms` | `5000` | How long after the trigger a failure still counts as downstream. |
| `--min-downstream` | `3` | The fewest downstream failures that make a cascade. |

`e000011 [auth] -> 3 downstream failures` reads as: the 401 on the session call was followed by three more failures
within five seconds, and the ids are listed below it. The bracket is a guess at the **kind** of the trigger from its path:

| Kind | When the path contains |
|---|---|
| `config` | `/config` |
| `auth` | `/auth`, `/token` or `/oauth` |
| `bootstrap` | `bootstrap` or `/init` |
| `request` | anything else |

Cascades can overlap, because every failure is a possible trigger. Lowering the thresholds shows the chain
inside the one above:

```bash
wiretrail checkout.har cascade --window-ms 500 --min-downstream 2
```

<!-- output: casc-window.txt from="cascades:" until="next useful" -->
```text
cascades:
  e000011 [auth] -> 3 downstream failures
    e000012, e000013, e000015
  e000012 [request] -> 2 downstream failures
    e000013, e000015
```

The first cascade, from the session call, still holds. A second one, from the `/v1/cart` failure, holds two of
the same failures inside half a second.

!!! warning "Downstream means later, not caused"
    WireTrail has no dependency data. A failure counts as downstream only because it came after the trigger
    within the window. Two unrelated problems that happen close together look like a cascade. Check each downstream
    call, as [Find the first failure](recipes/find-the-first-failure.md) does.

## When nothing cascades

The known-good capture has two failures, a 502 and a banner request that failed with no status, and no cascade:

```bash
wiretrail baseline.har cascade
```

<!-- output: casc-none.txt from="first failure" until="next useful" -->
```text
first failure: e000030 [502] api.shop.example/v1/inventory/{id}
  before: e000027, e000028, e000029
  after:  e000031, e000032, e000033
```

`cascade` still exits `1`, because there is a first failure to report.

## JSON

```bash
wiretrail checkout.har cascade --json | jq '.result.cascades[0]'
```

<!-- output: casc-json.txt -->
```text
{
  "trigger_id": "e000011",
  "trigger_kind": "auth",
  "downstream_failures": 3,
  "downstream_ids": [
    "e000012",
    "e000013",
    "e000015"
  ]
}
```

`first_failure` in the JSON result holds the trigger's id, status, host, path and the `before_ids` and `after_ids`.

## Exit code

`cascade` exits `1` when the capture has any failure at all, and `0` when it has none.

## Related

- [Find the first failure](recipes/find-the-first-failure.md) works through the result.
- [Errors](errors.md) groups every failure with its message.
- [Status transitions](transitions.md) shows which failures recovered.

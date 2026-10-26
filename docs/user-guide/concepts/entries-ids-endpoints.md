# Entries, ids and endpoints

Three ideas hold the commands together: an **entry** is one request, an **id** names it, and an **endpoint** is the shape of a request with its variable
parts removed.

## Entries and ids

Each request in the capture is an entry. WireTrail numbers them in the order they appear in the file, starting at zero, and writes the number as `e` and six digits: `e000000`,
`e000001`, `e000069`.

- An id is stable. It does not change between commands, between runs or with `--filter`. Entry `e000069` is the same request everywhere, so you can find
  it in `errors`, open it with `show-entry` and replay it with `curl`.
- It depends on the **file order**, not on time. Most tools write entries in start order, so the two agree.
- A different file has different ids. An id from one capture means nothing in another.

`show-entry` and `curl` also accept the bare number: `14` and `e14` both mean `e000014`.

## Endpoints

Looking at every URL separately would hide the pattern. `/v1/products/1042/price` and `/v1/products/1043/price` are one thing asked twelve times. So WireTrail
replaces the parts of a path that identify a **thing** with a placeholder, and calls the result a **normalized path**.

| A path segment that is… | Becomes | Example |
|---|---|---|
| All digits, with some exceptions below | `{id}` | `1042` → `{id}` |
| A UUID | `{id}` | `550e8400-e29b-41d4-a716-446655440000` → `{id}` |
| Hexadecimal and at least 16 characters | `{id}` | `0123456789abcdef0123` → `{id}` |
| An opaque blob: base64-like or percent-encoded, long | `{blob}` | `eyJyZWdpb24iOiJubCIs…` → `{blob}` |
| Anything else | kept | `v1`, `products`, `cart` |

A single digit as the first segment is kept, so `/3/tv/popular` stays as it is. Every other all-digit segment is an id.

A **blob** is what hides a secret in a path, such as a configuration object encoded into the URL. The test is long: at least 16 characters, not all hex, not a UUID, and
either percent-encoded or in the base64 alphabet with a mix of cases, digits or `+/=` that slugs do not have. The demo capture has one:

```bash
wiretrail checkout.har endpoints --top 40 | grep -E '\{id\}|\{blob\}'
```

<!-- output: norm-examples.txt from="12" -->
```text
  12  GET api.shop.example/v1/products/{id}/price
   3  POST o0.ingest.sentry.io/api/{id}/envelope/
   2  GET api.shop.example/v1/inventory/{id}
   1  PUT api.shop.example/v1/cart/items/{id}
   1  GET api.shop.example/v1/receipts/{id}/download
   1  GET cdn.shop.example/cfg/{blob}/manifest.json
```

An **endpoint** is a method, a host and a normalized path. `GET api.shop.example/v1/products/{id}/price` is one endpoint, however many products were fetched.
`endpoints`, `compare`, `pagination` and `diff` group by it, and `duplicates` and `retries` use it as part of their key.

## The raw path is still there

Normalization changes what is **shown** and what commands group by. It does not change the request. `--filter "path:..."` matches the **raw** path as it appeared in the
URL, so to select the twelve price calls you write a `*` where the id goes:

```bash
wiretrail checkout.har export --filter "path:/cfg/*" | jq -r '[.id, .norm_path] | join("  ")'
```

<!-- output: norm-raw.txt -->
```text
e000008  /cfg/{blob}/manifest.json
```

`show-entry` prints the raw URL, with opaque path segments masked. See [How redaction works](redaction.md).

## What normalization gets wrong

Normalization is a pattern, and patterns misfire:

- **A number that is not an id.** `/page/2` and `/api/2024/items` become `/page/{id}` and `/api/{id}/items`. Pagination in the path folds into one endpoint.
- **An id that is not a number.** `/users/ann` and `/users/bob` stay as two endpoints, because a name looks like a word.
- **A slug with a long mix of characters** can be mistaken for a blob, and a short token is never mistaken for one.

When a grouping looks wrong, open one entry with `show-entry` and read the raw path.

## Query strings

The query is not part of the normalized path. Commands that care about it, such as `duplicates`, build a **fingerprint**: method, host, normalized path and the query pairs,
sorted, without the cache-buster keys `_`, `cb`, `cachebuster`, `nonce`, `ts` and `timestamp`. Two calls with the same fingerprint are the same call as far as WireTrail is
concerned. The body is not in the fingerprint.

## Related

- [Duplicates](../repeats/duplicates.md) uses the fingerprint.
- [Filter entries](../analyze/filters.md) explains `path:` against the raw path.
- [Hosts, subsystems and endpoints](../analyze/inventory.md#endpoints) shows the endpoint list.

#!/usr/bin/env python3
"""Generate the demo captures that the user guide runs wiretrail on.

Three files land in docs/user-guide/assets/downloads/:

  checkout.har    A 70-second session of a made-up web shop (shop.example) with the problems
                  the guide teaches you to find: an expired token and a refresh race, a request
                  storm and an N+1 fan-out, identical heartbeat calls, a retry that never
                  recovers, a rate limit that is ignored, redirects, and slow calls whose time
                  goes to a different phase each.
  baseline.har    The same session from an earlier build that did none of that. `compare` and the
                  CI gate pages use it as the known-good capture.
  sanitized.har   checkout.har as a tool that strips headers and bodies on export would write it.
                  The `validate` page uses it.

These are SYNTHETIC. Nothing here was recorded from a real service: the hosts use the reserved
.example top-level domain, the addresses are from the documentation ranges (RFC 5737), and the
tokens and keys are made-up strings that are not valid anywhere. Every file says so in
`log.comment`. The output the guide shows is real wiretrail output on these files.

The generator is deterministic: it draws no random numbers, so running it again writes the same
bytes. The guide quotes ids, counts and timings, so do not change a scenario without re-running
scripts/docs-capture.sh and scripts/docs-embed.py and reading the diff.

usage: python3 scripts/docs-make-demo-hars.py
"""
from __future__ import annotations

import base64
import hashlib
import json
import re
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "user-guide" / "assets" / "downloads"

START = datetime(2026, 10, 20, 9, 14, 2, tzinfo=timezone.utc)
T0 = int(START.timestamp())

IP = {
    "shop.example": "203.0.113.10",
    "www.shop.example": "203.0.113.10",
    "cdn.shop.example": "203.0.113.20",
    "api.shop.example": "203.0.113.30",
    "auth.shop.example": "203.0.113.31",
    "search.shop.example": "203.0.113.40",
    "payments.shop.example": "203.0.113.50",
    "o0.ingest.sentry.io": "198.51.100.7",
}
UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"
APP = "4.12.0"


def b64u(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def jwt(claims: dict) -> str:
    """A JWT-shaped token. The signature is a fixed hash of a label: it verifies nothing anywhere."""
    head = b64u(json.dumps({"alg": "HS256", "typ": "JWT"}, separators=(",", ":")).encode())
    body = b64u(json.dumps(claims, separators=(",", ":")).encode())
    sig = b64u(hashlib.sha256(b"wiretrail-docs-demo: not a real signature").digest())
    return f"{head}.{body}.{sig}"


SUB = "user_8f21c4"
OLD_TOKEN = jwt({"iss": "https://auth.shop.example/", "aud": "shop-web", "sub": SUB, "iat": T0 - 3900, "exp": T0 - 300})
NEW_TOKEN = jwt({"iss": "https://auth.shop.example/", "aud": "shop-web", "sub": SUB, "iat": T0 + 1, "exp": T0 + 3601})
FRESH_TOKEN = jwt({"iss": "https://auth.shop.example/", "aud": "shop-web", "sub": SUB, "iat": T0 - 600, "exp": T0 + 3000})
# A device token minted by a phone whose clock runs ten minutes fast.
DEVICE_TOKEN = jwt({"iss": "https://device.shop.example/", "aud": "shop-search", "sub": "device_41b7", "iat": T0 + 620, "exp": T0 + 4220})
REFRESH = "rt_demo_" + "7f3a9c1e" * 4  # made up
CONFIG_BLOB = base64.b64encode(json.dumps({"region": "nl", "theme": "dark", "key": "demo-key-0000"}, separators=(",", ":")).encode()).decode()
EXPORT_BLOB = base64.b64encode(json.dumps({"order": 9921, "format": "pdf", "nonce": "demo"}, separators=(",", ":")).encode()).decode()


for _blob in (CONFIG_BLOB, EXPORT_BLOB):
    assert "/" not in _blob and "+" not in _blob, "pick another payload: this base64 would break the URL"


def tag(key: str, n: int = 10) -> str:
    return hashlib.sha1(key.encode()).hexdigest()[:n]


def jitter(key: str, lo: int, hi: int) -> int:
    """A repeatable number in [lo, hi] that depends only on key."""
    return lo + int(tag(key, 8), 16) % (hi - lo + 1)


def pairs(*items: tuple[str, str]) -> list[dict]:
    return [{"name": n, "value": v} for n, v in items]


class Capture:
    def __init__(self, name: str, creator_note: str):
        self.name = name
        self.note = creator_note
        self.rows: list[tuple[int, int, dict]] = []

    def add(
        self,
        at: int,
        method: str,
        url: str,
        status: int,
        *,
        token: str | None = None,
        cookie: bool = False,
        version: bool = True,
        req_type: str | None = None,
        req_body: str | None = None,
        req_extra: list[tuple[str, str]] | None = None,
        resp_type: str | None = "application/json",
        resp_body: str | None = None,
        resp_extra: list[tuple[str, str]] | None = None,
        size: int | None = None,
        wait: int | None = None,
        recv: int | None = None,
        send: int = 1,
        blocked: int = -1,
        dns: int = -1,
        connect: int = -1,
        ssl: int = -1,
        redirect: str = "",
        error: str | None = None,
        reqid: bool = True,
        pseudo: bool = False,
        http: str = "h2",
    ) -> None:
        scheme, rest = url.split("://", 1)
        host, _, tail = rest.partition("/")
        path_q = "/" + tail
        path = path_q.split("?", 1)[0]
        query = []
        if "?" in path_q:
            for kv in path_q.split("?", 1)[1].split("&"):
                k, _, v = kv.partition("=")
                query.append({"name": k, "value": v})
        key = f"{self.name}:{at}:{method}:{url}"
        wait = jitter(key + "w", 30, 90) if wait is None else wait
        recv = jitter(key + "r", 2, 14) if recv is None else recv

        headers: list[tuple[str, str]] = []
        if pseudo:
            headers += [(":authority", host), (":method", method), (":path", path_q), (":scheme", scheme)]
        headers += [("accept", "application/json, text/plain, */*"), ("accept-encoding", "gzip, deflate, br"), ("user-agent", UA)]
        if host != "shop.example":
            headers.append(("origin", "https://shop.example"))
        headers.append(("referer", "https://shop.example/"))
        if token:
            headers.append(("authorization", f"Bearer {token}"))
        if cookie:
            headers.append(("cookie", "session=sess_demo_0001; theme=dark"))
        if version and host == "api.shop.example":
            headers.append(("x-app-version", APP))
        if req_type:
            headers.append(("content-type", req_type))
        if req_body is not None:
            headers.append(("content-length", str(len(req_body))))
        headers += req_extra or []

        resp_headers: list[tuple[str, str]] = []
        if resp_type:
            resp_headers.append(("content-type", resp_type))
        resp_headers.append(("date", (START + timedelta(milliseconds=at + 120)).strftime("%a, %d %b %Y %H:%M:%S GMT")))
        resp_headers.append(("server", "shopfront"))
        if host in ("api.shop.example", "auth.shop.example", "payments.shop.example") and reqid:
            resp_headers.append(("x-request-id", "req_" + tag(key)))
            resp_headers.append(("cf-ray", tag(key + "ray", 16) + "-AMS"))
        if redirect:
            resp_headers.append(("location", redirect))
        resp_headers += resp_extra or []

        parts = [wait, recv, max(send, 0)] + [v for v in (blocked, dns, connect) if v >= 0]
        total = sum(parts)
        content_size = size if size is not None else (len(resp_body) if resp_body else 0)
        entry = {
            "startedDateTime": (START + timedelta(milliseconds=at)).strftime("%Y-%m-%dT%H:%M:%S.") + f"{(at % 1000):03d}Z",
            "time": total,
            "request": {
                "method": method,
                "url": url,
                "httpVersion": "h2" if http == "h2" else "http/1.1",
                "cookies": [],
                "headers": pairs(*headers),
                "queryString": query,
                "headersSize": -1,
                "bodySize": len(req_body) if req_body else 0,
            },
            "response": {
                "status": status,
                "statusText": {200: "OK", 201: "Created", 204: "No Content", 301: "Moved Permanently", 302: "Found", 401: "Unauthorized", 402: "Payment Required", 429: "Too Many Requests", 500: "Internal Server Error", 502: "Bad Gateway", 503: "Service Unavailable", 0: ""}.get(status, ""),
                "httpVersion": "h2" if http == "h2" else "http/1.1",
                "cookies": [],
                "headers": pairs(*resp_headers) if status else [],
                "content": {"size": content_size, "mimeType": resp_type if status else "x-unknown"},
                "redirectURL": redirect,
                "headersSize": -1,
                "bodySize": content_size if status else -1,
            },
            "cache": {},
            "timings": {"blocked": blocked, "dns": dns, "connect": connect, "ssl": ssl, "send": send, "wait": wait, "receive": recv},
            "serverIPAddress": IP[host],
        }
        if req_body is not None:
            entry["request"]["postData"] = {"mimeType": req_type or "text/plain", "text": req_body}
        if resp_body is not None:
            entry["response"]["content"]["text"] = resp_body
        if error:
            entry["_error"] = error
        self.rows.append((at, len(self.rows), entry))

    def har(self) -> dict:
        entries = [e for _, _, e in sorted(self.rows, key=lambda r: (r[0], r[1]))]
        return {
            "log": {
                "version": "1.2",
                "creator": {"name": "WireTrail demo generator (synthetic)", "version": "1"},
                "comment": self.note,
                "entries": entries,
            }
        }


def j(**kw) -> str:
    return json.dumps(kw, separators=(",", ":"))


def err(code: str, message: str) -> str:
    return json.dumps({"error": {"code": code, "message": message}}, separators=(",", ":"))


NOTE = (
    "Synthetic capture generated by scripts/docs-make-demo-hars.py for the WireTrail user guide. "
    "It is not real traffic: the hosts use the reserved .example domain and every token and key is made up."
)


def build(variant: str) -> Capture:
    cur = variant == "current"
    c = Capture(variant, NOTE)
    recovered = NEW_TOKEN if cur else FRESH_TOKEN  # the token the app holds once the session is healthy
    api = "https://api.shop.example"
    cdn = "https://cdn.shop.example"

    # ---- page load -----------------------------------------------------------------------
    if cur:
        c.add(0, "GET", "http://www.shop.example/", 301, resp_type="text/html", redirect="https://shop.example/", wait=18, recv=2, cookie=True, version=False)
    c.add(70, "GET", "https://shop.example/", 200, resp_type="text/html", resp_body="<!doctype html><title>Shop</title>", size=19840, wait=140, recv=40, cookie=True, version=False, dns=20, connect=60, ssl=40)
    c.add(330, "GET", f"{cdn}/assets/app.5b1e0c7d.js", 200, resp_type="application/javascript", size=188416, wait=35, recv=90, version=False, cookie=False)
    c.add(335, "GET", f"{cdn}/assets/vendor.9c2d41ab.js", 200, resp_type="application/javascript", size=421888, wait=38, recv=160, version=False)
    c.add(340, "GET", f"{cdn}/assets/app.2e7f1a40.css", 200, resp_type="text/css", size=38912, wait=30, recv=22, version=False)
    c.add(345, "GET", f"{cdn}/fonts/inter-var.woff2", 200, resp_type="font/woff2", size=100352, wait=31, recv=45, version=False)
    c.add(350, "GET", f"{cdn}/img/logo.svg", 200, resp_type="image/svg+xml", size=2048, wait=29, recv=4, version=False)
    c.add(430, "GET", f"{cdn}/cfg/{CONFIG_BLOB}/manifest.json", 200, resp_body=j(theme="dark", region="nl"), wait=34, recv=3, version=False, reqid=False)
    if cur:
        hero_at = [420, 2400, 6400, 12400, 22400, 36400]
        for i, at in enumerate(hero_at):
            c.add(at, "GET", f"{cdn}/img/hero.png", 302, resp_type="image/png", redirect=f"{cdn}/img/hero.webp", wait=jitter(f"hero{i}", 22, 30), recv=1, version=False)
        c.add(450, "GET", f"{cdn}/img/hero.webp", 200, resp_type="image/webp", size=2202009, wait=120, recv=1800, version=False)
    else:
        c.add(450, "GET", f"{cdn}/img/hero.webp", 200, resp_type="image/webp", size=2202009, wait=120, recv=420, version=False)

    # ---- boot: config, then the session with a token that may have expired -----------------
    cfg_body = j(currency="EUR", locale="nl-NL", features=["wishlist", "express-checkout"], padding="x" * (11200 if cur else 3300))
    c.add(900, "GET", f"{api}/v1/config", 200, resp_body=cfg_body, wait=60, recv=18, pseudo=True, dns=45, connect=90, ssl=60)
    for n, at in enumerate([2000, 7000, 12000, 17000, 22000, 27000]):
        c.add(at, "GET", f"{api}/v1/flags?cb={T0 + n * 7}", 200, resp_body=j(flags=["wishlist", "express-checkout"]), token=recovered)
    if cur:
        c.add(1000, "GET", f"{api}/v1/auth/session", 401, token=OLD_TOKEN, resp_body=err("token_expired", "JWT expired"), pseudo=True)
        c.add(1050, "GET", f"{api}/v1/cart", 401, token=OLD_TOKEN, resp_body=err("token_expired", "JWT expired"))
        c.add(1190, "GET", f"{api}/v1/cart", 401, token=OLD_TOKEN, resp_body=err("token_expired", "JWT expired"))
        c.add(
            1300, "POST", "https://auth.shop.example/oauth/token", 200,
            req_type="application/json", req_body=j(grant_type="refresh_token", refresh_token=REFRESH),
            resp_body=json.dumps({"access_token": NEW_TOKEN, "refresh_token": REFRESH, "token_type": "Bearer", "expires_in": 3600}, separators=(",", ":")),
            resp_extra=[("set-cookie", "refresh=" + REFRESH + "; HttpOnly; Secure; Path=/oauth")],
            wait=280, recv=6, dns=35, connect=85, ssl=55, version=False,
        )
        c.add(1340, "GET", f"{api}/v1/notifications", 401, token=OLD_TOKEN, resp_body=err("token_expired", "JWT expired"))
        c.add(1700, "GET", f"{api}/v1/auth/session", 200, token=recovered, resp_body=j(user=SUB, plan="plus"), pseudo=True)
        c.add(1710, "GET", f"{api}/v1/cart", 200, token=recovered, resp_body=j(items=[{"sku": 1042, "qty": 1}], total=89.0))
        c.add(1720, "GET", f"{api}/v1/notifications", 200, token=recovered, resp_body=j(unread=2))
    else:
        c.add(1000, "GET", f"{api}/v1/auth/session", 200, token=recovered, resp_body=j(user=SUB, plan="plus"), pseudo=True)
        c.add(1050, "GET", f"{api}/v1/cart", 200, token=recovered, resp_body=j(items=[{"sku": 1042, "qty": 1}], total=89.0))
        c.add(1340, "GET", f"{api}/v1/notifications", 200, token=recovered, resp_body=j(unread=2))

    # ---- catalog: pages, then one request per product ---------------------------------------
    pw = 520 if cur else 90
    cat = "category=shoes&limit=24"
    for n, at in enumerate([3000, 3400, 3800]):
        c.add(at, "GET", f"{api}/v1/products?{cat}&page={n + 1}", 200, token=recovered, resp_body=j(page=n + 1, items=[1042 + n * 24, 1043 + n * 24]), wait=pw + jitter(f"p{n}", 0, 12), recv=30)
    if cur:
        c.add(4100, "GET", f"{api}/v1/products?{cat}&page=3", 200, token=recovered, resp_body=j(page=3, items=[1090, 1091]), wait=pw + 7, recv=30)
        for n in range(12):
            pid = 1042 + n
            c.add(
                4200 + n * 60, "GET", f"{api}/v1/products/{pid}/price", 200, token=recovered, resp_body=j(id=pid, eur=79 + n),
                wait=jitter(f"price{n}", 80, 140), blocked=880 if n == 6 else -1,
            )
    else:
        c.add(4100, "POST", f"{api}/v1/prices", 200, token=recovered, req_type="application/json", req_body=j(ids=list(range(1042, 1054))), resp_body=j(prices={str(1042 + n): 79 + n for n in range(12)}), wait=140)

    # ---- heartbeat: the same call over and over ----------------------------------------------
    for n in range(12 if cur else 3):
        at = 6000 + n * 1500
        c.add(at, "POST", f"{api}/v1/session/ping", 200, token=recovered, req_type="application/json", req_body=j(sid="s_7c2e", ts=T0 * 1000 + at), resp_body=j(ok=True), wait=jitter(f"ping{n}", 25, 45))

    # ---- search as you type, then a rate limit that is not respected --------------------------
    sg = "https://search.shop.example"
    for n, q in enumerate(["r", "ru", "run", "runn", "runni", "runnin", "running"]):
        c.add(
            9000 + n * 200, "GET", f"{sg}/v1/suggest?q={q}", 200, resp_body=j(q=q, hits=[q + "ning shoes"]), reqid=False,
            req_extra=[("x-device-token", DEVICE_TOKEN)], resp_extra=[("x-ratelimit-limit", "30"), ("x-ratelimit-remaining", str(29 - n))], wait=jitter(f"sg{n}", 20, 40),
        )
    if cur:
        rl = [("retry-after", "5"), ("x-ratelimit-limit", "30"), ("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", str(T0 + 51))]
        for at, q in [(46000, "shoes"), (46800, "shoe"), (48100, "sho"), (52200, "sh")]:
            c.add(at, "GET", f"{sg}/v1/suggest?q={q}", 429, resp_body=err("rate_limited", "too many requests"), resp_extra=rl, reqid=False, wait=22)
        c.add(58000, "GET", f"{sg}/v1/suggest?q=shoes", 200, resp_body=j(q="shoes", hits=["shoes sale"]), resp_extra=[("x-ratelimit-limit", "30"), ("x-ratelimit-remaining", "29")], reqid=False, wait=30)
    else:
        c.add(46000, "GET", f"{sg}/v1/suggest?q=shoes", 200, resp_body=j(q="shoes", hits=["shoes sale"]), resp_extra=[("x-ratelimit-limit", "30"), ("x-ratelimit-remaining", "24")], reqid=False, wait=30)

    # ---- a flaky upstream: one retry that recovers, one that does not -------------------------
    c.add(13000, "GET", f"{api}/v1/inventory/1042?store=ams", 502, token=recovered, resp_body=err("bad_gateway", "bad gateway"), wait=210)
    c.add(13800, "GET", f"{api}/v1/inventory/1042?store=ams", 200, token=recovered, resp_body=j(sku=1042, stock=14), wait=95)
    c.add(15000, "GET", f"{cdn}/img/banner.webp", 0, resp_type=None, wait=0, recv=0, send=0, dns=30, connect=1870, error="net::ERR_CONNECTION_RESET", version=False)
    c.add(19500, "GET", f"{cdn}/img/banner.webp", 200, resp_type="image/webp", size=61440, wait=40, recv=30, version=False)
    sync_body = j(items=[{"sku": 1042, "qty": 1}], cart="c_55d1")
    if cur:
        gaps = [(25000, 503), (26200, 503), (28700, 503), (33900, 503), (42100, 500)]
        for n, (at, st) in enumerate(gaps):
            body = err("upstream_unavailable", "inventory service unavailable") if st == 503 else err("internal_error", "internal server error")
            c.add(at, "POST", f"{api}/v1/cart/sync", st, token=recovered, req_type="application/json", req_body=sync_body, resp_body=body, wait=jitter(f"sync{n}", 180, 260))
    else:
        c.add(25000, "POST", f"{api}/v1/cart/sync", 200, token=recovered, req_type="application/json", req_body=sync_body, resp_body=j(ok=True), wait=120)

    # ---- checkout ---------------------------------------------------------------------------
    pay = "https://payments.shop.example"
    gql = f"{api}/graphql"
    c.add(62000, "POST", gql, 200, token=recovered, req_type="application/json", req_body=j(operationName="Cart", query="query Cart { cart { total } }"), resp_body=j(data={"cart": {"total": 89.0}}), wait=70)
    if cur:
        c.add(62400, "POST", gql, 200, token=recovered, req_type="application/json", req_body=j(query="{ me { plan } }"), resp_body=j(data={"me": {"plan": "plus"}}), wait=64)
        c.add(62600, "POST", gql, 200, token=recovered, req_type="application/json", req_body=j(query="{ cart { items { sku } } }"), resp_body=j(data={"cart": {"items": [{"sku": 1042}]}}), wait=61)
    c.add(63000, "GET", f"{api}/v1/cart", 200, token=recovered, resp_body=j(items=[{"sku": 1042, "qty": 1}], total=89.0))
    c.add(63800, "OPTIONS", f"{pay}/v1/charges", 204, resp_type=None, wait=25, recv=1, reqid=False, version=False, dns=640 if cur else 30, connect=220, ssl=140)
    if cur:
        c.add(64000, "POST", f"{pay}/v1/charges", 402, token=recovered, req_type="application/json", req_body=j(order="o_4410", amount=8900, currency="EUR"), resp_body=err("card_declined", "Your card was declined."), wait=180, recv=20, version=False)
        c.add(66500, "POST", f"{pay}/v1/charges", 200, token=recovered, req_type="application/json", req_body=j(order="o_4410", amount=8900, currency="EUR"), req_extra=[("idempotency-key", "idem_demo_4410_2")], resp_body=j(id="ch_demo_1", status="paid"), wait=240, version=False)
    else:
        c.add(64000, "POST", f"{pay}/v1/charges", 200, token=recovered, req_type="application/json", req_body=j(order="o_4410", amount=8900, currency="EUR"), req_extra=[("idempotency-key", "idem_demo_4410_1")], resp_body=j(id="ch_demo_1", status="paid"), wait=240, dns=30, connect=140, ssl=90, version=False)
    c.add(67000, "OPTIONS", f"{api}/v1/orders", 204, resp_type=None, wait=20, recv=1, token=None)
    c.add(67200, "POST", f"{api}/v1/orders", 201, token=recovered, req_type="application/json", req_body=j(cart="c_55d1", payment="ch_demo_1"), resp_body=j(id="ord_9921", status="created"), wait=2400 if cur else 310, recv=8)
    c.add(67300, "PUT", f"{api}/v1/cart/items/1042", 200, token=recovered, req_type="application/json", req_body=j(qty=0), resp_body="", wait=55)
    c.add(
        67800, "POST", f"{api}/v1/uploads/avatar", 201, token=recovered, req_type="application/octet-stream", req_body="x" * 64,
        resp_body=j(id="up_1"), send=950 if cur else 140, wait=150, recv=4, size=60,
    )
    for n in range(3):
        c.add(68000 + n * 400, "POST", f"{api}/v1/track", 200, req_type="text/plain", req_body=j(event="view", n=n), resp_body=j(ok=True), version=False, wait=28)
    c.add(69000, "GET", f"{api}/v1/legacy/status", 200, resp_type="text/html", resp_body=j(ok=True), version=False, wait=35)
    c.add(69500, "GET", f"{api}/v1/search", 200, token=recovered, req_type="application/json", req_body=j(q="shoes"), resp_body=j(hits=3), wait=44)
    c.add(69800, "GET", f"{api}/v1/receipts/9921/download?d={EXPORT_BLOB}&token=rcpt_demo_8f21&sig=0f3a9c&page=1", 200, token=recovered, resp_type="application/pdf", size=48201, wait=88, recv=64)

    # ---- error reports ------------------------------------------------------------------------
    sentry = "https://o0.ingest.sentry.io/api/1/envelope/"
    c.add(2500, "POST", sentry, 200, resp_type="application/json", req_type="application/json", req_body='{"event_id":"demo"}', resp_body=j(id="demo"), wait=60, dns=40, connect=700, ssl=520, reqid=False, version=False)
    if cur:
        for at in (14000, 40000):
            c.add(at, "POST", sentry, 200, resp_type="application/json", req_type="application/json", req_body='{"event_id":"demo"}', resp_body=j(id="demo"), wait=48, reqid=False, version=False)
    return c


def sanitize(har: dict) -> dict:
    """What an exporter that strips secrets and bodies writes: no auth, no cookies, no bodies."""
    out = json.loads(json.dumps(har))
    out["log"]["creator"] = {"name": "WireTrail demo generator (synthetic, sanitized)", "version": "1"}
    out["log"]["comment"] = NOTE + " This copy has headers that carry credentials, and all bodies, removed."
    drop = {"authorization", "cookie", "set-cookie", "x-device-token"}
    secret_query = re.compile(r"([?&](?:token|sig|d)=)[^&]*")
    for e in out["log"]["entries"]:
        e["request"]["url"] = secret_query.sub(r"\1REDACTED", e["request"]["url"])
        for q in e["request"]["queryString"]:
            if q["name"] in ("token", "sig", "d"):
                q["value"] = "REDACTED"
        for side in ("request", "response"):
            e[side]["headers"] = [h for h in e[side]["headers"] if h["name"].lower() not in drop]
        e["request"].pop("postData", None)
        e["response"]["content"].pop("text", None)
    return out


def write(path: Path, har: dict) -> None:
    entries = har["log"]["entries"]
    head = {k: v for k, v in har["log"].items() if k != "entries"}
    lines = ",\n".join("  " + json.dumps(e, separators=(",", ":")) for e in entries)
    text = '{"log":' + json.dumps({**head, "entries": ["__ENTRIES__"]}, separators=(",", ":")).replace('"__ENTRIES__"', "\n" + lines + "\n") + "}\n"
    json.loads(text)  # the file must be valid JSON
    path.write_text(text)
    print(f"wrote {path.relative_to(ROOT)}  {len(entries)} entries  {len(text):,} bytes")


def mini(at: int, method: str, url: str, status: int, *, req_h=(), resp_h=(), req_body=None, req_mime="application/json", resp_body=None, wait=40, redirect="", timings=None, resp_type="application/json") -> dict:
    """One bare entry, for the small files that show a single behaviour."""
    started = (START + timedelta(milliseconds=at)).strftime("%Y-%m-%dT%H:%M:%S.") + f"{at % 1000:03d}Z"
    phases = timings or {"send": 1, "wait": wait, "receive": 5}
    # HAR time is the sum of the phases; ssl is already inside connect, so it is not added twice.
    total = sum(v for k, v in phases.items() if k != "ssl" and v >= 0)
    e = {
        "startedDateTime": started,
        "time": total,
        "request": {"method": method, "url": url, "httpVersion": "h2", "headers": pairs(*req_h), "queryString": [], "headersSize": -1, "bodySize": len(req_body or "")},
        "response": {"status": status, "statusText": "", "httpVersion": "h2", "headers": pairs(*resp_h), "content": {"size": len(resp_body or ""), "mimeType": resp_type},
                     "redirectURL": redirect, "headersSize": -1, "bodySize": len(resp_body or "")},
        "cache": {},
        "timings": phases,
        "serverIPAddress": "203.0.113.9",
    }
    if req_body is not None:
        e["request"]["postData"] = {"mimeType": req_mime, "text": req_body}
    if resp_body is not None:
        e["response"]["content"]["text"] = resp_body
    return e


def small(name: str, note: str, entries: list[dict]) -> None:
    har = {"log": {"version": "1.2", "creator": {"name": "WireTrail demo generator (synthetic)", "version": "1"}, "comment": note, "entries": entries}}
    path = OUT / "quirks" / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(har, separators=(",", ":")) + "\n")
    print(f"wrote {path.relative_to(ROOT)}  {len(entries)} entries")


def quirks() -> None:
    """Tiny captures that each show one thing wiretrail does that a reader might not expect.

    reference/limitations.md runs wiretrail on them, so every claim there can be reproduced."""
    api = "https://api.test.example"
    small("constant-limit.har", "Three different pages with the same page size.",
          [mini(i * 300, "GET", f"{api}/v1/items?page={i + 1}&limit=24", 200) for i in range(3)])
    small("short-secrets.har", "A login whose password and token are short, readable strings.",
          [mini(0, "POST", f"{api}/login", 200, req_body='{"user":"ann","password":"hunter2"}',
                resp_body='{"ok":true,"password":"hunter2","token":"abc123","note":"password reset sent"}')])
    small("two-hosts-refresh.har", "Host A refreshes its token. Host B keeps its own, unrelated token.", [
        mini(0, "GET", "https://api-a.test.example/v1/me", 200, req_h=[("authorization", "Bearer AAA-token-for-a")]),
        mini(100, "GET", "https://api-b.test.example/v1/me", 200, req_h=[("authorization", "Bearer BBB-token-for-b")]),
        mini(1000, "POST", "https://api-a.test.example/oauth/token", 200, resp_body='{"ok":true}'),
        mini(1200, "GET", "https://api-a.test.example/v1/me", 200, req_h=[("authorization", "Bearer AAA2-new-token-for-a")]),
        mini(1300, "GET", "https://api-b.test.example/v1/me", 200, req_h=[("authorization", "Bearer BBB-token-for-b")]),
    ])
    small("retry-after-date.har", "A 429 whose Retry-After is an HTTP date, not a number of seconds.", [
        mini(0, "GET", f"{api}/v1/x", 429, resp_h=[("retry-after", "Tue, 20 Oct 2026 09:14:32 GMT")]),
        mini(500, "GET", f"{api}/v1/x", 200),
    ])
    small("refresh-in-body.har", "A token refresh sent as a form body to a path that is not /token or /oauth.", [
        mini(0, "GET", f"{api}/v1/me", 200, req_h=[("authorization", "Bearer OLD-token-value-1")]),
        mini(500, "POST", f"{api}/v1/session/renew", 200, req_body="grant_type=refresh_token&refresh_token=x"),
        mini(800, "GET", f"{api}/v1/me", 200, req_h=[("authorization", "Bearer NEW-token-value-2")]),
    ])
    small("unauthorized-twice.har", "The same call answered 401, 401, then 200.",
          [mini(0, "GET", f"{api}/v1/y", 401), mini(300, "GET", f"{api}/v1/y", 401), mini(600, "GET", f"{api}/v1/y", 200)])
    small("tls-in-connect.har", "One call whose time is mostly the TLS handshake, written the way the HAR spec asks (ssl is part of connect).",
          [mini(0, "GET", "https://tls.test.example/", 200, timings={"dns": 5, "connect": 700, "ssl": 600, "send": 1, "wait": 20, "receive": 5}, wait=20)])
    small("unmasked-places.har", "Places where a secret-looking value is printed: a repeated call's query, a form body and a server message.", [
        mini(0, "GET", f"{api}/v1/ping?session=s3cr3t-short-value", 200),
        mini(100, "GET", f"{api}/v1/ping?session=s3cr3t-short-value", 200),
        mini(200, "POST", f"{api}/v1/login", 401, req_body="user=ann&password=hunter2&grant_type=password",
             req_mime="application/x-www-form-urlencoded", resp_body='{"message":"invalid token abc123 for ann"}'),
    ])
    small("many-error-groups.har", "Eleven routes that each return four 404s, and one route that returns three 503s.",
          [mini(i * 7000, "GET", f"{api}/missing{i // 4}", 404, resp_body='{"error":"not found"}') for i in range(44)]
          + [mini((44 + i) * 7000, "GET", f"{api}/broken", 503, resp_body='{"error":"unavailable"}') for i in range(3)])
    small("volatile-substring.har", "Two orders whose bodies differ only in order_id and events, which are not timestamps.", [
        mini(0, "POST", f"{api}/v1/orders", 201, req_body='{"order_id":1001,"events":["a"],"total":10}'),
        mini(500, "POST", f"{api}/v1/orders", 201, req_body='{"order_id":1002,"events":["b"],"total":10}'),
    ])
    small("relative-redirect.har", "Six redirects whose Location is a relative path.",
          [mini(i * 100, "GET", "https://cdn.test.example/a.png", 302, redirect="/b.png") for i in range(6)])


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    cur = build("current").har()
    write(OUT / "checkout.har", cur)
    write(OUT / "baseline.har", build("baseline").har())
    write(OUT / "sanitized.har", sanitize(cur))
    quirks()


if __name__ == "__main__":
    main()

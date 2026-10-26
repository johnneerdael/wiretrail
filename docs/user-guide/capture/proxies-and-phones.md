# From a proxy or a phone

A native app or a phone has no Network panel. A **proxy** sits between the app and the internet, records what passes through it and exports
the session as a HAR. Charles, Proxyman, HTTP Toolkit, Fiddler and mitmproxy all do.

## Export from a proxy

| Tool | How to get a HAR |
|---|---|
| Charles | Choose **File, Export Session**, and pick the HTTP Archive format (`.har`). |
| Proxyman | Choose **File, Export**, and pick HAR. |
| HTTP Toolkit | Use the export option on the session and choose HAR. |
| Fiddler | Choose **File, Export Sessions**, and pick HTTPArchive. |
| mitmproxy | It stores flows in its own format. Export to HAR with the HAR export in `mitmweb` or in a recent version, depending on yours. |

The menus differ between versions. Look for the words "HAR" or "HTTP Archive". Then check that the file loads: `wiretrail capture.har validate`.

## Read an app's HTTPS traffic

A proxy can only show you the content of HTTPS requests if the device **trusts the proxy's certificate**. The usual route:

1. Point the device or emulator at the proxy's address and port, in its network settings.
2. Install the proxy's certificate authority on the device. The proxy's documentation says how, and most have a page for the phone to open.
3. On iOS, also switch on full trust for that certificate in the certificate trust settings.

Two things stop this working, and neither is a WireTrail problem:

- **Android apps do not trust a user-installed certificate by default** in recent versions of the system. An app has to opt in, or be a debug build.
- **An app that pins its certificate** refuses the proxy's. You see failed connections and no content.

Without a trusted certificate the proxy records only the host names and the sizes of encrypted traffic. A HAR from that has no paths and no bodies, and
`validate` says so.

## Capture a web page on a phone

For a page in the phone's browser, remote debugging gives the same Network panel as on the desktop. Chrome on Android shows up in desktop Chrome's
`chrome://inspect`, and Safari on iOS shows up in the desktop Safari's Develop menu. Both save HAR files the way [a desktop browser](browsers.md) does.

## What a proxy adds

A proxy sees the whole device, not one page, so it records every app's traffic, including analytics and background calls. That is useful
for a start-up capture and noisy for anything else. Filter by host in the proxy before you export, or by host in WireTrail, with
`--filter "host:api.example.com"`.

It also records the proxy's own timings. The `wait` and `receive` figures are measured at the proxy, which is a little different from what the app felt.

## Related

- [From a browser](browsers.md) for a page in a desktop browser.
- [What a useful capture holds](what-a-capture-needs.md) lists what to switch on.
- [Handle captures safely](handle-captures-safely.md) before you share the file.

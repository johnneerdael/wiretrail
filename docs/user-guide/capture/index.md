# Get a HAR

A **HAR** (HTTP Archive) is a JSON file that records every request a browser or an app made and every response it got:
the URL, the headers, the timing of each step and, if you asked for them, the bodies. Nearly every browser and proxy
can export one.

WireTrail reads HAR versions 1.2 and 1.3, which is what the tools below write. It reads the file as it is, and it
does not need the capture to come from a particular tool.

## Where a HAR comes from

| You are looking at | Use | Page |
|---|---|---|
| A web page in a desktop browser | The browser's developer tools | [From a browser](browsers.md) |
| A native app, a phone, or a program with no developer tools | A proxy that sits between the app and the network | [From a proxy or a phone](proxies-and-phones.md) |

## Before you capture

Decide what the capture is for, and keep it to that. A capture of one flow, from a clean start to the moment the problem shows, is
easier to read and safer to share than an hour of browsing. [What a useful capture holds](what-a-capture-needs.md) lists what the analysis
needs, and `validate` tells you whether yours has it.

A HAR holds credentials. The cookies, the `Authorization` headers and the bodies in it can sign someone in. Treat the file like a password
vault, and read [Handle captures safely](handle-captures-safely.md) before you send it to anyone.

## Related

- [Validate a capture](../analyze/validate.md) checks a file against what the commands need.
- [Follow along with the demo captures](../getting-started/follow-along.md) if you want to try WireTrail first.

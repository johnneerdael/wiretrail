# Timing phases

A HAR records, for each request, how long it spent in each of several phases. `slowest`, `show-entry` and `report` read them. `startup` and `compare` use only the total. This
page defines the phases and explains the label `slowest` puts on a request.

--8<-- "diagram-phases.html"

## The phases

| Phase | What it measures |
|---|---|
| `blocked` | Time the request spent waiting before it could be sent. Often it is queued behind other requests on the same connection, or waiting for a proxy. |
| `dns` | Looking up the host name. |
| `connect` | Opening the connection to the server. When the connection is encrypted this **includes** the TLS handshake. |
| `ssl` | The TLS handshake. In a spec-compliant HAR it is already counted inside `connect`. |
| `send` | Sending the request, headers and body. |
| `wait` | Waiting for the server to start answering. It is also called time to first byte, and it is the server's thinking time plus the trip there and back. |
| `receive` | Reading the response. |

A request on a connection that is already open has no `dns`, `connect` or `ssl`. A HAR writes `-1` for a phase that does not apply, and WireTrail reads it as "not
applicable". `slowest` prints those phases as `0ms`, and the JSON shows `null`.

## The total

The duration WireTrail shows for a request is the **`time` field of the entry**, which the capturing tool measured. It is not recomputed from the phases. In a well-made HAR the two agree, and
`time` is the sum of `blocked`, `dns`, `connect`, `send`, `wait` and `receive`, with `ssl` not added a second time.

## The bottleneck label

`slowest` names the **largest single phase** of each request:

| Label | The phase |
|---|---|
| `queueing/blocked` | `blocked` |
| `DNS` | `dns` |
| `TCP connect` | `connect` |
| `TLS handshake` | `ssl` |
| `request upload` | `send` |
| `server wait/TTFB` | `wait` |
| `download/receive` | `receive` |
| `unknown` | None is above zero. |

The comparison goes in the order above and the first phase to reach the largest value wins, so a tie goes to the earlier one.

!!! warning "TLS time is inside connect"
    Because `connect` includes `ssl`, `connect` is never smaller than `ssl` in a HAR that follows the specification. So `TLS handshake` is not chosen unless a tool writes `ssl` separately and
    larger, and a slow handshake is labelled `TCP connect`. `slowest` prints `ssl` in `phases`, so read that figure. See [Slowest calls](../failures/slowest.md#read-a-row).

## A label is a place to look

The label says where the time went, and not why. `server wait/TTFB` points at the server, but a slow network between the client and the server shows up there too, and
a capture taken through a proxy measures the proxy's view. Use the label to decide which team to ask, then check with a second measurement.

## Related

- [Slowest calls](../failures/slowest.md) prints the phases for each request.
- [Why a call is slow](../failures/recipes/why-a-call-is-slow.md) follows one call through them.
- [HAR support](../reference/har-support.md) lists the timing fields WireTrail reads.

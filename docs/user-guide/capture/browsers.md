# From a browser

Chrome, Edge, Firefox and Safari can all save what the Network panel recorded as a HAR. The steps below are the same idea in each, and the
menu names change between versions, so look for the word "HAR".

## Chrome and Edge

1. Open the developer tools (F12, or Cmd+Option+I on a Mac) and choose the **Network** tab.
2. Turn on **Preserve log**, so that requests are kept when the page navigates.
3. Consider turning on **Disable cache**, so that every request is sent and recorded. Leave it off if you are studying what the cache does.
4. Reload the page, or do the thing that goes wrong. The panel records as you go.
5. Right-click the list of requests and choose **Save all as HAR with content**, or use the download icon in the Network toolbar.

Recent versions can offer a **sanitized** export that removes cookies and credentials, as well as one that keeps them. The sanitized file
is safer to share, and it limits what WireTrail can tell you: there are no `Authorization` headers for `auth` and `jwt` to read. Run `validate`
on the result to see what you lost. [What a useful capture holds](what-a-capture-needs.md) has an example.

## Firefox

1. Open the developer tools and choose the **Network** tab.
2. Reload the page or reproduce the problem.
3. Right-click in the request list and choose **Save All As HAR**.

## Safari

1. Enable the Develop menu in Safari's settings, then open **Develop, Show Web Inspector** and choose the **Network** tab.
2. Reproduce the problem.
3. Use the **Export** button in the Network tab to save a HAR.

## Tips for a capture that analyzes well

- **Start recording before the first request.** Open the tools, then load the page. A capture that begins halfway has no start-up.
- **Keep it to one flow.** Sign in, add to cart, check out: not an afternoon.
- **Capture the good case too.** A second capture of the same flow, from before the problem, is what [`compare`](../compare/compare.md) needs.
- **Write down what you did.** The build, the time, the account, the network. A capture does not record why it was made.
- **Check it loads.** `wiretrail capture.har validate` takes a second.

## Related

- [From a proxy or a phone](proxies-and-phones.md) for apps without developer tools.
- [What a useful capture holds](what-a-capture-needs.md) says what each command needs.
- [Handle captures safely](handle-captures-safely.md) before you share the file.

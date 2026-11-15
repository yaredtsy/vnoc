---
node: 3W0W7885978656QSHZCZF6X40B
hint: httpx/_transports/default.py::HTTPTransport.handle_request
---
# Where httpx meets httpcore

This is the one place a request crosses from httpx into httpcore. Everything before it is httpx's own model (`Request`, `URL`, `Headers`); everything after is httpcore's connection pool.

## What it does

1. Takes the `httpx.Request` that `_send_single_request` built.
2. Builds the matching `httpcore.Request`: the method, the URL in parts, the raw headers, the same body stream and the extensions.
3. Sends it through the connection pool.
4. Wraps the answer in an `httpx.Response`.

```python
with map_httpcore_exceptions():
    resp = self._pool.handle_request(req)
```

## Errors

httpcore's exceptions come back as httpx's, always the most specific one that fits:

| httpcore | httpx |
|---|---|
| `ConnectTimeout` | `ConnectTimeout` |
| `ReadTimeout` | `ReadTimeout` |
| `ConnectError` | `ConnectError` |
| `RemoteProtocolError` | `RemoteProtocolError` |

> A `ReadTimeout` from the pool reaches you as `httpx.ReadTimeout`, not as a bare `TimeoutException`.

## Good to know

- `httpcore` is imported inside the method, not at the top of the file.
- The body is handed over as the same stream: it is not copied here.
- Only a sync request is accepted; the async twin is `AsyncHTTPTransport`.

---
node: 17BGP5PS3SCJ6BFMG5T6MDG10Q
hint: httpx/_client.py::Client.send
---
# How a request is sent

`send` takes a `Request` that is already built and returns a `Response`. It is a short loop around three private methods.

## The path

`send` → `_send_handling_auth` → `_send_handling_redirects` → `_send_single_request`

1. **`_send_handling_auth`** runs the auth flow, a generator: each `yield` is one more request to send.
2. **`_send_handling_redirects`** follows `3xx` answers, up to `max_redirects`.
3. **`_send_single_request`** puts exactly one request on the wire.

## What to remember

- An auth flow can send more than one request for one call (digest auth sends two).
- Redirects are followed here, **not** in the transport.
- `response.history` collects every response on the way.

> If a request hangs, check the pool limits first.

```python
client = httpx.Client()
response = client.send(client.build_request("GET", "https://example.org"))
```

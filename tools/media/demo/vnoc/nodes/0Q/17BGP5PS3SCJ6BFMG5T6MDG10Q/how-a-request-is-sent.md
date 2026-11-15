---
node: 17BGP5PS3SCJ6BFMG5T6MDG10Q
hint: httpx/_client.py::Client.send
---
# How a request is sent

`send` → `_send_handling_auth` → `_send_handling_redirects` → `_send_single_request`

- auth flows are generators: each `yield` is one more request
- redirects are followed here, not in the transport
- `history` collects every response

> If a request hangs, check the pool limits first.

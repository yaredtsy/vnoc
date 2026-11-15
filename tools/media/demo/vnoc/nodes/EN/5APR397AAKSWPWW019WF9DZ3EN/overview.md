---
node: 5APR397AAKSWPWW019WF9DZ3EN
hint: httpx
---
# Overview

A tour of the package, for someone reading it for the first time.

## Layout

| Where | What lives there |
|---|---|
| `_api.py` | the one-line helpers: `get`, `post`, … |
| `_client.py` | `Client` and `AsyncClient`: the real work |
| `_transports/` | how bytes leave the process |
| `_models.py` | `Request`, `Response`, `Headers`, `Cookies` |
| `_auth.py`, `_config.py` | auth flows, `Timeout`, `Limits`, `Proxy` |

## Where to start

**Start reading at `Client.send`.** It is the loop every request goes through: auth, then redirects, then one request on the wire.

## Conventions

- Names that start with `_` are private; the public API is re-exported by `__init__.py`.
- Sync and async code are written twice, side by side (`Client` and `AsyncClient`).

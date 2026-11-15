---
node: 3A8XGXJVW6FVABEE44TDTP3JCG
hint: httpx/_api.py
---
# Module helpers

Every helper opens a new `Client`, makes one request and closes the client again.

```python
import httpx

r = httpx.get("https://example.org")  # a Client lives for this one call
```

## Use them for

- scripts and one-off checks
- trying something in a shell

## Don't use them for

- anything repeated. Each call opens a fresh connection pool and gives up keep-alive: create one `httpx.Client()` and reuse it.

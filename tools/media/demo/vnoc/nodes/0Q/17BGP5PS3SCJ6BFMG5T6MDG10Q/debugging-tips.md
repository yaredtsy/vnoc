---
node: 17BGP5PS3SCJ6BFMG5T6MDG10Q
hint: httpx/_client.py::Client.send
---
# Debugging tips

## See every request

```python
client = httpx.Client(event_hooks={"request": [print]})
```

The hook runs once per hop, so a redirect shows up as two lines.

## See what the transport did

```python
import logging

logging.basicConfig(level=logging.INFO)
```

httpx then logs one line per request, like `HTTP Request: GET https://example.org "HTTP/1.1 200 OK"`.

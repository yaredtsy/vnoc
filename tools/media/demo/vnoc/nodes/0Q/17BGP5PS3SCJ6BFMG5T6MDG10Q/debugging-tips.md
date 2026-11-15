---
node: 17BGP5PS3SCJ6BFMG5T6MDG10Q
hint: httpx/_client.py::Client.send
---
# Debugging tips

See every request as it goes out:

```python
client = httpx.Client(event_hooks={"request": [print]})
```

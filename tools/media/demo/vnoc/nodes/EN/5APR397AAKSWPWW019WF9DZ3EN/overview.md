---
node: 5APR397AAKSWPWW019WF9DZ3EN
hint: httpx
---
# Overview

Public API in `_api.py`: each helper opens a one-off `Client`. The real work is in `_client.py`; transports live in `_transports/`.

**Start reading at `Client.send`.**

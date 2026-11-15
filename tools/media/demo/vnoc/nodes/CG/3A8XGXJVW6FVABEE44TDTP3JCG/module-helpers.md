---
node: 3A8XGXJVW6FVABEE44TDTP3JCG
hint: httpx/_api.py
---
# Module helpers

Every function opens a new `Client` and closes it. Fine for scripts; use one `Client` for anything repeated.

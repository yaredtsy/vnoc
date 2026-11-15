---
node: 03JMTWEPN2MJ6YHY9J5X6R00EK
hint: httpx/_client.py::Client._send_single_request
---
# One request

Sends exactly one request, without handling any redirection.

## Steps

1. Pick the transport for the URL (`_transport_for_url`).
2. Call `transport.handle_request(request)` inside `request_context`, which puts the request on any httpx error raised.
3. Wrap the response stream, so `response.elapsed` is set when the stream closes.
4. Store the cookies the response sets.

## Notes

- A sync client refuses an async request with a `RuntimeError`.
- It logs the request and the status line at `INFO`.

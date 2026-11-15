// The docs written for the demo (docs/18-release-0.2/01-media.md §2.2). They are what the docs viewer shows in the pictures, so they are written the way
// people write docs: a heading, a short opening, sections, a list, a code block, a table, a quote. Everything they say is true of httpx 0.28.1.
// `key` is the node's id from `vnoc/structure`. The title becomes the heading and the file name; `body` follows it.
export const docs = [
  {
    key: "httpx",
    title: "Overview",
    body: `A tour of the package, for someone reading it for the first time.

## Layout

| Where | What lives there |
|---|---|
| \`_api.py\` | the one-line helpers: \`get\`, \`post\`, … |
| \`_client.py\` | \`Client\` and \`AsyncClient\`: the real work |
| \`_transports/\` | how bytes leave the process |
| \`_models.py\` | \`Request\`, \`Response\`, \`Headers\`, \`Cookies\` |
| \`_auth.py\`, \`_config.py\` | auth flows, \`Timeout\`, \`Limits\`, \`Proxy\` |

## Where to start

**Start reading at \`Client.send\`.** It is the loop every request goes through: auth, then redirects, then one request on the wire.

## Conventions

- Names that start with \`_\` are private; the public API is re-exported by \`__init__.py\`.
- Sync and async code are written twice, side by side (\`Client\` and \`AsyncClient\`).`,
  },
  {
    key: "httpx/_api.py",
    title: "Module helpers",
    body: `Every helper opens a new \`Client\`, makes one request and closes the client again.

\`\`\`python
import httpx

r = httpx.get("https://example.org")  # a Client lives for this one call
\`\`\`

## Use them for

- scripts and one-off checks
- trying something in a shell

## Don't use them for

- anything repeated. Each call opens a fresh connection pool and gives up keep-alive: create one \`httpx.Client()\` and reuse it.`,
  },
  {
    key: "httpx/_client.py::Client.send",
    title: "How a request is sent",
    body: `\`send\` takes a \`Request\` that is already built and returns a \`Response\`. It is a short loop around three private methods.

## The path

\`send\` → \`_send_handling_auth\` → \`_send_handling_redirects\` → \`_send_single_request\`

1. **\`_send_handling_auth\`** runs the auth flow, a generator: each \`yield\` is one more request to send.
2. **\`_send_handling_redirects\`** follows \`3xx\` answers, up to \`max_redirects\`.
3. **\`_send_single_request\`** puts exactly one request on the wire.

## What to remember

- An auth flow can send more than one request for one call (digest auth sends two).
- Redirects are followed here, **not** in the transport.
- \`response.history\` collects every response on the way.

> If a request hangs, check the pool limits first.

\`\`\`python
client = httpx.Client()
response = client.send(client.build_request("GET", "https://example.org"))
\`\`\``,
  },
  {
    key: "httpx/_client.py::Client.send",
    title: "Debugging tips",
    body: `## See every request

\`\`\`python
client = httpx.Client(event_hooks={"request": [print]})
\`\`\`

The hook runs once per hop, so a redirect shows up as two lines.

## See what the transport did

\`\`\`python
import logging

logging.basicConfig(level=logging.INFO)
\`\`\`

httpx then logs one line per request, like \`HTTP Request: GET https://example.org "HTTP/1.1 200 OK"\`.`,
  },
  {
    key: "httpx/_client.py::Client._send_single_request",
    title: "One request",
    body: `Sends exactly one request, without handling any redirection.

## Steps

1. Pick the transport for the URL (\`_transport_for_url\`).
2. Call \`transport.handle_request(request)\` inside \`request_context\`, which puts the request on any httpx error raised.
3. Wrap the response stream, so \`response.elapsed\` is set when the stream closes.
4. Store the cookies the response sets.

## Notes

- A sync client refuses an async request with a \`RuntimeError\`.
- It logs the request and the status line at \`INFO\`.`,
  },
  {
    key: "httpx/_transports/default.py::HTTPTransport.handle_request",
    title: "Where httpx meets httpcore",
    body: `This is the one place a request crosses from httpx into httpcore. Everything before it is httpx's own model (\`Request\`, \`URL\`, \`Headers\`); everything after is httpcore's connection pool.

## What it does

1. Takes the \`httpx.Request\` that \`_send_single_request\` built.
2. Builds the matching \`httpcore.Request\`: the method, the URL in parts, the raw headers, the same body stream and the extensions.
3. Sends it through the connection pool.
4. Wraps the answer in an \`httpx.Response\`.

\`\`\`python
with map_httpcore_exceptions():
    resp = self._pool.handle_request(req)
\`\`\`

## Errors

httpcore's exceptions come back as httpx's, always the most specific one that fits:

| httpcore | httpx |
|---|---|
| \`ConnectTimeout\` | \`ConnectTimeout\` |
| \`ReadTimeout\` | \`ReadTimeout\` |
| \`ConnectError\` | \`ConnectError\` |
| \`RemoteProtocolError\` | \`RemoteProtocolError\` |

> A \`ReadTimeout\` from the pool reaches you as \`httpx.ReadTimeout\`, not as a bare \`TimeoutException\`.

## Good to know

- \`httpcore\` is imported inside the method, not at the top of the file.
- The body is handed over as the same stream: it is not copied here.
- Only a sync request is accepted; the async twin is \`AsyncHTTPTransport\`.`,
  },
  {
    // the Missing one: make-overlay.mjs removes this function's ID line and renames the function afterwards
    key: "httpx/_config.py::Timeout.as_dict",
    title: "Old timeout notes",
    body: `The four timeouts as a dict, for httpcore's \`timeout\` extension.

- keys: \`connect\`, \`read\`, \`write\`, \`pool\`
- keep them in sync with httpcore`,
    missing: true,
  },
];

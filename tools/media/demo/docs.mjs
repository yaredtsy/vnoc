// The docs written for the demo (docs/18-release-0.2/01-media.md §2.2): short on purpose, they must be readable in a GIF.
// `key` is the node's id from `vnoc/structure`. The title becomes the heading and the file name; `body` follows it.
export const docs = [
  {
    key: "httpx",
    title: "Overview",
    body: `Public API in \`_api.py\`: each helper opens a one-off \`Client\`. The real work is in \`_client.py\`; transports live in \`_transports/\`.

**Start reading at \`Client.send\`.**`,
  },
  {
    key: "httpx/_api.py",
    title: "Module helpers",
    body: `Every function opens a new \`Client\` and closes it. Fine for scripts; use one \`Client\` for anything repeated.`,
  },
  {
    key: "httpx/_client.py::Client.send",
    title: "How a request is sent",
    body: `\`send\` → \`_send_handling_auth\` → \`_send_handling_redirects\` → \`_send_single_request\`

- auth flows are generators: each \`yield\` is one more request
- redirects are followed here, not in the transport
- \`history\` collects every response

> If a request hangs, check the pool limits first.`,
  },
  {
    key: "httpx/_client.py::Client.send",
    title: "Debugging tips",
    body: `See every request as it goes out:

\`\`\`python
client = httpx.Client(event_hooks={"request": [print]})
\`\`\``,
  },
  {
    key: "httpx/_client.py::Client._send_single_request",
    title: "One request",
    body: `Sends exactly one request, no redirects. Sets \`response.elapsed\` when the stream closes. Cookies are stored here.`,
  },
  {
    key: "httpx/_transports/default.py::HTTPTransport.handle_request",
    title: "Where httpx meets httpcore",
    body: `Maps an \`httpx.Request\` to an \`httpcore.Request\` and back. httpcore's exceptions come back as httpx exceptions.`,
  },
  {
    // the Missing one: make-overlay.mjs removes this function's ID line and renames the function afterwards
    key: "httpx/_config.py::Timeout.as_dict",
    title: "Old timeout notes",
    body: `The four timeouts as a dict, for httpcore's \`timeout\` extension. Keep the keys in sync with httpcore.`,
    missing: true,
  },
];

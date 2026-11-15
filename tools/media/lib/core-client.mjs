// A stand-in editor for the V-NOC core, used to make the demo's docs without a window: it starts the core, speaks LSP framing to it, and applies the
// edits the core asks for (`workspace/applyEdit`) to the files on disk. The same thing crates/vnoc/tests/lifecycle.rs does in Rust.
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

/** Applies LSP text edits (line/character; the demo's code is ASCII, so a character is a code unit) to a text. */
export function applyEdits(text, edits) {
  const starts = [0];
  for (let i = text.indexOf("\n"); i >= 0; i = text.indexOf("\n", i + 1)) starts.push(i + 1);
  const offset = ({ line, character }) => starts[line] + character;
  for (const e of [...edits].sort((a, b) => offset(b.range.start) - offset(a.range.start))) {
    text = text.slice(0, offset(e.range.start)) + e.newText + text.slice(offset(e.range.end));
  }
  return text;
}

export class CoreClient {
  #child;
  #buffer = Buffer.alloc(0);
  #next = 0;
  #pending = new Map();
  #waiters = [];
  /** the notifications that came and nobody has asked for yet */
  #notifications = [];

  constructor(corePath) {
    this.#child = spawn(corePath, [], { stdio: ["pipe", "pipe", "inherit"], env: { ...process.env, VNOC_DRIVER: "" } });
    this.#child.stdout.on("data", (chunk) => this.#read(chunk));
  }

  async initialize({ driverPath, root }) {
    const init = await this.request("initialize", { processId: null, rootUri: null, capabilities: {}, initializationOptions: { driverPath, root } });
    this.notify("initialized", {});
    return init;
  }

  #read(chunk) {
    this.#buffer = Buffer.concat([this.#buffer, chunk]);
    for (;;) {
      const end = this.#buffer.indexOf("\r\n\r\n");
      if (end < 0) return;
      const length = Number(/Content-Length: (\d+)/i.exec(this.#buffer.subarray(0, end).toString())?.[1]);
      if (this.#buffer.length < end + 4 + length) return;
      const message = JSON.parse(this.#buffer.subarray(end + 4, end + 4 + length).toString());
      this.#buffer = this.#buffer.subarray(end + 4 + length);
      this.#handle(message);
    }
  }

  #handle(message) {
    if (message.method && message.id !== undefined) return this.#answerCore(message);
    if (message.method) {
      this.#notifications.push(message);
      this.#waiters = this.#waiters.filter((w) => !w());
      return;
    }
    const pending = this.#pending.get(message.id);
    this.#pending.delete(message.id);
    if (message.error) pending.reject(new Error(`${message.error.message} (${JSON.stringify(message.error)})`));
    else pending.resolve(message.result);
  }

  /** The core asked the editor for something: only `workspace/applyEdit` is expected. */
  #answerCore(request) {
    if (request.method !== "workspace/applyEdit") throw new Error(`the core asked for ${request.method}`);
    for (const change of request.params.edit.documentChanges) {
      const path = fileURLToPath(change.textDocument.uri);
      writeFileSync(path, applyEdits(readFileSync(path, "utf8"), change.edits));
    }
    this.#send({ jsonrpc: "2.0", id: request.id, result: { applied: true } });
  }

  #send(message) {
    const body = Buffer.from(JSON.stringify(message));
    this.#child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    this.#child.stdin.write(body);
  }

  notify(method, params) {
    this.#send({ jsonrpc: "2.0", method, params });
  }

  request(method, params) {
    const id = ++this.#next;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#send({ jsonrpc: "2.0", id, method, params });
    });
  }

  /** Resolves with the params of the next `method` notification (one that already came counts). */
  waitFor(method, ms = 60_000) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`no ${method} in ${ms} ms`)), ms);
      const take = () => {
        const at = this.#notifications.findIndex((n) => n.method === method);
        if (at < 0) return false;
        clearTimeout(timer);
        resolve(this.#notifications.splice(at, 1)[0].params);
        return true;
      };
      if (!take()) this.#waiters.push(take);
    });
  }

  /** Tells the core these files changed on disk and waits until it has read them again. */
  async filesChanged(paths) {
    this.#notifications = this.#notifications.filter((n) => n.method !== "vnoc/indexChanged");
    this.notify("vnoc/filesChanged", { paths });
    await this.waitFor("vnoc/indexChanged");
  }

  async close() {
    await this.request("shutdown", null);
    this.notify("exit", null);
    await new Promise((resolve) => this.#child.once("exit", resolve));
  }
}

export const uriOf = (file) => pathToFileURL(file).toString();

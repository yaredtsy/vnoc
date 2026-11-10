// The bridge to the extension host: a webview cannot reach the core or any file, so it posts a request and waits for the answer, listens
// for events, and posts notes. All three are typed by `@vnoc/shared/messages.ts`: a name that is not in that file does not compile.

import type { Events, Notes, Requests } from "@vnoc/shared/messages.ts";

declare function acquireVsCodeApi(): {
  postMessage(message: unknown): void;
  getState<T>(): T | undefined;
  /** returns the state it was given (so `useEffect(() => vscode.setState(x))` is a type error: React would call the result) */
  setState<T>(state: T): T;
};

/** `acquireVsCodeApi` may be called once per webview. */
export const vscode = acquireVsCodeApi();

let nextId = 0;
const pending = new Map<number, { resolve: (v: any) => void; reject: (e: Error) => void }>();

window.addEventListener("message", (e: MessageEvent) => {
  if (e.data?.type !== "res") return;
  const p = pending.get(e.data.id);
  if (!p) return;
  pending.delete(e.data.id);
  if (e.data.error) p.reject(new Error(e.data.error));
  else p.resolve(e.data.result);
});

type Params<M extends keyof Requests> = Requests[M]["params"] extends void ? [] : [Requests[M]["params"]];

/** Asks the host something; the method name decides the parameters and the result. */
export function request<M extends keyof Requests>(method: M, ...params: Params<M>): Promise<Requests[M]["result"]> {
  const id = ++nextId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    vscode.postMessage({ type: "req", id, method, params: params[0] });
  });
}

/** Listens for one kind of event from the host. Returns the way to stop listening. */
export function on<E extends keyof Events>(name: E, callback: (data: Events[E]) => void): () => void {
  const listener = (e: MessageEvent) => {
    if (e.data?.type === "event" && e.data.name === name) callback(e.data.data);
  };
  window.addEventListener("message", listener);
  return () => window.removeEventListener("message", listener);
}

/** Tells the host something that needs no answer. */
export function post(note: Notes): void {
  vscode.postMessage(note);
}

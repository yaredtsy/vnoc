// The bridge to the extension host: the webview cannot reach the core, so it posts a request and waits for the answer.

declare function acquireVsCodeApi(): {
  postMessage(message: unknown): void;
  getState<T>(): T | undefined;
  setState<T>(state: T): void;
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

export function request<T>(method: "structure" | "ping" | "pendingFocus" | "code" | "addDoc", params?: object): Promise<T> {
  const id = ++nextId;
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve, reject });
    vscode.postMessage({ type: "req", id, method, params });
  });
}

/** The core's index changed (a save, a branch switch): the host tells the canvas to ask for the structure again. */
export function onIndexChanged(callback: () => void): () => void {
  const listener = (e: MessageEvent) => {
    if (e.data?.type === "indexChanged") callback();
  };
  window.addEventListener("message", listener);
  return () => window.removeEventListener("message", listener);
}

/** The sidebar's Focus button: the host tells an open canvas which node to zoom into. Returns the way to stop listening. */
export function onFocus(callback: (id: string) => void): () => void {
  const listener = (e: MessageEvent) => {
    if (e.data?.type === "focus" && typeof e.data.id === "string") callback(e.data.id);
  };
  window.addEventListener("message", listener);
  return () => window.removeEventListener("message", listener);
}

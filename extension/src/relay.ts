import type { CallTreeResult, Events, EventMessage, Range, RequestMessage, Requests, ResponseMessage, StructureResult } from "@vnoc/shared/messages";
import type { ContextId, Session } from "@vnoc/shared/session.ts";

import type { Docs } from "./docs";

// What a webview may ask of the extension: one handler per request in `shared/messages.ts`. Leave one out and TypeScript says which.

export type Handlers = { [M in keyof Requests]: (params: Requests[M]["params"]) => Promise<Requests[M]["result"]> };

/** What the handlers need from the rest of the extension. */
export interface Deps {
  structure(): Promise<StructureResult>;
  /** a request to the core */
  core<T>(method: string, params?: object): Promise<T>;
  /** the "Add doc" flow for a node; resolves to whether a doc was added */
  addDoc(key: string): Promise<boolean>;
  docs: Docs;
  /** the text of a definition, from VS Code's document */
  code(path: string, range: Range): Promise<string>;
  goToCode(key: string): Promise<void>;
  /** the call trees held, by entry key */
  callTrees(): Record<string, CallTreeResult>;
  /** opens a file below the opened folder at a range */
  openAt(path: string, range: Range): Promise<void>;
  session(): Session;
  /** the list of call sites, with a preview as it is moved through */
  pickCallSite(sites: { path: string; range: Range }[]): Promise<void>;
  /** shows the NEEDS ATTENTION view */
  focusAttention(): Promise<void>;
  /** reveals the output channel */
  showLog(): Promise<void>;
  /** fetches a call tree again after it failed */
  retryCalls(key: string): Promise<void>;
  /** a click on a sidebar row: the click rule (07 §4) */
  rowClick(context: ContextId, id: string): Promise<void>;
  /** a button of the sidebar's states */
  command(name: "openFolder" | "refreshStructure"): Promise<void>;
}

export function createHandlers(d: Deps): Handlers {
  return {
    structure: () => d.structure(),
    docs: ({ nodeId }) => d.core("vnoc/docs", { id: nodeId }), // the core calls the permanent ID `id`; every other id in the UI is the location key
    docText: ({ path }) => d.docs.text(path),
    code: ({ path, range }) => d.code(path, range),
    addDoc: async ({ key }) => ({ added: await d.addDoc(key) }),
    openDoc: ({ path }) => d.docs.open(path),
    openLink: ({ href, from }) => d.docs.openLink(href, from),
    goToCode: ({ key }) => d.goToCode(key),
    callTrees: async () => d.callTrees(),
    openAt: ({ path, range }) => d.openAt(path, range),
    session: async () => d.session(),
    pickCallSite: ({ sites }) => d.pickCallSite(sites),
    focusAttention: () => d.focusAttention(),
    showLog: () => d.showLog(),
    retryCalls: ({ key }) => d.retryCalls(key),
    rowClick: ({ context, id }) => d.rowClick(context, id),
    command: ({ name }) => d.command(name),
  };
}

/** Serves one request message: the handler for its method, and the answer (or the error) as the message to post back. */
export async function serve(handlers: Handlers, message: RequestMessage): Promise<ResponseMessage> {
  const handler = handlers[message.method] as ((params: unknown) => Promise<unknown>) | undefined;
  try {
    if (!handler) throw new Error(`unknown request ${String(message.method)}`);
    return { type: "res", id: message.id, result: await handler(message.params) };
  } catch (e) {
    return { type: "res", id: message.id, error: String((e as Error).message ?? e) };
  }
}

/** The message that carries an event to a webview. */
export function event<E extends keyof Events>(name: E, data: Events[E]): EventMessage {
  return { type: "event", name, data } as EventMessage;
}

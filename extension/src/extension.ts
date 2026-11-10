import * as vscode from "vscode";

import type { Notes, TreeNode } from "@vnoc/shared/messages";
import type { ContextState } from "@vnoc/shared/session.ts";
import { Commands } from "./commands";
import { Core, watchFiles } from "./core";
import { Docs } from "./docs";
import { CANVAS, DOCS, Panels } from "./panels";
import { createHandlers } from "./relay";
import { SessionStore } from "./session";
import { StructureView } from "./structureView";

// Wires the pieces together: the core, the session, the two webview tabs, the sidebar, the commands. Nothing here decides anything.

/** What the webview last said it drew (for the test suite). */
let rendered: { visible: number; hidden: number } | undefined;
const renderWaiters: ((r: { visible: number; hidden: number }) => void)[] = [];

let core: Core | undefined;

export function activate(ctx: vscode.ExtensionContext) {
  const output = vscode.window.createOutputChannel("V-NOC");
  let panels: Panels;
  let sidebar: StructureView;
  let sidebarView: vscode.TreeView<TreeNode>;
  let commands: Commands;

  /** The core has a new index: the sidebar redraws, and every open tab asks for the new structure. */
  const indexChanged = (revision: number) => {
    refreshSidebar();
    panels.emit("indexChanged", { revision });
  };
  /** Redraws the sidebar from the new structure, then the header (its names come from that structure). */
  const refreshSidebar = () => void sidebar.refresh().then(showFocusInSidebar);
  /** The sidebar starts at the focused node and says so in its header. */
  const showFocusInSidebar = () => {
    const stack = store.get().contexts[store.get().active].focus;
    sidebar.setFocus(stack[stack.length - 1]);
    const names = stack.map((id) => sidebar.find(id)?.name ?? "?");
    sidebarView.description = names.length > 0 ? `Focus: ${names.join(" › ")}` : undefined;
  };

  const c = new Core(ctx, output, indexChanged);
  core = c;
  const store = new SessionStore(ctx.workspaceState, () => c.tree);
  const docs = new Docs(() => c.gitRoot, (name, data) => panels.emit(name, data));
  // what a webview may ask of the extension (`shared/messages.ts` lists it, `relay.ts` serves it)
  const handlers = createHandlers({
    structure: () => c.loadStructure(),
    core: (method, params) => c.request(method, params),
    addDoc: (key) => commands.addDocTo(key),
    docs,
    code: (path, range) => commands.codeOf(path, range),
    goToCode: (key) => commands.goToCodeOf(key),
    session: () => store.get(),
  });
  panels = new Panels(ctx, handlers, (note) => onNote(note, store, output));
  commands = new Commands(c, store, panels);

  sidebar = new StructureView(async () => (await c.loadStructure()).root);
  sidebarView = vscode.window.createTreeView("vnoc.structure", { treeDataProvider: sidebar });
  showFocusInSidebar(); // the focus saved with the workspace
  refreshSidebar();

  // Registered synchronously, before anything is awaited: on restart VS Code waits for these serializers (S1).
  ctx.subscriptions.push(
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    sidebarView,
    ...commands.register(),
    watchFiles(c, (fsPath) => docs.changed(fsPath)),
    docs.watchEditors(),
    // every change of the session goes to the open webviews, whole; the sidebar redraws its header when the focus moved
    store.onChange(({ prev, next }) => {
      panels.emit("session", next);
      if (prev.contexts[prev.active].focus !== next.contexts[next.active].focus) showFocusInSidebar();
    }),
    // what the session points at may be gone (a focused node you deleted: you are one step back, not looking at nothing)
    c.onStructure(() => {
      store.dispatch({ type: "treeChanged" });
      commands.syncCursor();
    }),
    { dispose: () => void c.stop() },
  );

  // What the test suite drives.
  return {
    session: () => store.get(),
    /** what the next window restart would read back */
    saved: () => ctx.workspaceState.get<{ contexts: Record<string, ContextState> }>("vnoc.session.v1"),
    ping: () => c.request<{ pong: boolean; pid: number; version: string }>("vnoc/ping"),
    structure: () => c.request<any>("vnoc/structure"),
    /** Resolves when a webview reports that it drew the structure (immediately if one already has). */
    waitRendered: (timeoutMs: number) =>
      new Promise<{ visible: number; hidden: number }>((resolve, reject) => {
        if (rendered) return resolve(rendered);
        const t = setTimeout(() => reject(new Error("the webview did not report a drawing")), timeoutMs);
        renderWaiters.push((r) => (clearTimeout(t), resolve(r)));
      }),
  };
}

/** What a webview says that needs no answer. */
function onNote(note: Notes, store: SessionStore, output: vscode.OutputChannel) {
  switch (note.type) {
    case "intent":
      store.dispatch(note.intent, note.context);
      break;
    case "rendered":
      rendered = { visible: note.visible, hidden: note.hidden };
      output.appendLine(`webview drew ${note.visible} nodes (${note.hidden} hidden)`);
      renderWaiters.splice(0).forEach((w) => w(rendered!));
      break;
    case "close":
      break; // the panel closes itself (panels.ts)
  }
}

export async function deactivate() {
  await core?.stop();
}

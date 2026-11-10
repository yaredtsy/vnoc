import * as vscode from "vscode";

import type { Notes } from "@vnoc/shared/messages";
import type { ContextState } from "@vnoc/shared/session.ts";
import { Commands } from "./commands";
import { Core, watchFiles } from "./core";
import { CursorMark } from "./cursorMark";
import { Docs } from "./docs";
import { FocusView } from "./focusView";
import { CANVAS, DOCS, Panels } from "./panels";
import { createHandlers } from "./relay";
import { SessionStore } from "./session";
import { type Row, StructureView } from "./structureView";

// Wires the pieces together: the core, the session, the two webview tabs, the sidebar, the commands. Nothing here decides anything.

/** What the webview last said it drew (for the test suite). */
let rendered: { visible: number; hidden: number } | undefined;
const renderWaiters: ((r: { visible: number; hidden: number }) => void)[] = [];

let core: Core | undefined;

export function activate(ctx: vscode.ExtensionContext) {
  const output = vscode.window.createOutputChannel("V-NOC");
  let panels: Panels;
  let sidebar: StructureView;
  let sidebarView: vscode.TreeView<Row>;
  const focusView = new FocusView();
  const cursorMark = new CursorMark();
  let commands: Commands;

  /** The core has a new index: the sidebar redraws, and every open tab asks for the new structure. */
  const indexChanged = (revision: number) => {
    refreshSidebar();
    panels.emit("indexChanged", { revision });
  };
  /** Redraws the sidebar from the new structure, then the header (its names come from that structure). */
  const refreshSidebar = () => void sidebar.refresh().then(showFocusInSidebar);
  /** The tree starts at the focused node (with the focus bar above it), and the FOCUS view lists every step. */
  const showFocusInSidebar = () => {
    const stack = store.get().contexts[store.get().active].focus;
    sidebar.setFocus(stack);
    focusView.update(stack, (id) => sidebar.find(id));
  };
  /** The row of the selected node, selected in the tree: both surfaces show the same node. (`reveal` fires the selection event; `reduce` returns the same session for it, so nothing loops.) */
  const revealSelection = () => {
    const id = store.get().contexts[store.get().active].selected;
    const row = id ? sidebar.rowForId(id) : undefined;
    if (row && sidebarView.visible) void Promise.resolve(sidebarView.reveal(row, { select: true, focus: false })).catch(() => {});
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
  commands = new Commands(c, store, panels, async (id) => {
    const row = sidebar.rowForId(id);
    if (row) await sidebarView.reveal(row, { select: true, focus: true, expand: false }); // shows the V-NOC side bar too
  });

  sidebar = new StructureView(async () => (await c.loadStructure()).root);
  sidebarView = vscode.window.createTreeView("vnoc.structure", { treeDataProvider: sidebar, showCollapseAll: true });
  const focusTree = vscode.window.createTreeView("vnoc.focusStack", { treeDataProvider: focusView });
  showFocusInSidebar(); // the focus saved with the workspace

  // Registered synchronously, before anything is awaited: on restart VS Code waits for these serializers (S1).
  ctx.subscriptions.push(
    vscode.window.registerWebviewPanelSerializer(CANVAS, { deserializeWebviewPanel: async (panel) => panels.restore("canvas", panel) }),
    vscode.window.registerWebviewPanelSerializer(DOCS, { deserializeWebviewPanel: async (panel) => panels.restore("docs", panel) }),
    sidebarView,
    focusTree,
    vscode.window.registerFileDecorationProvider(cursorMark),
    // the sidebar's four states: loading and error say so in the view, empty and error also show the welcome text (package.json `viewsWelcome`)
    sidebar.onDidChangeState((state) => {
      sidebarView.message = state.kind === "loading" ? "Reading the code…" : state.kind === "error" ? state.message : undefined;
      void vscode.commands.executeCommand("setContext", "vnoc.sidebarState", state.kind);
    }),
    // a click or the arrow keys on a row select it; `select` never opens anything (opening is `vnoc.open`'s job)
    sidebarView.onDidChangeSelection((e) => {
      const row = e.selection[0];
      if (row?.row === "node") store.dispatch({ type: "select", id: row.node.id });
    }),
    sidebarView.onDidChangeVisibility((e) => e.visible && revealSelection()),
    ...commands.register(),
    watchFiles(c, (fsPath) => docs.changed(fsPath)),
    docs.watchEditors(),
    // every change of the session goes to the open webviews, whole; the sidebar redraws its header when the focus moved
    store.onChange(({ prev, next }) => {
      panels.emit("session", next);
      if (prev.contexts[prev.active].focus !== next.contexts[next.active].focus) showFocusInSidebar();
      if (prev.contexts[prev.active].selected !== next.contexts[next.active].selected) revealSelection();
      if (prev.cursor !== next.cursor) {
        cursorMark.update(next.cursor);
        sidebar.setCursor(next.cursor);
      }
    }),
    // what the session points at may be gone (a focused node you deleted: you are one step back, not looking at nothing)
    c.onStructure(() => {
      store.dispatch({ type: "treeChanged" });
      commands.syncCursor();
    }),
    { dispose: () => void c.stop() },
  );

  // the files whose menus carry the V-NOC submenu (later: from the driver manifests)
  void vscode.commands.executeCommand("setContext", "vnoc.extensions", [".py", ".pyi"]);
  refreshSidebar(); // last: the listeners above hear the "loading" state

  // What the test suite drives.
  return {
    session: () => store.get(),
    /** what the next window restart would read back */
    saved: () => ctx.workspaceState.get<{ contexts: Record<string, ContextState> }>("vnoc.session.v1"),
    ping: () => c.request<{ pong: boolean; pid: number; version: string }>("vnoc/ping"),
    structure: () => c.request<any>("vnoc/structure"),
    /** selects a node's row in the sidebar tree, as a click would (the session hears it from `onDidChangeSelection`) */
    selectInTree: async (id: string) => {
      const row = sidebar.rowForId(id);
      if (row) await sidebarView.reveal(row, { select: true, focus: false });
    },
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

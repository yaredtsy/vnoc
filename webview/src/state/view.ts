// What only this webview draws: which cards show their code, the doc panel, the docs tab's pin. Owned here, and saved with `vscode.setState` by the one
// subscriber at the bottom, so it survives hiding and showing the tab and a window reload. (Everything a second surface shows is the session.)

import { create } from "zustand";

import { vscode } from "../host.ts";

interface Saved {
  codeOpen?: string[];
  docsOpen?: boolean;
  docWidth?: number;
  minimap?: boolean;
  pinned?: string;
}

interface View {
  /** cards that show their code instead of the summary */
  codeOpen: ReadonlySet<string>;
  /** the canvas's doc panel, and its width */
  docsOpen: boolean;
  docWidth: number;
  /** the canvas's minimap: off unless asked for (the bar's ⋯ menu) */
  minimap: boolean;
  /** the docs tab: the node it is held on (a tab opened by "Show Docs" on a row or a card starts pinned; the page carries it, as a message could be lost) */
  pinned: string | null;
  // not saved: how the canvas points at things (docs/10-ui-rework/06-how-it-looks.md)
  /** a crumb under the pointer: "look here" without selecting */
  hover: string | null;
  /** a card the canvas just brought into view: it pulses for about a second */
  revealed: string | null;
  /** the drawn cards that hold the selection / the cursor inside a closed card: they show a dot */
  hidesSelected: string | null;
  hidesCursor: string | null;
}

const saved = vscode.getState<Saved>() ?? {};

export const useView = create<View>(() => ({
  codeOpen: new Set(saved.codeOpen),
  docsOpen: saved.docsOpen ?? false,
  docWidth: saved.docWidth ?? 380,
  minimap: saved.minimap ?? false,
  pinned: document.getElementById("root")?.dataset.node ?? saved.pinned ?? null,
  hover: null,
  revealed: null,
  hidesSelected: null,
  hidesCursor: null,
}));

export const setHover = (hover: string | null) => useView.setState({ hover });

/** Marks a card as just revealed; the mark goes away by itself (the pulse in node-card.css lasts about as long). */
export function pulse(id: string): void {
  useView.setState({ revealed: id });
  setTimeout(() => useView.setState((v) => (v.revealed === id ? { revealed: null } : v)), 1000);
}

export function toggleCode(id: string): void {
  useView.setState(({ codeOpen }) => {
    const next = new Set(codeOpen);
    if (!next.delete(id)) next.add(id);
    return { codeOpen: next };
  });
}

/** Shows or hides one card's code. */
export function setCodeOpen(id: string, open: boolean): void {
  useView.setState(({ codeOpen }) => {
    const next = new Set(codeOpen);
    if (open) next.add(id);
    else next.delete(id);
    return { codeOpen: next };
  });
}

export const setDocsOpen = (docsOpen: boolean) => useView.setState({ docsOpen });
export const setMinimap = (minimap: boolean) => useView.setState({ minimap });
export const setDocWidth = (docWidth: number) => useView.setState({ docWidth });
export const pin = (pinned: string | null) => useView.setState({ pinned });

// the one place the view is saved
useView.subscribe(({ codeOpen, docsOpen, docWidth, minimap, pinned }) => {
  vscode.setState<Saved>({ codeOpen: [...codeOpen], docsOpen, docWidth, minimap, pinned: pinned ?? undefined });
});

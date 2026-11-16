// What only this webview draws: which cards show their code, the doc panel, the docs tab's pin. Owned here, and saved with `vscode.setState` by the one
// subscriber at the bottom, so it survives hiding and showing the tab and a window reload. (Everything a second surface shows is the session.)

import { create } from "zustand";

import { vscode } from "../host.ts";
import { type Drafts } from "./draftRules.ts";
import { useDrafts } from "./drafts.ts";

interface Saved {
  codeOpen?: string[];
  docsOpen?: boolean;
  docWidth?: number;
  minimap?: boolean;
  pinned?: string;
  panelShares?: Record<string, number>;
  /** the drafts of the cards being edited (docs/17 E6): in the one object, so the one `setState` below keeps them */
  drafts?: Drafts;
}

interface View {
  /** cards that show their code instead of the summary */
  codeOpen: ReadonlySet<string>;
  /** call cards whose list of sites is open (`N more ▾`); not saved */
  sitesOpen: ReadonlySet<string>;
  /** the canvas's doc panel, and its width */
  docsOpen: boolean;
  docWidth: number;
  /** the canvas's minimap: off unless asked for (the bar's ⋯ menu) */
  minimap: boolean;
  /** the sidebar's EXPLORE panels: a share of the view's height, kept per depth (1 = an EXPLORE of MAIN, 2 = an EXPLORE of an EXPLORE; 06 §4.1) */
  panelShares: Readonly<Record<string, number>>;
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
  sitesOpen: new Set(),
  docsOpen: saved.docsOpen ?? false,
  docWidth: saved.docWidth ?? 380,
  minimap: saved.minimap ?? false,
  panelShares: saved.panelShares ?? {},
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

export function toggleSites(id: string): void {
  useView.setState(({ sitesOpen }) => {
    const next = new Set(sitesOpen);
    if (!next.delete(id)) next.add(id);
    return { sitesOpen: next };
  });
}

export const setDocsOpen = (docsOpen: boolean) => useView.setState({ docsOpen });
export const setMinimap = (minimap: boolean) => useView.setState({ minimap });
export const setDocWidth = (docWidth: number) => useView.setState({ docWidth });
export const pin = (pinned: string | null) => useView.setState({ pinned });
/** Keeps the shares of the panels at these depths (a drag or a double-click on a splitter). */
export const setPanelShares = (shares: Record<string, number>) => useView.setState(({ panelShares }) => ({ panelShares: { ...panelShares, ...shares } }));

// the one place the view is saved: the view and the drafts are one object, because `setState` keeps one object per webview (a second call would wipe this one)
function save() {
  const { codeOpen, docsOpen, docWidth, minimap, pinned, panelShares } = useView.getState();
  vscode.setState<Saved>({ codeOpen: [...codeOpen], docsOpen, docWidth, minimap, pinned: pinned ?? undefined, panelShares, drafts: useDrafts.getState().drafts });
}
useView.subscribe(save);
useDrafts.subscribe(save);

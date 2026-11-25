// What only this webview draws: which cards show their code, the doc panel, the docs tab's pin. Owned here, and saved with `vscode.setState` by the one
// subscriber at the bottom, so it survives hiding and showing the tab and a window reload. (Everything a second surface shows is the session.)

import { create } from "zustand";

import { vscode } from "../host.ts";
import { DEFAULT_TOURS_SHARE } from "../sidebar/tours/split.ts";
import { type Drafts } from "./draftRules.ts";
import { useDrafts } from "./drafts.ts";

interface Saved {
  codeOpen?: string[];
  docsOpen?: boolean;
  docWidth?: number;
  minimap?: boolean;
  pinned?: string;
  panelShares?: Record<string, number>;
  /** the tour's stage (docs/26-tours/design/03-stage.md §6): its height, the width of its text pane (null: 40%), and whether it is folded to the mini bar. Kept per window */
  stageHeight?: number;
  stageText?: number | null;
  stageFolded?: boolean;
  /** the sidebar's TOURS section is folded (it is, until a tour is opened) */
  toursFolded?: boolean;
  /** the TOURS section's share of the sidebar's height (the splitter between the tree and it): kept per window */
  toursShare?: number;
  /** the drafts of the cards being edited (docs/17 E6): in the one object, so the one `setState` below keeps them */
  drafts?: Drafts;
}

/** What a card's code needs: columns for its width, lines for its height. */
export interface CodeSize {
  columns: number;
  lines: number;
}

interface View {
  /** cards that show their code instead of the summary */
  codeOpen: ReadonlySet<string>;
  /** call cards whose list of sites is open (`N more ▾`); not saved */
  sitesOpen: ReadonlySet<string>;
  /** for each card that shows its code, how many characters wide (`codeColumns`) and how many lines tall the code it read is: the layout makes the card as wide and its code view as tall; not saved */
  codeSize: Readonly<Record<string, CodeSize>>;
  /** the canvas's doc panel, and its width */
  docsOpen: boolean;
  docWidth: number;
  /** the canvas's minimap: off unless asked for (the bar's ⋯ menu) */
  minimap: boolean;
  /** the sidebar's EXPLORE panels: a share of the view's height, kept per depth (1 = an EXPLORE of MAIN, 2 = an EXPLORE of an EXPLORE; 06 §4.1) */
  panelShares: Readonly<Record<string, number>>;
  /** the tour's stage: its height in px, the width of its text pane in px (null: 40% of the width, between 320 and 460), folded to the mini bar */
  stageHeight: number;
  stageText: number | null;
  stageFolded: boolean;
  /** the sidebar's TOURS section, folded */
  toursFolded: boolean;
  /** the TOURS section's share of the sidebar's height */
  toursShare: number;
  /** the docs tab: the node it is held on (a tab opened by "Show Docs" on a row or a card starts pinned; the page carries it, as a message could be lost) */
  pinned: string | null;
  // not saved: how the canvas points at things (docs/10-ui-rework/06-how-it-looks.md)
  /** a crumb under the pointer: "look here" without selecting */
  hover: string | null;
  /** a card the canvas just brought into view: it pulses for about a second */
  revealed: string | null;
  /** the cards the canvas dims because the sidebar's search does not find them (`canvas/useSearchDim.ts`); null: none */
  dimmed: ReadonlySet<string> | null;
  /** the drawn cards that hold the selection / the cursor inside a closed card: they show a dot */
  hidesSelected: string | null;
  hidesCursor: string | null;
  /** "Edit in Card" (and a doc's pencil) asks the editor of this card or doc to take the keyboard, once it exists; the editor clears it (9.7). `n` makes a second ask a new one */
  focusCode: { id: string; n: number } | null;
}

const saved = vscode.getState<Saved>() ?? {};

export const useView = create<View>(() => ({
  codeOpen: new Set(saved.codeOpen),
  sitesOpen: new Set(),
  codeSize: {},
  docsOpen: saved.docsOpen ?? false,
  docWidth: saved.docWidth ?? 380,
  minimap: saved.minimap ?? false,
  panelShares: saved.panelShares ?? {},
  stageHeight: saved.stageHeight ?? 215,
  stageText: saved.stageText ?? null,
  stageFolded: saved.stageFolded ?? false,
  toursFolded: saved.toursFolded ?? true,
  toursShare: saved.toursShare ?? DEFAULT_TOURS_SHARE,
  pinned: document.getElementById("root")?.dataset.node ?? saved.pinned ?? null,
  hover: null,
  revealed: null,
  dimmed: null,
  hidesSelected: null,
  hidesCursor: null,
  focusCode: null,
}));

/** Asks the editor of this card (or doc) to take the keyboard, so typing starts the draft. */
export const requestCodeFocus = (id: string) => useView.setState(({ focusCode }) => ({ focusCode: { id, n: (focusCode?.n ?? 0) + 1 } }));
/** The editor took it: nothing is waiting any more. Only the ask it answered is cleared. */
export const clearCodeFocus = (id: string, n: number) => useView.setState(({ focusCode }) => (focusCode?.id === id && focusCode.n === n ? { focusCode: null } : {}));

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

/** The code of this card was read: its view must be this many characters wide and this many lines tall (null: it has none any more). Said only when it changed, so no layout is made for nothing. */
export function setCodeSize(id: string, size: CodeSize | null): void {
  useView.setState(({ codeSize }) => {
    const had = codeSize[id] ?? null;
    if (had?.columns === size?.columns && had?.lines === size?.lines) return {};
    const { [id]: _, ...rest } = codeSize;
    return { codeSize: size === null ? rest : { ...rest, [id]: size } };
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
export const setStageHeight = (stageHeight: number) => useView.setState({ stageHeight });
export const setStageText = (stageText: number | null) => useView.setState({ stageText });
export const setToursFolded = (toursFolded: boolean) => useView.setState({ toursFolded });
export const setToursShare = (toursShare: number) => useView.setState({ toursShare });
export const setStageFolded = (stageFolded: boolean) => useView.setState({ stageFolded });
export const setDocWidth = (docWidth: number) => useView.setState({ docWidth });
export const pin = (pinned: string | null) => useView.setState({ pinned });
/** Keeps the shares of the panels at these depths (a drag or a double-click on a splitter). */
export const setPanelShares = (shares: Record<string, number>) => useView.setState(({ panelShares }) => ({ panelShares: { ...panelShares, ...shares } }));

// the one place the view is saved: the view and the drafts are one object, because `setState` keeps one object per webview (a second call would wipe this one)
function save() {
  const { codeOpen, docsOpen, docWidth, minimap, pinned, panelShares, stageHeight, stageText, stageFolded, toursFolded, toursShare } = useView.getState();
  vscode.setState<Saved>({ codeOpen: [...codeOpen], docsOpen, docWidth, minimap, pinned: pinned ?? undefined, panelShares, stageHeight, stageText, stageFolded, toursFolded, toursShare, drafts: useDrafts.getState().drafts });
}
useView.subscribe(save);
useDrafts.subscribe(save);

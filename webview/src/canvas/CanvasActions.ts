// What a card may ask the canvas to do. A context (not a callback in each node's data) so the nodes array does not change when the handler does,
// and so cards deeper in the tree (header, later the code toggle) reach it without passing props down.

import { createContext, useContext } from "react";

export interface CanvasActions {
  /** expand or collapse a node's children */
  toggle(id: string): void;
  /** show or hide a node's code inside its card */
  toggleCode(id: string): void;
}

const Ctx = createContext<CanvasActions | null>(null);

export const CanvasActionsProvider = Ctx.Provider;

export function useCanvasActions(): CanvasActions {
  const actions = useContext(Ctx);
  if (!actions) throw new Error("useCanvasActions: no <CanvasActionsProvider> above this card");
  return actions;
}

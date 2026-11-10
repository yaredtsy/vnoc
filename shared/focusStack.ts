// The focus stack (plan 09-02): which node the canvas is zoomed into. Empty = the whole tree; the last id is the node on screen, the ids before it
// are the way back (a crumb each). Pure functions on an immutable array, so React state can hold it and the rules are tested without a browser.

export type FocusStack = readonly string[];

/**
 * Focus `id`. Focusing what is already on top changes nothing (a double-click on the root of the view), and focusing a node that is
 * already lower in the stack goes back to it instead of adding it a second time, so the crumbs stay short.
 */
export function push(stack: FocusStack, id: string): FocusStack {
  if (stack[stack.length - 1] === id) return stack;
  const at = stack.indexOf(id);
  return at >= 0 ? stack.slice(0, at + 1) : [...stack, id];
}

/** Go back one level. */
export function pop(stack: FocusStack): FocusStack {
  return stack.slice(0, -1);
}

/** Keep the first `depth` entries: clicking a crumb jumps back to it. */
export function jump(stack: FocusStack, depth: number): FocusStack {
  return depth >= stack.length ? stack : stack.slice(0, Math.max(depth, 0));
}

/** The "node on top disappeared" rule: after a refresh, drop entries from the top until the one left still exists. */
export function prune(stack: FocusStack, exists: (id: string) => boolean): FocusStack {
  let n = stack.length;
  while (n > 0 && !exists(stack[n - 1])) n--;
  return n === stack.length ? stack : stack.slice(0, n);
}

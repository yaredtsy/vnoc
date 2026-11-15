import * as vscode from "vscode";

// The STRUCTURE view's loading state (docs/16-ui-redesign/06-sidebar.md §8): while the first read of the code is on its way the view shows VS Code's own thin
// progress line (the page draws the skeleton rows). Later reads, when the index changes under rows already drawn, are quiet.

let starts = 0;

/** Runs the first read of the structure with the progress line on the view; it ends when the read does, whether it worked or not. */
export function readingTheCode<T>(read: () => Promise<T>): Promise<T> {
  starts++;
  return Promise.resolve(vscode.window.withProgress({ location: { viewId: "vnoc.structure" } }, read));
}

/** How many times the line was started (the test suite reads it). */
export const readingStarts = (): number => starts;

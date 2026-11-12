import * as vscode from "vscode";

import type { CallTreeResult } from "@vnoc/shared/messages";
import type { Core } from "./core";

/** The most call trees kept at once: the oldest goes first. */
const KEEP = 50;

/**
 * The call trees the extension holds: the latest one for each entry, until the window closes. They are not part of the session: they are large, and
 * mean nothing after a restart (the code may have changed). A canvas that opens later asks for them (`callTrees`); one that is open hears each new one.
 */
export class CallTrees {
  private readonly trees = new Map<string, CallTreeResult>();

  constructor(
    private readonly core: Core,
    /** a tree arrived: tell the open canvases */
    private readonly arrived: (key: string, result: CallTreeResult) => void,
  ) {}

  all(): Record<string, CallTreeResult> {
    return Object.fromEntries(this.trees);
  }

  /**
   * Asks the core for the tree of the node `key`, with a progress notification and a Cancel button: cancelling sends `$/cancelRequest`, and the core
   * cancels the driver under it. Resolves to whether a tree arrived (not when it was cancelled); any other failure is thrown.
   */
  async run(key: string, name: string): Promise<boolean> {
    const result = await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: `V-NOC: call tree of ${name}…`, cancellable: true }, async (_progress, token) => {
      try {
        return await this.core.request<CallTreeResult>("vnoc/callTree", { key }, token);
      } catch (e) {
        if (token.isCancellationRequested) return undefined;
        throw e;
      }
    });
    if (!result) return false;
    this.trees.delete(key); // a new one is the newest again
    this.trees.set(key, result);
    for (const oldest of [...this.trees.keys()].slice(0, Math.max(0, this.trees.size - KEEP))) this.trees.delete(oldest);
    this.arrived(key, result);
    return true;
  }
}

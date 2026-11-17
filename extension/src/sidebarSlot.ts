// The STRUCTURE view that is current, whether its page has drawn, and what happens when it goes (docs/20-search/05-on-screen.md §3). Nothing here touches `vscode`, so
// the rules are tested in Node: `Surfaces` gives it the view's events.

export class SidebarSlot<V> {
  private current: V | undefined;
  /** The current page has drawn at least once, so it is listening for the events a command sends it. */
  private drew = false;
  private waiters: (() => void)[] = [];

  /** `closed`: the current view was disposed (VS Code disposes the page when the view moves or the window reloads webviews, and the page's own cleanup does not run). */
  private readonly closed: () => void;

  constructor(closed: () => void) {
    this.closed = closed;
  }

  get view(): V | undefined {
    return this.current;
  }

  /** A view was resolved: it is the current one, and it has not drawn, so nothing it is sent is heard until it has. */
  adopt(view: V): void {
    this.current = view;
    this.drew = false;
  }

  /** The view's page drew. Only the current one counts. */
  rendered(view: V): void {
    if (this.current !== view) return;
    this.drew = true;
    this.waiters.splice(0).forEach((w) => w());
  }

  /** The view was disposed. One that was already replaced changes nothing; the current one leaves no page, so `closed` runs. */
  disposed(view: V): void {
    if (this.current !== view) return;
    this.current = undefined;
    this.drew = false;
    this.closed();
  }

  /** Resolves when the page has drawn (at once if it has), or after `ms`: a page with no folder open never draws a list, and the command that waits must not hang. */
  whenDrew(ms: number): Promise<void> {
    if (this.drew) return Promise.resolve();
    return new Promise((resolve) => {
      const done = () => (clearTimeout(timer), resolve());
      const timer = setTimeout(() => {
        this.waiters = this.waiters.filter((w) => w !== done);
        resolve();
      }, ms);
      this.waiters.push(done);
    });
  }
}

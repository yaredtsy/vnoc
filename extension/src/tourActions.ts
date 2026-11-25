import type { SettingChoice, TourCoreRequests, TourId, TourIdsAnswer, TourPinAnswer, TourSettingsAnswer, TourUnreadable, TourView } from "@vnoc/shared/tours.ts";

// What the user does to a tour from a menu or a button: **Pin**, **Commit and Pin…**, **Write IDs for Its Stops**, **Tour Settings…** (docs/26-tours/05-on-screen.md §3, §5 "Decided for
// TR22a"). The core does each (`vnoc/tourPin`, `vnoc/tourIds`, `vnoc/tourSettings`); the host asks, says what happened in a message, and saves the files the IDs go into. No `vscode`
// here: what it needs of the window is in `ActionDeps`, so the words and the order of the steps are tested with `node --test`.

export type Level = "info" | "warn" | "error";

/** What the actions need from the window. */
export interface ActionDeps {
  request<M extends keyof TourCoreRequests>(method: M, params: TourCoreRequests[M]["params"]): Promise<TourCoreRequests[M]["result"]>;
  /** the tour as the host holds it, or as the core answers it */
  view(id: TourId): Promise<TourView | TourUnreadable>;
  /** the files with unsaved changes, relative to the opened folder */
  dirtyFiles(): string[];
  /** saves these files (relative to the opened folder) */
  save(paths: string[]): Promise<void>;
  /** a message with buttons: the one pressed, or undefined when it was dismissed */
  say(level: Level, text: string, buttons: string[]): Promise<string | undefined>;
  /** a QuickPick: the item picked, or undefined */
  pick<T extends { label: string }>(items: T[], options: { title: string; placeHolder: string }): Promise<T | undefined>;
  /** has the canvas page show **Commit and Pin…** for the tour */
  openDialog(id: TourId, title: string | null): void;
  /** the source control view */
  showChanges(): void;
  /** an input box with `value` in it: the text typed, or undefined when it was dismissed */
  input(options: { title: string; prompt: string; value: string }): Promise<string | undefined>;
  /** the tour is the window's open tour: its context closes (a tour that went to the trash cannot stay open) */
  closeIfOpen(id: TourId): void;
  /** tells the pages these tours went to the trash: they show one row in their place with `words` and **Undo** (`tourDeleted`) */
  deleted(ids: TourId[], tokens: string[], words: string): void;
  /** the list of tours is asked for again, so what an act changed is on the pages at once */
  refreshList(): void;
}

export const BUTTONS = { commitAndPin: "Commit and Pin…", saveAndWrite: "Save and Write IDs", cancel: "Cancel", showChanges: "Show Changes", pin: "Pin", always: "Always Write IDs in This Project" } as const;

const files = (n: number): string => (n === 1 ? "1 file" : `${n} files`);

/** *"Save 3 files first? The IDs are written into them."* */
export const saveFirstMessage = (n: number): string => `Save ${files(n)} first? The IDs are written into ${n === 1 ? "it" : "them"}.`;

/** The files of the tour's stops that have unsaved changes: the IDs are written into the files on disk, so a dirty one is saved first (paths relative to the opened folder, as `dirtyFiles` has them). */
export function idsToSave(dirty: readonly string[], view: Pick<TourView, "stops">): string[] {
  const wanted = new Set(view.stops.filter((s) => !s.planned).flatMap((s) => [s.path, s.now?.path].filter((p): p is string => typeof p === "string")));
  return dirty.filter((path) => wanted.has(path));
}

/** The message after a pin that did not happen, in the core's words: what stands in the way. Only files that are not committed in the way: **Commit and Pin…** is offered. */
export function pinRefusal(answer: TourPinAnswer): { text: string; offerCommit: boolean } {
  const reasons = answer.reasons ?? [];
  const only = reasons.length === 1 && reasons[0].code === "uncommitted";
  return { text: `Not pinned: ${reasons.map((r) => r.words).join(" · ") || "the core gave no reason"}.`, offerCommit: only || (answer.state === "ask" && reasons.length <= 1) };
}

/** What the two QuickPicks of **Tour Settings…** are made of: the two settings, each with its value now; then the three values, the current one marked. */
export const SETTING_KEYS = [
  { key: "ids", label: "IDs for the stops", detail: "Write an ID line into the code of each stop when a tour is finished, so the tour follows a rename" },
  { key: "commit", label: "Commit before pinning", detail: "Commit the tour's files, and only those, when a tour is finished, so it can be pinned" },
] as const;
export type SettingKey = (typeof SETTING_KEYS)[number]["key"];

export const settingItems = (shown: Pick<TourSettingsAnswer, "ids" | "commit">) => SETTING_KEYS.map((s) => ({ ...s, description: `now: ${shown[s.key].value}${shown[s.key].from === "default" ? " (the default)" : ""}` }));

const VALUES: { value: SettingChoice; detail: string }[] = [
  { value: "always", detail: "do it without asking" },
  { value: "ask", detail: "ask each time" },
  { value: "never", detail: "never offer it" },
];
export const valueItems = (current: SettingChoice) => VALUES.map((v) => ({ ...v, label: v.value, description: v.value === current ? "current" : "", picked: v.value === current }));

export class TourActions {
  private readonly d: ActionDeps;

  constructor(deps: ActionDeps) {
    this.d = deps;
  }

  /** A refusal of the core is shown in its words, in one message. */
  private async refused(e: unknown): Promise<void> {
    await this.d.say("error", String((e as Error).message ?? e), []);
  }

  private async titleOf(id: TourId): Promise<string | null> {
    const view = await this.d.view(id).catch(() => null);
    return view !== null && "stops" in view ? view.title : null;
  }

  /** **Pin**: the core pins the tour where it stands. When files that are not committed are the one thing in the way, the message offers **Commit and Pin…**. */
  async pin(id: TourId): Promise<"pinned" | "not_pinned" | "refused"> {
    let answer: TourPinAnswer;
    try {
      answer = await this.d.request("vnoc/tourPin", { id });
    } catch (e) {
      await this.refused(e);
      return "refused";
    }
    if (answer.state === "pinned") {
      await this.d.say("info", `${answer.already ? "Already pinned" : "Pinned"} at ${(answer.commit ?? "").slice(0, 7)}.`, []);
      return "pinned";
    }
    const said = pinRefusal(answer);
    const pick = await this.d.say("warn", said.text, said.offerCommit ? [BUTTONS.commitAndPin] : []);
    if (pick === BUTTONS.commitAndPin) await this.commitAndPin(id);
    return "not_pinned";
  }

  /** **Commit and Pin…**: the dialog is the page's (the one dialog a tour has); the host has the canvas show it. */
  async commitAndPin(id: TourId): Promise<void> {
    this.d.openDialog(id, await this.titleOf(id));
  }

  /**
   * **Write IDs for Its Stops**: a stop's file with unsaved changes is saved first (the IDs are written into the file on disk), with one question; then `vnoc/tourIds`; then one message with
   * **Show Changes** and **Always Write IDs in This Project**. The core's refusal (`[ids] default = "none"`) is shown in its words.
   */
  async writeIds(id: TourId): Promise<"written" | "cancelled" | "refused"> {
    try {
      const view = await this.d.view(id);
      if ("stops" in view) {
        const dirty = idsToSave(this.d.dirtyFiles(), view);
        if (dirty.length > 0) {
          const pick = await this.d.say("warn", saveFirstMessage(dirty.length), [BUTTONS.saveAndWrite, BUTTONS.cancel]);
          if (pick !== BUTTONS.saveAndWrite) return "cancelled";
          await this.d.save(dirty);
        }
      }
      const written: TourIdsAnswer = await this.d.request("vnoc/tourIds", { id });
      const pick = await this.d.say("info", written.words, written.changed.length > 0 ? [BUTTONS.showChanges, BUTTONS.always] : []);
      if (pick === BUTTONS.showChanges) this.d.showChanges();
      else if (pick === BUTTONS.always) await this.d.request("vnoc/tourSettings", { ids: "always" });
      return "written";
    } catch (e) {
      await this.refused(e);
      return "refused";
    }
  }

  /** **Rename…**: an input box with the title; an empty or unchanged one sends nothing. The core's refusal (a title it does not take) is shown in its words, its sentence after a rename is the message. */
  async rename(id: TourId): Promise<"renamed" | "unchanged" | "refused"> {
    try {
      const title = (await this.titleOf(id)) ?? "";
      const typed = await this.d.input({ title: "Rename the Tour", prompt: "The title of the tour", value: title });
      const wanted = typed?.trim() ?? "";
      if (wanted === "" || wanted === title) return "unchanged";
      const set = await this.d.request("vnoc/tourEdit", { id, title: wanted });
      this.d.refreshList();
      await this.d.say("info", set.words, []);
      return "renamed";
    } catch (e) {
      await this.refused(e);
      return "refused";
    }
  }

  /**
   * **Share with the Project** (`share`) and **Make Private Again**: `vnoc/tourShare`, then the core's sentence as the message. A tour that is not pinned has **Pin** on the message: teammates
   * would see its changed stops without the old code.
   */
  async share(id: TourId, share: boolean): Promise<"moved" | "refused"> {
    try {
      const moved = await this.d.request("vnoc/tourShare", { id, share });
      this.d.refreshList();
      const pick = await this.d.say("info", moved.words, share && !moved.pinned ? [BUTTONS.pin] : []);
      if (pick === BUTTONS.pin) await this.pin(id);
      return "moved";
    } catch (e) {
      await this.refused(e);
      return "refused";
    }
  }

  /** **Delete**: the tour and its state go to the trash; the pages show the core's words with **Undo**. No dialog. A tour being written is refused by the core, in words. The open tour closes. */
  async delete(id: TourId): Promise<"deleted" | "refused"> {
    try {
      const gone = await this.d.request("vnoc/tourDelete", { id });
      this.d.closeIfOpen(id);
      this.d.deleted([id], [gone.token], gone.words);
      this.d.refreshList();
      return "deleted";
    } catch (e) {
      await this.refused(e);
      return "refused";
    }
  }

  /** **Delete Finished Tours**: the core deletes the private finished ones; one **Undo** on the pages restores all of them. Nothing to delete is said in the core's words. */
  async deleteFinished(): Promise<number> {
    try {
      const done = await this.d.request("vnoc/tourDeleteFinished", undefined);
      if (done.deleted.length === 0) {
        await this.d.say("info", done.words, []);
        return 0;
      }
      for (const gone of done.deleted) this.d.closeIfOpen(gone.id);
      this.d.deleted(done.deleted.map((g) => g.id), done.deleted.map((g) => g.token), done.words);
      this.d.refreshList();
      return done.deleted.length;
    } catch (e) {
      await this.refused(e);
      return 0;
    }
  }

  /** **Undo**: each token's tour comes back (its file and its state, where they were). A token the trash no longer has is the core's refusal, shown in its words. */
  async undo(tokens: readonly string[]): Promise<boolean> {
    let ok = true;
    for (const token of tokens) {
      try {
        await this.d.request("vnoc/tourRestore", { token });
      } catch (e) {
        ok = false;
        await this.refused(e);
      }
    }
    this.d.refreshList();
    return ok;
  }

  /** **Tour Settings…**: a QuickPick of the two settings, then of `always` / `ask` / `never` with the current one marked, written through `tourSettings`. */
  async settings(): Promise<SettingKey | null> {
    try {
      const now = await this.d.request("vnoc/tourSettings", undefined);
      const setting = await this.d.pick(settingItems(now), { title: "Tour Settings", placeHolder: "Which setting?" });
      if (!setting) return null;
      const current = now[setting.key].value;
      const value = await this.d.pick(valueItems(current), { title: setting.label, placeHolder: setting.detail });
      if (!value) return null;
      const set = await this.d.request("vnoc/tourSettings", { [setting.key]: value.value });
      const trouble = [...(set.problems ?? []), ...(set.does_nothing ? [set.does_nothing] : [])];
      if (trouble.length > 0) await this.d.say("warn", trouble.join(" "), []);
      return setting.key;
    } catch (e) {
      await this.refused(e);
      return null;
    }
  }
}

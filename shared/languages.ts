import type { LanguageInfo } from "./messages.ts";

/**
 * What the core's `vnoc/languages` says about the files: the extensions of the drivers that run, with the dot (`.bend`), and those among them whose driver has call
 * trees. The menus, the watcher and "Show Calls" are made from these; a driver that failed has no files here.
 */
export function languageExtensions(languages: LanguageInfo[]): { all: string[]; callTree: string[] } {
  const dotted = (l: LanguageInfo) => l.extensions.map((e) => `.${e}`);
  const running = languages.filter((l) => l.state === "running");
  return {
    all: [...new Set(running.flatMap(dotted))].sort(),
    callTree: [...new Set(running.filter((l) => l.capabilities.callTree === true).flatMap(dotted))].sort(),
  };
}

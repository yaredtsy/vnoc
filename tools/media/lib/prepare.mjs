// Prepares the demo project: a clone of httpx at the pinned tag, with the V-NOC docs on top, committed, so the Source Control badge shows nothing.
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";

import { here, httpx, paths } from "../config.mjs";

const git = (cwd, ...args) => execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });

/** A fresh clone of httpx in the work folder (`withOverlay: false` for making the overlay itself). Returns its path. */
export function prepareDemo({ withOverlay = true } = {}) {
  const dir = join(paths.work, "httpx");
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(paths.work, { recursive: true });
  git(paths.work, "clone", "--quiet", "--depth", "1", "--branch", httpx.tag, httpx.url, dir);
  const head = git(dir, "rev-parse", "HEAD").trim();
  if (head !== httpx.commit) throw new Error(`httpx ${httpx.tag} is ${head}, not the pinned ${httpx.commit}: the tag moved`);
  // a branch, not a detached HEAD: the status bar shows its name instead of a hash
  git(dir, "switch", "--quiet", "-c", "main");
  if (withOverlay) {
    git(dir, "apply", join(here, "demo/overlay.patch"));
    cpSync(join(here, "demo/vnoc"), join(dir, ".vnoc"), { recursive: true });
    git(dir, "add", "-A");
    git(dir, "-c", "user.name=demo", "-c", "user.email=demo@example.com", "commit", "-q", "-m", "demo");
  }
  return dir;
}

export const demoExists = () => existsSync(join(here, "demo/overlay.patch"));

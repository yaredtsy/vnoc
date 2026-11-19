import assert from "node:assert/strict";
import { test } from "node:test";

import { addToPath, cliDir, cliInstalled, type Facts, inExtensionFolder, isVnocBinary, linkPath, onPath, parseWrapper, type Place, planInstall, planMove, planUninstall, removeFromPath, wrapperText } from "./cliInstall.ts";

// *Install `vnoc` Command in PATH*: the pure part, on each platform, for each state of the place the command goes and each mode (07 §3).

const extension = { publisher: "vnoc", name: "vnoc" };
const mac = (over: Partial<Facts> = {}): Facts => ({
  platform: "darwin",
  home: "/Users/ann",
  env: { PATH: "/usr/bin:/Users/ann/.local/bin", SHELL: "/bin/zsh" },
  binary: "/Users/ann/.vscode/extensions/vnoc.vnoc-0.3.0-darwin-arm64/bin/vnoc",
  production: true,
  extension,
  place: { kind: "nothing" },
  ...over,
});
const linux = (over: Partial<Facts> = {}): Facts => mac({ platform: "linux", home: "/home/ann", env: { PATH: "/usr/bin", SHELL: "/usr/bin/bash" }, binary: "/home/ann/.vscode/extensions/vnoc.vnoc-0.3.0-linux-x64/bin/vnoc", ...over });
const windows = (over: Partial<Facts> = {}): Facts => ({
  platform: "win32",
  home: "C:\\Users\\ann",
  env: { PATH: "C:\\Windows", LOCALAPPDATA: "C:\\Users\\ann\\AppData\\Local" },
  binary: "C:\\Users\\ann\\.vscode\\extensions\\vnoc.vnoc-0.3.0-win32-x64\\bin\\vnoc.exe",
  production: true,
  extension,
  place: { kind: "nothing" },
  ...over,
});

const OLD_MAC = "/Users/ann/.vscode/extensions/vnoc.vnoc-0.2.0-darwin-arm64/bin/vnoc";
const OLD_WIN = "C:\\Users\\ann\\.vscode\\extensions\\vnoc.vnoc-0.2.0-win32-x64\\bin\\vnoc.exe";
const link = (target: string, exists = true): Place => ({ kind: "link", target, exists });
const wrapper = (target: string, exists = true): Place => ({ kind: "wrapper", target, exists });

test("where the command goes: ~/.local/bin, %LOCALAPPDATA%\\vnoc\\bin, and VNOC_CLI_DIR over both", () => {
  assert.deepEqual(linkPath(mac()), { dir: "/Users/ann/.local/bin", link: "/Users/ann/.local/bin/vnoc" });
  assert.deepEqual(linkPath(linux()), { dir: "/home/ann/.local/bin", link: "/home/ann/.local/bin/vnoc" });
  assert.deepEqual(linkPath(windows()), { dir: "C:\\Users\\ann\\AppData\\Local\\vnoc\\bin", link: "C:\\Users\\ann\\AppData\\Local\\vnoc\\bin\\vnoc.cmd" });
  assert.deepEqual(linkPath(mac({ env: { VNOC_CLI_DIR: "/tmp/tools" } })), { dir: "/tmp/tools", link: "/tmp/tools/vnoc" });
  assert.deepEqual(linkPath(windows({ env: { VNOC_CLI_DIR: "D:\\tools" } })), { dir: "D:\\tools", link: "D:\\tools\\vnoc.cmd" });
  assert.ok("error" in cliDir(windows({ env: {} })));
});

test("nothing there: create, on each platform; the link points at the binary the extension runs", () => {
  for (const f of [mac(), linux(), windows()]) {
    const plan = planInstall(f);
    assert.equal(plan.step.do, "create");
    assert.equal((plan.step as { target: string }).target, f.binary);
    assert.ok(plan.message.includes(f.binary), plan.message);
  }
  assert.equal((planInstall(mac()).step as { link: string }).link, "/Users/ann/.local/bin/vnoc");
});

test("a link that already runs this binary is left, and says so", () => {
  const plan = planInstall(mac({ place: link(mac().binary) }));
  assert.equal(plan.step.do, "leave");
  assert.ok(plan.message.includes("already runs"), plan.message);
  assert.equal(planInstall(windows({ place: wrapper(windows().binary) })).step.do, "leave");
});

test("production moves another version's link, a development window never does", () => {
  const moved = planInstall(mac({ place: link(OLD_MAC) }));
  assert.deepEqual(moved.step, { do: "move", link: "/Users/ann/.local/bin/vnoc", from: OLD_MAC, target: mac().binary });
  assert.deepEqual(planMove(mac({ place: link(OLD_MAC) })), moved.step);
  assert.equal(planInstall(windows({ place: wrapper(OLD_WIN) })).step.do, "move");
  assert.equal(planMove(windows({ place: wrapper(OLD_WIN) }))?.do, "move");
  // development, and the test suite (also not production): the install refuses, the activation does nothing
  const dev = mac({ production: false, place: link(OLD_MAC) });
  const refused = planInstall(dev);
  assert.equal(refused.step.do, "refuse");
  assert.ok(refused.message.includes("development window"), refused.message);
  assert.equal(planMove(dev), undefined);
  // a link that is dangling because the old folder is gone is still another version's: it is moved
  assert.equal(planMove(mac({ place: link(OLD_MAC, false) }))?.do, "move");
});

test("a link to this binary, to nothing of V-NOC's, or no link at all is never moved on activation", () => {
  assert.equal(planMove(mac({ place: link(mac().binary) })), undefined);
  assert.equal(planMove(mac({ place: { kind: "nothing" } })), undefined);
  assert.equal(planMove(mac({ place: link("/Users/ann/work/vnoc/target/debug/vnoc") })), undefined);
  assert.equal(planMove(mac({ place: { kind: "file" } })), undefined);
});

test("a foreign link or file is refused, and the message says whose", () => {
  for (const f of [mac({ place: link("/opt/tools/vnoc-1/vnoc") }), mac({ place: link("/Users/ann/work/vnoc/target/debug/vnoc") }), linux({ place: { kind: "file" } }), windows({ place: { kind: "file" } })]) {
    const plan = planInstall(f);
    assert.equal(plan.step.do, "refuse");
    assert.ok(plan.message.includes("it is yours"), plan.message);
    assert.equal(plan.windowsPath, undefined);
  }
  // production too: a link to a place that is not another version's folder is the user's
  assert.equal(planInstall(mac({ place: link("/opt/tools/vnoc-1/vnoc") })).step.do, "refuse");
});

test("the PATH advice: present when the folder is not on it, by the shell, and absent when it is", () => {
  const off = (over: Partial<Facts>) => planInstall(mac({ env: { PATH: "/usr/bin", ...over.env }, ...over })).pathAdvice;
  assert.equal(off({ env: { SHELL: "/bin/zsh" } }), '~/.local/bin is not on your PATH: add export PATH="$HOME/.local/bin:$PATH" to ~/.zshrc, then open a new terminal.');
  assert.equal(off({ env: { SHELL: "/bin/bash" } }), '~/.local/bin is not on your PATH: add export PATH="$HOME/.local/bin:$PATH" to ~/.bash_profile, then open a new terminal.');
  assert.ok(planInstall(linux()).pathAdvice?.includes("to ~/.bashrc"));
  assert.ok(off({ env: { SHELL: "/usr/bin/fish" } })?.includes("fish_add_path $HOME/.local/bin"));
  assert.ok(off({ env: { SHELL: "/bin/tcsh" } })?.includes("your shell's startup file"));
  assert.ok(off({ env: {} })?.includes("your shell's startup file"));
  // on the PATH, with or without a trailing slash: no advice; and it is in the message of a refusal? no: a refusal has none
  assert.equal(planInstall(mac()).pathAdvice, undefined);
  assert.equal(planInstall(mac({ env: { PATH: "/usr/bin:/Users/ann/.local/bin/" } })).pathAdvice, undefined);
  assert.equal(planInstall(mac({ env: { PATH: "/usr/bin" }, place: { kind: "file" } })).pathAdvice, undefined);
  // another folder is named as it is, and `~` only under the home folder
  const elsewhere = planInstall(mac({ env: { PATH: "/usr/bin", VNOC_CLI_DIR: "/opt/tools/bin", SHELL: "/bin/zsh" } })).pathAdvice;
  assert.equal(elsewhere, '/opt/tools/bin is not on your PATH: add export PATH="/opt/tools/bin:$PATH" to ~/.zshrc, then open a new terminal.');
  // Windows edits the PATH itself and gives no advice
  assert.equal(planInstall(windows()).pathAdvice, undefined);
  assert.ok(planInstall(mac({ env: { PATH: "/usr/bin" } })).message.includes("is not on your PATH"));
});

test("Windows: the user PATH entry is added unless it is there (case does not matter), and uninstall takes out only what install added", () => {
  const dir = "C:\\Users\\ann\\AppData\\Local\\vnoc\\bin";
  assert.equal(planInstall(windows()).windowsPath, "add");
  assert.ok(planInstall(windows()).message.includes("Added"));
  assert.equal(planInstall(windows({ userPath: `C:\\Tools;${dir}` })).windowsPath, "present");
  assert.equal(planInstall(windows({ userPath: `C:\\Tools;${dir.toUpperCase()}\\` })).windowsPath, "present");
  assert.ok(!planInstall(windows({ userPath: dir })).message.includes("Added"));
  const here = windows({ place: wrapper(windows().binary), userPath: `C:\\Tools;${dir}` });
  assert.equal(planUninstall({ ...here, addedToUserPath: true }).windowsPath, "remove");
  assert.equal(planUninstall({ ...here, addedToUserPath: false }).windowsPath, "keep");
  assert.equal(planUninstall({ ...here, addedToUserPath: true, userPath: "C:\\Tools" }).windowsPath, "keep");
  assert.equal(addToPath("C:\\Tools;", dir), `C:\\Tools;${dir}`);
  assert.equal(addToPath(undefined, dir), dir);
  assert.equal(removeFromPath(`C:\\Tools;${dir};%USERPROFILE%\\bin`, dir), "C:\\Tools;%USERPROFILE%\\bin");
  assert.equal(removeFromPath(dir, dir), "");
  assert.ok(onPath(dir, `X;${dir}`, "win32") && !onPath(dir, "X;Y", "win32"));
});

test("development mode adds the VNOC_DRIVER note, production does not", () => {
  const dev = planInstall(mac({ production: false, driver: "/repo/drivers/python/target/debug/vnoc-driver-python" }));
  assert.equal(dev.step.do, "create");
  assert.ok(dev.driverNote?.includes("VNOC_DRIVER=/repo/drivers/python/target/debug/vnoc-driver-python"), dev.driverNote);
  assert.ok(dev.message.includes("VNOC_DRIVER"));
  assert.equal(planInstall(mac()).driverNote, undefined);
  assert.ok(planInstall(mac({ production: false })).driverNote?.includes("VNOC_DRIVER set to the driver"));
});

test("uninstall removes a link to a V-NOC program or our wrapper, and nothing else", () => {
  assert.equal(planUninstall(mac({ place: link(mac().binary) })).step.do, "remove");
  assert.equal(planUninstall(mac({ place: link("/Users/ann/work/vnoc/target/debug/vnoc") })).step.do, "remove");
  assert.equal(planUninstall(windows({ place: wrapper(windows().binary) })).step.do, "remove");
  assert.equal(planUninstall(mac()).step.do, "leave");
  assert.ok(planUninstall(mac()).message.startsWith("Nothing to remove"));
  for (const f of [mac({ place: link("/usr/bin/python3") }), mac({ place: { kind: "file" } }), windows({ place: { kind: "file" } })]) {
    const plan = planUninstall(f);
    assert.equal(plan.step.do, "refuse");
    assert.ok(plan.message.includes("it is yours"), plan.message);
  }
});

test("the context key: a command that exists and points to a V-NOC program that is there", () => {
  assert.equal(cliInstalled(mac({ place: link(mac().binary) })), true);
  assert.equal(cliInstalled(mac({ place: link(mac().binary, false) })), false);
  assert.equal(cliInstalled(mac({ place: link("/usr/bin/python3") })), false);
  assert.equal(cliInstalled(mac({ place: { kind: "file" } })), false);
  assert.equal(cliInstalled(mac()), false);
  assert.equal(cliInstalled(windows({ place: wrapper(windows().binary) })), true);
});

test("another version's folder is found by the extension's own name, not a fixed one", () => {
  assert.ok(inExtensionFolder(OLD_MAC, extension));
  assert.ok(inExtensionFolder("/home/ann/.vscode-insiders/extensions/vnoc.vnoc-1.0.0/bin/vnoc", extension));
  assert.ok(inExtensionFolder(OLD_WIN, extension));
  assert.ok(!inExtensionFolder(OLD_MAC, { publisher: "acme", name: "vnoc" }));
  assert.ok(!inExtensionFolder("/Users/ann/.vscode/extensions/vnoc.vnoc-extras-0.2.0/lib/vnoc", extension));
  assert.ok(!inExtensionFolder("/repo/target/debug/vnoc", extension));
  assert.ok(inExtensionFolder("/x/acme.tool-2.0.0/bin/vnoc", { publisher: "acme", name: "tool" }));
  assert.ok(isVnocBinary("/a/b/vnoc") && isVnocBinary("C:\\a\\VNOC.EXE") && !isVnocBinary("/usr/bin/vnoc-driver-python"));
});

test("the wrapper is one line that runs the program and passes the arguments on, and reads back", () => {
  const text = wrapperText("C:\\Users\\ann\\ext\\bin\\vnoc.exe");
  assert.equal(text, '@"C:\\Users\\ann\\ext\\bin\\vnoc.exe" %*\r\n');
  assert.equal(parseWrapper(text), "C:\\Users\\ann\\ext\\bin\\vnoc.exe");
  assert.equal(parseWrapper(text.replace(/\r/g, "")), "C:\\Users\\ann\\ext\\bin\\vnoc.exe");
  assert.equal(parseWrapper("@echo off\r\npython %*\r\n"), undefined);
  assert.equal(parseWrapper(""), undefined);
});

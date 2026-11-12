import * as path from "path";
import Mocha from "mocha";

export function run(): Promise<void> {
  const mocha = new Mocha({ ui: "tdd", timeout: 60_000, color: false });
  mocha.addFile(path.resolve(__dirname, "skeleton.test.js"));
  mocha.addFile(path.resolve(__dirname, "session.test.js"));
  mocha.addFile(path.resolve(__dirname, "sidebar.test.js"));
  mocha.addFile(path.resolve(__dirname, "attention.test.js"));
  mocha.addFile(path.resolve(__dirname, "refactor.test.js"));
  mocha.addFile(path.resolve(__dirname, "calls.test.js"));
  return new Promise((resolve, reject) => mocha.run((failures) => (failures ? reject(new Error(`${failures} tests failed`)) : resolve())));
}

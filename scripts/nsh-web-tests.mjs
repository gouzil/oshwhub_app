import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const nsh = process.env.NSH_BIN || "nsh";
const bindgen = process.env.NSH_WASM_BINDGEN || "wasm-bindgen";
const run = (command, args, cwd = root) => execFileSync(command, args, { cwd, stdio: "inherit" });
assert.match(execFileSync(nsh, ["--version"], { encoding: "utf8" }), /\b0\.4\.0\b/);
assert.match(execFileSync(bindgen, ["--version"], { encoding: "utf8" }), /\b0\.2\.129\b/);

const output = join(root, "output/nsh-web-tests");
mkdirSync(output, { recursive: true });
writeFileSync(
  join(output, "index.html"),
  '<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Rust browser tests</title></head><body>Rust/WASM core harness.</body></html>',
);
run("cargo", [
  "build",
  "--manifest-path",
  "tests/wasm-core/Cargo.toml",
  "--target",
  "wasm32-unknown-unknown",
  "--release",
  "--locked",
]);
const target = process.env.CARGO_TARGET_DIR
  ? resolve(root, process.env.CARGO_TARGET_DIR)
  : join(root, "tests/wasm-core/target");
run(bindgen, [
  "--target",
  "web",
  "--out-dir",
  output,
  "--out-name",
  "probe",
  join(target, "wasm32-unknown-unknown/release/oshwhub_wasm_core_probe.wasm"),
]);

for (const [directory, name, destination] of [
  ["apps/nsh", "OSHWHub", "app"],
  ["tests/nsh-surface", "SurfaceProbe", "surface"],
]) {
  run(nsh, ["package", "--target", "web"], join(root, directory));
  const source = join(root, directory, ".nsh/out", name);
  assert.ok(existsSync(join(source, "index.html")), `missing ${name} Web package`);
  rmSync(join(output, destination), { recursive: true, force: true });
  cpSync(source, join(output, destination), { recursive: true });
}
run("pnpm", ["exec", "playwright", "test", "-c", "tests/nsh/playwright.config.ts"]);

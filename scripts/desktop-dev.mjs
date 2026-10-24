import { spawn } from "node:child_process";
import path from "node:path";
import { setTimeout } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { buildCore } from "./desktop-build.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const url = "http://127.0.0.1:5173/";
const children = new Set();
let stopping = false;

function stop(code = 0) {
  if (stopping) return;
  stopping = true;
  for (const child of children) {
    if (process.platform === "win32")
      spawn("taskkill", ["/pid", String(child.pid), "/T", "/F"], { stdio: "ignore" });
    else {
      try {
        process.kill(-child.pid, "SIGTERM");
      } catch {}
    }
  }
  process.exitCode = code;
}
function start(args, env = process.env) {
  const child = spawn("pnpm", ["--filter", "@oshwhub/desktop", ...args], {
    cwd: root,
    stdio: "inherit",
    env,
    detached: process.platform !== "win32",
    shell: process.platform === "win32",
  });
  children.add(child);
  child.once("error", () => stop(1));
  child.once("exit", (code) => {
    children.delete(child);
    stop(code ?? 1);
  });
  return child;
}

process.once("SIGINT", () => stop());
process.once("SIGTERM", () => stop());
try {
  await buildCore(false);
  if (!stopping) {
    start(["dev"]);
    let ready = false;
    for (let attempt = 0; attempt < 120 && !stopping; attempt++) {
      try {
        ready = (await fetch(url, { signal: AbortSignal.timeout(500) })).ok;
      } catch {}
      if (ready) break;
      await setTimeout(250);
    }
    if (!ready) throw new Error("Desktop renderer did not start");
    if (!stopping) start(["exec", "electron", "."], { ...process.env, OSHWHUB_RENDERER_URL: url });
  }
} catch (error) {
  console.error(error.message);
  stop(1);
}

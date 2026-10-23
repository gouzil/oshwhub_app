import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdir, mkdtemp, realpath, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { PassThrough, Writable } from "node:stream";
import test from "node:test";
import { assetPath } from "../apps/desktop/electron/assets.mjs";
import {
  assertTrustedSender,
  externalUrl,
  validateInput,
} from "../apps/desktop/electron/security.mjs";
import { Sidecar } from "../apps/desktop/electron/sidecar.mjs";
import { assertHostBuild, hostTarget } from "./desktop-build.mjs";

function fakeChild() {
  const child = new EventEmitter();
  child.stdout = new PassThrough();
  child.stderr = new PassThrough();
  child.writes = [];
  child.stdin = new Writable({
    write(chunk, _encoding, callback) {
      child.writes.push(JSON.parse(chunk));
      callback();
    },
  });
  child.kill = () => {
    child.killed = true;
  };
  return child;
}
const output = '{"ok":false,"error":{"code":"scene_expired","requestId":"business-id"}}';

test("IPC rejects other windows, subframes and unexpected locations", () => {
  const frame = { url: "oshwhub://app/" };
  const sender = { mainFrame: frame };
  const window = { webContents: sender, isDestroyed: () => false };
  assert.doesNotThrow(() =>
    assertTrustedSender({ sender, senderFrame: frame }, window, "oshwhub://app/"),
  );
  for (const event of [
    { sender: {}, senderFrame: frame },
    { sender, senderFrame: { ...frame } },
    { sender },
  ]) {
    assert.throws(() => assertTrustedSender(event, window, "oshwhub://app/"));
  }
  frame.url = "https://evil.test/";
  assert.throws(() =>
    assertTrustedSender({ sender, senderFrame: frame }, window, "oshwhub://app/"),
  );
});

test("IPC inputs are byte bounded and external URLs only allow HTTP(S)", () => {
  assert.equal(validateInput("{}"), "{}");
  for (const input of [null, {}, "é".repeat(524289)]) assert.throws(() => validateInput(input));
  assert.equal(externalUrl("https://example.com/a"), "https://example.com/a");
  for (const url of [
    "file:///etc/passwd",
    "javascript:alert(1)",
    "oshwhub://app/",
    "data:text/html,hi",
    "https://user:pass@example.com",
    "\nhttps://example.com",
    {},
    "https://",
  ])
    assert.throws(() => externalUrl(url));
});

test("sidecar correlates concurrent responses and preserves business output", async () => {
  const child = fakeChild();
  const sidecar = new Sidecar(child);
  const one = sidecar.dispatch('{"command":"health"}');
  const two = sidecar.dispatch('{"command":"demo"}');
  const [a, b] = child.writes;
  child.stdout.write(`${JSON.stringify({ id: b.id, output })}\n`);
  child.stdout.write(`${JSON.stringify({ id: a.id, output: '{"ok":true}' })}\n`);
  assert.equal(await two, output);
  assert.equal(await one, '{"ok":true}');
  sidecar.close();
  assert.equal(child.killed, true);
});

test("sidecar settles pending calls on exit, malformed replies, write errors and close", async () => {
  for (const failure of [
    "exit",
    "error",
    "invalid",
    "shape",
    "id",
    "oversized",
    "write",
    "close",
  ]) {
    const child = fakeChild();
    const sidecar = new Sidecar(child, { maxResponseBytes: 1024 });
    const pending = sidecar.dispatch("{}");
    const rejected = assert.rejects(pending, /core|sidecar/i);
    if (failure === "exit") child.emit("exit", 1, null);
    if (failure === "error") child.emit("error", new Error("spawn failure"));
    if (failure === "invalid") child.stdout.write("not json\n");
    if (failure === "shape") child.stdout.write('{"id":"1","output":{}}\n');
    if (failure === "id") child.stdout.write('{"id":"unknown","output":"{}"}\n');
    if (failure === "oversized") child.stdout.write("x".repeat(1025));
    if (failure === "write") child.stdin.emit("error", new Error("broken pipe"));
    if (failure === "close") sidecar.close();
    await rejected;
    await assert.rejects(sidecar.dispatch("{}"), /core|sidecar/i);
    sidecar.close();
  }
});

test("sidecar timeout closes the process and pending calls; request queue is bounded", async () => {
  const child = fakeChild();
  const sidecar = new Sidecar(child, { timeoutMs: 10, maxPending: 1 });
  const timedOut = assert.rejects(sidecar.dispatch("{}"), /timed out/i);
  await assert.rejects(sidecar.dispatch("{}"), /busy/i);
  await timedOut;
  assert.equal(child.killed, true);
});

test("packaged assets reject traversal, foreign origins and symlinks", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "oshwhub-assets-"));
  try {
    const dist = path.join(root, "dist");
    await mkdir(dist);
    await writeFile(path.join(dist, "index.html"), "app");
    await writeFile(path.join(root, "secret"), "secret");
    await symlink(path.join(root, "secret"), path.join(dist, "link"));
    assert.equal(
      await assetPath("oshwhub://app/", dist),
      await realpath(path.join(dist, "index.html")),
    );
    for (const url of [
      "oshwhub://evil/index.html",
      "file:///etc/passwd",
      "oshwhub://app/%2e%2e/secret",
      "oshwhub://app/%2f..%2fsecret",
      "oshwhub://app/link",
      "oshwhub://app/%5c..%5csecret",
      "oshwhub://app/%00",
    ]) {
      await assert.rejects(assetPath(url, dist));
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("packaging rejects foreign architecture or platform", () => {
  assert.match(hostTarget(), /^(aarch64|x86_64)-(apple-darwin|pc-windows-msvc|unknown-linux-gnu)$/);
  assert.doesNotThrow(() => assertHostBuild([]));
  assert.throws(() => assertHostBuild([process.arch === "arm64" ? "--x64" : "--arm64"]));
  assert.throws(() => assertHostBuild([process.platform === "win32" ? "--mac" : "--win"]));
});

test("sandbox preload exposes only two named promise-based methods", async () => {
  const { runInNewContext } = await import("node:vm");
  const { readFile } = await import("node:fs/promises");
  let api;
  const invokes = [];
  runInNewContext(
    await readFile(new URL("../apps/desktop/electron/preload.cjs", import.meta.url), "utf8"),
    {
      require: (name) => {
        assert.equal(name, "electron");
        return {
          contextBridge: {
            exposeInMainWorld: (name, exposed) => {
              assert.equal(name, "oshwhubDesktop");
              api = exposed;
            },
          },
          ipcRenderer: {
            invoke: (...args) => {
              invokes.push(args);
              return Promise.resolve("result");
            },
          },
        };
      },
    },
  );
  assert.deepEqual(Object.keys(api), ["dispatch", "openExternal"]);
  assert.equal(Object.isFrozen(api), true);
  assert.equal(await api.dispatch("{}"), "result");
  await api.openExternal("https://example.com/");
  assert.deepEqual(invokes, [
    ["oshwhub:dispatch", "{}"],
    ["oshwhub:openExternal", "https://example.com/"],
  ]);
});

test("sidecar decodes fragmented UTF-8 frames and multiple responses in one chunk", async () => {
  const child = fakeChild();
  const sidecar = new Sidecar(child);
  const first = sidecar.dispatch("{}");
  const second = sidecar.dispatch("{}");
  const [a, b] = child.writes;
  const bytes = Buffer.from(
    `${JSON.stringify({ id: a.id, output: "中文" })}\n${JSON.stringify({ id: b.id, output: "done" })}\n`,
  );
  const split = bytes.indexOf(Buffer.from("中")) + 1;
  child.stdout.write(bytes.subarray(0, split));
  child.stdout.write(bytes.subarray(split));
  assert.equal(await first, "中文");
  assert.equal(await second, "done");
  sidecar.close();
});

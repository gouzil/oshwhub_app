import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { processSnapshot, processSummary } from "./nsh-process-sample.mjs";

const rows = `
 10 1 0:02.50 100 Sun Nov  8 00:00:00 2026 /App/OSHWHub
 11 10 01:03.25 200 Sun Nov  8 00:00:01 2026 /App/Helper
 12 11 1-02:03:04.50 300 Sun Nov  8 00:00:02 2026 /App/GPU Helper
 13 1 0:09.00 900 Sun Nov  8 00:00:03 2026 /Other/App
`;

test("sample includes the whole application tree and converts KiB and CPU time", () => {
  assert.deepEqual(processSnapshot(rows, 10), [
    {
      pid: 10,
      ppid: 1,
      cpuSeconds: 2.5,
      rssBytes: 102400,
      started: "Sun Nov 8 00:00:00 2026",
      command: "/App/OSHWHub",
    },
    {
      pid: 11,
      ppid: 10,
      cpuSeconds: 63.25,
      rssBytes: 204800,
      started: "Sun Nov 8 00:00:01 2026",
      command: "/App/Helper",
    },
    {
      pid: 12,
      ppid: 11,
      cpuSeconds: 93784.5,
      rssBytes: 307200,
      started: "Sun Nov 8 00:00:02 2026",
      command: "/App/GPU Helper",
    },
  ]);
});

test("missing roots and malformed process data cannot become zero samples", () => {
  assert.throws(() => processSnapshot(rows, 99), /not found/);
  assert.throws(() => processSnapshot("10 1 broken", 10), /process row/);
  assert.throws(() => processSnapshot(rows.replace("01:03.25", "01:93.25"), 10), /CPU time/);
});

const samples = [
  { elapsedMs: 100, processes: [{ pid: 10, started: "start", cpuSeconds: 5, rssBytes: 100 }] },
  { elapsedMs: 1100, processes: [{ pid: 10, started: "start", cpuSeconds: 5.5, rssBytes: 200 }] },
  { elapsedMs: 2100, processes: [{ pid: 10, started: "start", cpuSeconds: 6, rssBytes: 150 }] },
];

test("one CPU second across two wall seconds is 50 percent with peak RSS", () => {
  assert.deepEqual(processSummary(samples), {
    durationSec: 2,
    cpuSeconds: 1,
    cpuPct: 50,
    peakRssBytes: 200,
  });
});

test("process churn and PID reuse invalidate CPU totals", () => {
  for (const processes of [[], [{ ...samples[2].processes[0], started: "reused" }]]) {
    assert.throws(
      () => processSummary([samples[0], { ...samples[2], processes }]),
      /process tree changed/,
    );
  }
});

test("backwards CPU counters and invalid sampling boundaries are rejected", () => {
  assert.throws(() => processSummary([samples[1], samples[0]]), /elapsed/);
  assert.throws(
    () =>
      processSummary([
        samples[0],
        { ...samples[1], processes: [{ ...samples[1].processes[0], cpuSeconds: 4 }] },
      ]),
    /CPU counter/,
  );
  assert.throws(() => processSummary(samples.slice(0, 1)), /two samples/);
});

test("CPU and peak RSS include renderer and GPU helper processes", () => {
  const child = { pid: 11, started: "child", cpuSeconds: 3, rssBytes: 50 };
  assert.deepEqual(
    processSummary([
      { ...samples[0], processes: [...samples[0].processes, child] },
      {
        ...samples[2],
        processes: [...samples[2].processes, { ...child, cpuSeconds: 4, rssBytes: 100 }],
      },
    ]),
    { durationSec: 2, cpuSeconds: 2, cpuPct: 100, peakRssBytes: 250 },
  );
});

test(
  "macOS CLI captures a live process and preserves existing raw evidence",
  { skip: process.platform !== "darwin" },
  async () => {
    const directory = await mkdtemp(join(tmpdir(), "oshwhub-process-sample-"));
    const path = join(directory, "sample.json");
    const worker = spawn(process.execPath, ["-e", "while (true) {}"], { stdio: "ignore" });
    try {
      await new Promise((resolve, reject) => {
        worker.once("spawn", resolve);
        worker.once("error", reject);
      });
      const args = ["scripts/nsh-process-sample.mjs", String(worker.pid), "0.6", path];
      await promisify(execFile)(process.execPath, args);
      const raw = await readFile(path, "utf8");
      const capture = JSON.parse(raw);
      assert.equal(capture.kind, "process-diagnostic");
      assert.ok(capture.samples.length >= 2);
      assert.equal(capture.samples[0].processes[0].pid, worker.pid);
      assert.ok(capture.summary.cpuPct > 0 && capture.summary.cpuPct < 300);
      assert.ok(capture.summary.peakRssBytes > 0);
      await assert.rejects(promisify(execFile)(process.execPath, args), /EEXIST/);
      assert.equal(await readFile(path, "utf8"), raw);
    } finally {
      worker.kill();
      await rm(directory, { recursive: true, force: true });
    }
  },
);

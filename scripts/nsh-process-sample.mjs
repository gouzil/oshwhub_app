import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { open } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout } from "node:timers/promises";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

export function processSnapshot(output, rootPid) {
  const rows = output
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const row = line.match(
        /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(\d+)\s+(\w+)\s+(\w+)\s+(\d+)\s+(\d\d:\d\d:\d\d)\s+(\d{4})\s+(.+)$/,
      );
      assert.ok(row, `Invalid process row: ${line}`);
      const [, pid, ppid, time, rss, day, month, date, clock, year, command] = row;
      const parts = time.match(/^(?:(\d+)-)?(?:(\d+):)?(\d+):(\d+(?:\.\d+)?)$/);
      assert.ok(
        parts && Number(parts[4]) < 60 && (!parts[2] || Number(parts[3]) < 60),
        `Invalid CPU time: ${time}`,
      );
      const cpuSeconds =
        Number(parts[1] || 0) * 86400 +
        Number(parts[2] || 0) * 3600 +
        Number(parts[3]) * 60 +
        Number(parts[4]);
      const rssBytes = Number(rss) * 1024;
      assert.ok(
        Number.isFinite(cpuSeconds) && Number.isSafeInteger(rssBytes),
        "Invalid process counters",
      );
      return {
        pid: Number(pid),
        ppid: Number(ppid),
        cpuSeconds,
        rssBytes,
        started: `${day} ${month} ${date} ${clock} ${year}`,
        command,
      };
    });
  assert.ok(
    rows.some((row) => row.pid === rootPid),
    `Root PID ${rootPid} not found`,
  );
  const selected = new Set([rootPid]);
  for (let previous = -1; previous !== selected.size; ) {
    previous = selected.size;
    for (const row of rows) if (selected.has(row.ppid)) selected.add(row.pid);
  }
  return rows.filter((row) => selected.has(row.pid)).sort((a, b) => a.pid - b.pid);
}

export function processSummary(samples) {
  assert.ok(samples.length >= 2, "Need at least two samples");
  const identity = (sample) =>
    JSON.stringify(sample.processes.map(({ pid, started }) => [pid, started]));
  const initial = identity(samples[0]);
  let cpuSeconds = 0;
  let peakRssBytes = 0;
  for (let i = 0; i < samples.length; i++) {
    const sample = samples[i];
    assert.equal(identity(sample), initial, "Application process tree changed during sampling");
    peakRssBytes = Math.max(
      peakRssBytes,
      sample.processes.reduce((sum, row) => sum + row.rssBytes, 0),
    );
    if (i === 0) continue;
    assert.ok(sample.elapsedMs > samples[i - 1].elapsedMs, "Invalid elapsed sampling time");
    for (let p = 0; p < sample.processes.length; p++) {
      const delta = sample.processes[p].cpuSeconds - samples[i - 1].processes[p].cpuSeconds;
      assert.ok(delta >= 0 && Number.isFinite(delta), "CPU counter moved backwards or is invalid");
      cpuSeconds += delta;
    }
  }
  const durationSec = (samples.at(-1).elapsedMs - samples[0].elapsedMs) / 1000;
  return { durationSec, cpuSeconds, cpuPct: (cpuSeconds / durationSec) * 100, peakRssBytes };
}

async function main() {
  assert.equal(process.platform, "darwin", "This sampler requires macOS ps");
  assert.equal(
    process.argv.length,
    5,
    "Usage: node scripts/nsh-process-sample.mjs <root-pid> <seconds> <output.json>",
  );
  const rootPid = Number(process.argv[2]);
  const durationMs = Number(process.argv[3]) * 1000;
  assert.ok(
    Number.isSafeInteger(rootPid) && rootPid > 0 && rootPid !== process.pid,
    "Invalid target PID",
  );
  assert.ok(
    Number.isFinite(durationMs) && durationMs >= 100 && durationMs <= 3600000,
    "Duration must be 0.1 to 3600 seconds",
  );
  const file = await open(process.argv[4], "wx");
  const capture = {
    kind: "process-diagnostic",
    rootPid,
    startedAt: new Date().toISOString(),
    method:
      "macOS ps cputime (centisecond resolution), summed process-tree RSS in bytes, roughly 1 Hz",
    samples: [],
  };
  try {
    const start = performance.now();
    do {
      if (capture.samples.length)
        await setTimeout(
          Math.max(
            0,
            start + Math.min(durationMs, capture.samples.length * 1000) - performance.now(),
          ),
        );
      const before = performance.now();
      const { stdout } = await promisify(execFile)(
        "/bin/ps",
        ["-axo", "pid=,ppid=,time=,rss=,lstart=,comm="],
        {
          encoding: "utf8",
          env: { ...process.env, LC_ALL: "C" },
          maxBuffer: 16 * 1024 * 1024,
          timeout: 10000,
        },
      );
      const after = performance.now();
      capture.samples.push({
        elapsedMs: (before + after) / 2 - start,
        pollDurationMs: after - before,
        processes: processSnapshot(stdout, rootPid),
      });
    } while (capture.samples.at(-1).elapsedMs < durationMs);
    capture.summary = processSummary(capture.samples);
  } catch (error) {
    capture.error = error.message;
    process.exitCode = 1;
  } finally {
    await file.writeFile(`${JSON.stringify(capture, null, 2)}\n`);
    await file.close();
  }
  console.log(JSON.stringify(capture.summary || { error: capture.error }));
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}

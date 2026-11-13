import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import { compare, compareDatasets, validateDataset } from "./nsh-performance.mjs";

const scenarios = () =>
  Object.fromEntries(
    ["pages", "pcb2d", "schematic", "pcb3d"].map((name) => [
      name,
      {
        sample: { label: "fixed example fixture", kind: "example" },
        startup: { runs: Array.from({ length: 10 }, () => ({ durationMs: 100 })) },
        interaction: {
          groups: Array.from({ length: 5 }, () => ({
            durationSec: 60,
            frameMs: Array(3750).fill(16),
            droppedFrames: 0,
            cpuPct: 20,
            rssBytes: 1000,
            gpuBytes: 2000,
            actions: { layerMs: [10], pickMs: [10], bomMs: [10] },
            gesture: { compile: 0, pathParse: 0, meshUpload: 0 },
          })),
        },
        lifecycle: {
          cycles: Array.from({ length: 20 }, () => ({
            rssBeforeBytes: 1000,
            rssAfterBytes: 1000,
            gpuBeforeBytes: 2000,
            gpuAfterBytes: 2000,
          })),
        },
      },
    ]),
  );

const dataset = (role = "baseline") => ({
  schemaVersion: 1,
  role,
  exactCommit: role === "baseline" ? "old-commit-0123456789abcdef" : "new-commit-fedcba9876543210",
  metadata: {
    device: "MacBookPro18,3",
    os: "macOS 15.7",
    gpu: { name: "Apple M3 Max", kind: "hardware" },
    backend: "metal",
    refreshHz: 60,
    dpr: 2,
    viewport: { width: 1440, height: 900 },
    inputSha256: "a".repeat(64),
    cache: "cold",
    release: true,
    metricDefinitions: {
      startupMs: "process start to first interactive frame",
      frameMs: "present-to-present frame duration in milliseconds",
      dropRatio: "dropped frames divided by submitted frames",
      startupMedianMs: "median startup duration across independent runs",
      startupP95Ms: "p95 startup duration across independent runs",
      frameP50Ms: "p50 frame duration per interaction group",
      frameP95Ms: "p95 frame duration per interaction group",
      frameP99Ms: "p99 frame duration per interaction group",
      dropP50Ratio: "p50 dropped-frame ratio across independent interaction groups",
      dropP95Ratio: "p95 dropped-frame ratio across independent interaction groups",
      dropP99Ratio: "p99 dropped-frame ratio across independent interaction groups",
      cpuPct: "p95 peak process CPU percentage across interaction groups",
      rssBytes: "p95 peak process resident memory bytes across interaction groups",
      gpuBytes: "p95 peak GPU allocation bytes across interaction groups",
      layerP95Ms: "p95 layer switch duration",
      pickP95Ms: "p95 pick duration",
      bomP95Ms: "p95 BOM filter duration",
      rssGrowthBytes: "resident memory change per lifecycle cycle",
      gpuGrowthBytes: "GPU allocation change per lifecycle cycle",
      rssTrendBytesPerCycle: "linear resident memory trend over lifecycle cycles",
      gpuTrendBytesPerCycle: "linear GPU allocation trend over lifecycle cycles",
      gestureCompileCount: "gesture-time工程compile count",
      pathParseCount: "gesture-time path parse count",
      meshUploadCount: "gesture-time mesh upload count",
    },
    sampling: {
      method: "independent-run-bootstrap",
      startupRuns: 10,
      interactionGroups: 5,
      interactionDurationSec: 60,
      lifecycleCycles: 20,
      bootstrapReplicates: 2000,
      confidence: 0.95,
    },
  },
  scenarios: scenarios(),
});

describe("nsh performance comparison", () => {
  it("starts red for an invalid or incomplete dataset", () => {
    assert.throws(() => validateDataset({}), /schemaVersion|role|metadata|scenarios/);
  });

  it("accepts a complete equal fixture and reports deterministic equality", () => {
    const result = compareDatasets(dataset(), dataset("candidate"));
    assert.equal(compare(dataset(), dataset("candidate")).status, "pass");
    assert.equal(result.status, "pass");
    assert.match(result.report, /equal|相同/i);
    assert.deepEqual(compareDatasets(dataset(), dataset("candidate")).items, result.items);
    const legacyName = dataset();
    legacyName.exactcommit = legacyName.exactCommit;
    delete legacyName.exactCommit;
    assert.equal(validateDataset(legacyName).role, "baseline");
  });

  it("rejects mismatched metadata, fake software GPU, and insufficient samples", () => {
    const other = dataset("candidate");
    other.metadata.viewport.width += 1;
    assert.throws(() => compareDatasets(dataset(), other), /metadata|viewport/);

    const software = dataset();
    software.metadata.gpu = { name: "SwiftShader", kind: "hardware" };
    assert.throws(() => validateDataset(software), /software GPU|SwiftShader/i);
    const cpuBackend = dataset();
    cpuBackend.metadata.backend = "cpu";
    assert.throws(() => validateDataset(cpuBackend), /software GPU|cpu/i);

    const short = dataset();
    short.scenarios.pages.startup.runs = short.scenarios.pages.startup.runs.slice(0, 9);
    assert.throws(() => validateDataset(short), /startup.*10|insufficient/i);

    const nan = dataset();
    nan.scenarios.pcb2d.interaction.groups[0].rssBytes = Number.NaN;
    assert.throws(() => validateDataset(nan), /finite|NaN/i);
  });

  it("rejects truncated or overlong frame observations despite a nominal 60-second duration", () => {
    for (const frames of [[1, 1], Array(7500).fill(16)]) {
      const capture = dataset();
      capture.scenarios.pages.interaction.groups[0].frameMs = frames;
      assert.throws(() => validateDataset(capture), /frame.*coverage|observation/i);
    }
  });

  it("requires the specified 95% confidence for acceptance", () => {
    const capture = dataset();
    capture.metadata.sampling.confidence = 0.51;
    assert.throws(() => validateDataset(capture), /confidence.*0.95/);
  });

  it("fails a clear regression and marks noisy overlap inconclusive", () => {
    const slower = dataset("candidate");
    for (const run of slower.scenarios.pages.startup.runs) run.durationMs = 110;
    const failed = compareDatasets(dataset(), slower);
    assert.equal(failed.status, "fail");
    assert.match(failed.report, /pages.*startup|启动/i);

    const actionSlower = dataset("candidate");
    actionSlower.scenarios.pages.interaction.groups.forEach((group) => {
      group.actions.layerMs = [20];
    });
    const actionFailure = compareDatasets(dataset(), actionSlower);
    assert.equal(actionFailure.status, "fail");
    assert.ok(
      actionFailure.items.some((item) => item.metric === "layerP95Ms" && item.decision === "fail"),
    );

    const noisy = dataset("candidate");
    noisy.scenarios.pages.startup.runs = [90, 100, 100, 100, 100, 100, 100, 100, 100, 110].map(
      (durationMs) => ({ durationMs }),
    );
    const uncertain = compareDatasets(dataset(), noisy);
    assert.equal(uncertain.status, "inconclusive");

    const sameDerivedMetrics = dataset("candidate");
    sameDerivedMetrics.scenarios.pages.interaction.groups[0].frameMs[0] = 15;
    const sameButNotRaw = compareDatasets(dataset(), sameDerivedMetrics);
    assert.equal(sameButNotRaw.status, "inconclusive");

    const leak = dataset("candidate");
    leak.scenarios.pages.lifecycle.cycles.forEach((cycle, index) => {
      cycle.rssAfterBytes += index + 1;
    });
    assert.equal(compareDatasets(dataset(), leak).status, "fail");

    const baselineLeak = dataset();
    baselineLeak.scenarios.pages.lifecycle.cycles.forEach((cycle, index) => {
      cycle.rssAfterBytes += index + 1;
    });
    const fixed = dataset("candidate");
    assert.equal(compareDatasets(baselineLeak, fixed).status, "fail");
    assert.ok(
      compareDatasets(baselineLeak, fixed).items.some(
        (item) =>
          item.scenario === "pages" &&
          item.metric === "rssTrendBytesPerCycle" &&
          item.decision === "fail",
      ),
    );

    const wrongSample = dataset("candidate");
    wrongSample.scenarios.pages.sample.label = "different input";
    assert.throws(() => compareDatasets(dataset(), wrongSample), /sample label/i);
  });

  it("uses the documented CLI exit codes and writes a reviewable report", () => {
    const directory = mkdtempSync(join(tmpdir(), "nsh-performance-"));
    const baseline = join(directory, "baseline.json");
    const candidate = join(directory, "candidate.json");
    writeFileSync(baseline, JSON.stringify(dataset()));
    writeFileSync(candidate, JSON.stringify(dataset("candidate")));
    const script = join(process.cwd(), "scripts/nsh-performance.mjs");
    const run = spawnSync(process.execPath, [script, baseline, candidate], { encoding: "utf8" });
    assert.equal(run.status, 0, run.stderr);
    assert.match(run.stdout, /schemaVersion|performance|性能/i);
    const slower = dataset("candidate");
    slower.scenarios.pages.startup.runs.forEach((item) => {
      item.durationMs = 110;
    });
    writeFileSync(candidate, JSON.stringify(slower));
    assert.equal(
      spawnSync(process.execPath, [script, baseline, candidate], { encoding: "utf8" }).status,
      1,
    );
    const uncertain = dataset("candidate");
    uncertain.scenarios.pages.startup.runs = [90, 100, 100, 100, 100, 100, 100, 100, 100, 110].map(
      (durationMs) => ({ durationMs }),
    );
    writeFileSync(candidate, JSON.stringify(uncertain));
    assert.equal(
      spawnSync(process.execPath, [script, baseline, candidate], { encoding: "utf8" }).status,
      2,
    );
    const bad = spawnSync(process.execPath, [script, baseline, join(directory, "missing.json")], {
      encoding: "utf8",
    });
    assert.equal(bad.status, 3);
    assert.match(`${bad.stdout}\n${bad.stderr}`, /invalid|error/i);
  });
});

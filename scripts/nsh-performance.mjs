#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
import { isAbsolute, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const SCHEMA_VERSION = 1;
export const SCENARIOS = ["pages", "pcb2d", "schematic", "pcb3d"];
export const EXIT_CODES = Object.freeze({ pass: 0, fail: 1, inconclusive: 2, invalid: 3 });

const SAMPLE_KINDS = new Set(["real-board", "synthetic-stress", "example"]);
const SOFTWARE_GPU = /swiftshader|llvmpipe|software|basic\s+render|cpu(?:\s+raster)?|swrast/i;
const REQUIRED_DEFINITIONS = [
  "startupMedianMs",
  "startupP95Ms",
  "frameP50Ms",
  "frameP95Ms",
  "frameP99Ms",
  "dropRatio",
  "dropP50Ratio",
  "dropP95Ratio",
  "dropP99Ratio",
  "cpuPct",
  "rssBytes",
  "gpuBytes",
  "layerP95Ms",
  "pickP95Ms",
  "bomP95Ms",
  "rssGrowthBytes",
  "gpuGrowthBytes",
  "rssTrendBytesPerCycle",
  "gpuTrendBytesPerCycle",
  "gestureCompileCount",
  "pathParseCount",
  "meshUploadCount",
];

export class InvalidPerformanceData extends Error {
  constructor(message) {
    super(`Invalid performance data: ${message}`);
    this.name = "InvalidPerformanceData";
  }
}

const invalid = (message) => {
  throw new InvalidPerformanceData(message);
};

const isObject = (value) => value !== null && typeof value === "object" && !Array.isArray(value);

function requireObject(value, path) {
  if (!isObject(value)) invalid(`${path} must be an object`);
  return value;
}

function requireString(value, path, { nonEmpty = true } = {}) {
  if (typeof value !== "string" || (nonEmpty && value.trim() === ""))
    invalid(`${path} must be a non-empty string`);
  return value;
}

function requireNumber(value, path, { min = -Infinity, integer = false } = {}) {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < min ||
    (integer && !Number.isInteger(value))
  ) {
    invalid(`${path} must be a finite number${Number.isFinite(min) ? ` >= ${min}` : ""}`);
  }
  return value;
}

function requireArray(value, path, { min = 0 } = {}) {
  if (!Array.isArray(value) || value.length < min) invalid(`${path} needs at least ${min} samples`);
  return value;
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (isObject(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function finiteArray(values, path, { min = 1, minimum = -Infinity } = {}) {
  requireArray(values, path, { min });
  values.forEach((value, index) => {
    requireNumber(value, `${path}[${index}]`, { min: minimum });
  });
  return values;
}

function validateMetadata(metadata) {
  requireObject(metadata, "metadata");
  requireString(metadata.device, "metadata.device");
  requireString(metadata.os, "metadata.os");
  const gpu = requireObject(metadata.gpu, "metadata.gpu");
  requireString(gpu.name, "metadata.gpu.name");
  if (gpu.kind !== "hardware" || SOFTWARE_GPU.test(gpu.name)) {
    invalid(`software GPU is not a valid hardware comparison target: ${gpu.name}`);
  }
  requireString(metadata.backend, "metadata.backend");
  if (SOFTWARE_GPU.test(metadata.backend))
    invalid(`software GPU/backend is not allowed: ${metadata.backend}`);
  requireNumber(metadata.refreshHz, "metadata.refreshHz", { min: Number.MIN_VALUE });
  requireNumber(metadata.dpr, "metadata.dpr", { min: Number.MIN_VALUE });
  const viewport = requireObject(metadata.viewport, "metadata.viewport");
  requireNumber(viewport.width, "metadata.viewport.width", { min: Number.MIN_VALUE });
  requireNumber(viewport.height, "metadata.viewport.height", { min: Number.MIN_VALUE });
  if (typeof metadata.inputSha256 !== "string" || !/^[a-f0-9]{64}$/i.test(metadata.inputSha256)) {
    invalid("metadata.inputSha256 must be a 64-character SHA-256 hex digest");
  }
  if (!["cold", "warm"].includes(metadata.cache)) invalid("metadata.cache must be cold or warm");
  if (metadata.release !== true)
    invalid("metadata.release must be true; debug/headless data cannot pass");

  const definitions = requireObject(metadata.metricDefinitions, "metadata.metricDefinitions");
  for (const name of REQUIRED_DEFINITIONS)
    requireString(definitions[name], `metadata.metricDefinitions.${name}`);

  const sampling = requireObject(metadata.sampling, "metadata.sampling");
  if (sampling.method !== "independent-run-bootstrap")
    invalid("metadata.sampling.method must be independent-run-bootstrap");
  requireNumber(sampling.startupRuns, "metadata.sampling.startupRuns", { min: 10, integer: true });
  requireNumber(sampling.interactionGroups, "metadata.sampling.interactionGroups", {
    min: 5,
    integer: true,
  });
  requireNumber(sampling.interactionDurationSec, "metadata.sampling.interactionDurationSec", {
    min: 60,
  });
  requireNumber(sampling.lifecycleCycles, "metadata.sampling.lifecycleCycles", {
    min: 20,
    integer: true,
  });
  requireNumber(sampling.bootstrapReplicates, "metadata.sampling.bootstrapReplicates", {
    min: 1000,
    integer: true,
  });
  if (sampling.confidence !== 0.95) {
    invalid("metadata.sampling.confidence must be 0.95");
  }
  return metadata;
}

function validateSample(sample, path) {
  const value = requireObject(sample, path);
  requireString(value.label, `${path}.label`);
  if (!SAMPLE_KINDS.has(value.kind))
    invalid(`${path}.kind must be real-board, synthetic-stress, or example`);
}

function exactCommit(dataset) {
  if (
    dataset.exactCommit !== undefined &&
    dataset.exactcommit !== undefined &&
    dataset.exactCommit !== dataset.exactcommit
  ) {
    invalid("exactCommit and exactcommit disagree");
  }
  return dataset.exactCommit ?? dataset.exactcommit;
}

function validateScenario(scenario, name, metadata) {
  const path = `scenarios.${name}`;
  const value = requireObject(scenario, path);
  validateSample(value.sample, `${path}.sample`);

  const startup = requireObject(value.startup, `${path}.startup`);
  const startupRuns = requireArray(startup.runs, `${path}.startup.runs`, {
    min: metadata.sampling.startupRuns,
  });
  startupRuns.forEach((run, index) => {
    const item = requireObject(run, `${path}.startup.runs[${index}]`);
    requireNumber(item.durationMs, `${path}.startup.runs[${index}].durationMs`, { min: 0 });
  });

  const interaction = requireObject(value.interaction, `${path}.interaction`);
  const groups = requireArray(interaction.groups, `${path}.interaction.groups`, {
    min: metadata.sampling.interactionGroups,
  });
  groups.forEach((group, index) => {
    const itemPath = `${path}.interaction.groups[${index}]`;
    const item = requireObject(group, itemPath);
    requireNumber(item.durationSec, `${itemPath}.durationSec`, {
      min: metadata.sampling.interactionDurationSec,
    });
    finiteArray(item.frameMs, `${itemPath}.frameMs`, { min: 2, minimum: Number.MIN_VALUE });
    const observedMs = item.frameMs.reduce((total, ms) => total + ms, 0);
    const boundaryMs = 1000 / metadata.refreshHz;
    if (Math.abs(observedMs - item.durationSec * 1000) > boundaryMs) {
      invalid(
        `${itemPath}.frameMs observation coverage must match durationSec within one refresh interval`,
      );
    }
    requireNumber(item.droppedFrames, `${itemPath}.droppedFrames`, { min: 0, integer: true });
    requireNumber(item.cpuPct, `${itemPath}.cpuPct`, { min: 0 });
    requireNumber(item.rssBytes, `${itemPath}.rssBytes`, { min: 0 });
    requireNumber(item.gpuBytes, `${itemPath}.gpuBytes`, { min: 0 });
    const actions = requireObject(item.actions, `${itemPath}.actions`);
    for (const action of ["layerMs", "pickMs", "bomMs"])
      finiteArray(actions[action], `${itemPath}.actions.${action}`, { min: 1, minimum: 0 });
    const gesture = requireObject(item.gesture, `${itemPath}.gesture`);
    for (const count of ["compile", "pathParse", "meshUpload"]) {
      requireNumber(gesture[count], `${itemPath}.gesture.${count}`, { min: 0, integer: true });
      if (gesture[count] !== 0)
        invalid(`${itemPath}.gesture.${count} must be zero during interaction`);
    }
  });

  const lifecycle = requireObject(value.lifecycle, `${path}.lifecycle`);
  const cycles = requireArray(lifecycle.cycles, `${path}.lifecycle.cycles`, {
    min: metadata.sampling.lifecycleCycles,
  });
  cycles.forEach((cycle, index) => {
    const itemPath = `${path}.lifecycle.cycles[${index}]`;
    const item = requireObject(cycle, itemPath);
    for (const metric of ["rssBeforeBytes", "rssAfterBytes", "gpuBeforeBytes", "gpuAfterBytes"]) {
      requireNumber(item[metric], `${itemPath}.${metric}`, { min: 0 });
    }
  });
  return value;
}

/** Validate one capture. Throws InvalidPerformanceData; no invalid capture is compared. */
export function validateDataset(dataset) {
  const value = requireObject(dataset, "dataset");
  if (value.schemaVersion !== SCHEMA_VERSION) invalid(`schemaVersion must be ${SCHEMA_VERSION}`);
  if (!new Set(["baseline", "candidate"]).has(value.role))
    invalid("role must be baseline or candidate");
  requireString(exactCommit(value), "exactCommit");
  const metadata = validateMetadata(value.metadata);
  const scenarios = requireObject(value.scenarios, "scenarios");
  for (const name of SCENARIOS) validateScenario(scenarios[name], name, metadata);
  return value;
}

function quantile(values, probability) {
  const sorted = [...values].sort((left, right) => left - right);
  const index = (sorted.length - 1) * probability;
  const lower = Math.floor(index);
  const upper = Math.ceil(index);
  if (lower === upper) return sorted[lower];
  return sorted[lower] + (sorted[upper] - sorted[lower]) * (index - lower);
}

function p95(values) {
  return quantile(values, 0.95);
}

function median(values) {
  return quantile(values, 0.5);
}

function fnv1a(value) {
  let hash = 2166136261;
  for (const byte of new TextEncoder().encode(value)) {
    hash ^= byte;
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return hash >>> 0;
}

function random(seed) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state ^ (state >>> 16), 2246822507) + 3266489909) >>> 0;
    state ^= state >>> 13;
    return (state >>> 0) / 4294967296;
  };
}

function bootstrap(valuesA, valuesB, estimate, replicates, confidence, seed) {
  const sampleA = valuesA.length;
  const sampleB = valuesB.length;
  const rng = random(fnv1a(seed));
  const differences = [];
  for (let repetition = 0; repetition < replicates; repetition += 1) {
    const resampledA = Array.from({ length: sampleA }, () => valuesA[Math.floor(rng() * sampleA)]);
    const resampledB = Array.from({ length: sampleB }, () => valuesB[Math.floor(rng() * sampleB)]);
    differences.push(estimate(resampledB) - estimate(resampledA));
  }
  const alpha = (1 - confidence) / 2;
  return { low: quantile(differences, alpha), high: quantile(differences, 1 - alpha) };
}

function formatNumber(value) {
  if (Object.is(value, -0)) return "0";
  return Number.isInteger(value)
    ? String(value)
    : value.toFixed(4).replace(/0+$/, "").replace(/\.$/, "");
}

function compareMetric({
  scenario,
  metric,
  baseline,
  candidate,
  estimate,
  sampling,
  forbidPositive = false,
  rawEqual = false,
}) {
  const baselineEstimate = estimate(baseline);
  const candidateEstimate = estimate(candidate);
  const exact = rawEqual;
  const ci = exact
    ? { low: 0, high: 0 }
    : bootstrap(
        baseline,
        candidate,
        estimate,
        sampling.bootstrapReplicates,
        sampling.confidence,
        `${scenario}:${metric}`,
      );
  const delta = candidateEstimate - baselineEstimate;
  let decision;
  let reason;
  if (forbidPositive && (baselineEstimate > 0 || candidateEstimate > 0)) {
    decision = "fail";
    reason =
      "continuous lifecycle growth detected in baseline or candidate; both releases must return to their starting allocation";
  } else if (exact) {
    decision = "pass";
    reason = "exact equal independent-run estimates";
  } else if (ci.high < 0) {
    decision = "pass";
    reason = "candidate is strictly lower; bootstrap upper bound is < 0";
  } else if (ci.low > 0) {
    decision = "fail";
    reason = "candidate regression; bootstrap lower bound is > 0";
  } else {
    decision = "inconclusive";
    reason = "bootstrap interval crosses zero; more independent runs are required";
  }
  return {
    scenario,
    metric,
    baseline: baselineEstimate,
    candidate: candidateEstimate,
    delta,
    ci,
    decision,
    reason,
  };
}

function metricValues(dataset, scenarioName) {
  const scenario = dataset.scenarios[scenarioName];
  const startup = scenario.startup.runs.map((run) => run.durationMs);
  const groups = scenario.interaction.groups;
  const interactions = {
    frameP50Ms: groups.map((group) => quantile(group.frameMs, 0.5)),
    frameP95Ms: groups.map((group) => quantile(group.frameMs, 0.95)),
    frameP99Ms: groups.map((group) => quantile(group.frameMs, 0.99)),
    dropRatio: groups.map(
      (group) => group.droppedFrames / (group.frameMs.length + group.droppedFrames),
    ),
    dropP50Ratio: groups.map(
      (group) => group.droppedFrames / (group.frameMs.length + group.droppedFrames),
    ),
    dropP95Ratio: groups.map(
      (group) => group.droppedFrames / (group.frameMs.length + group.droppedFrames),
    ),
    dropP99Ratio: groups.map(
      (group) => group.droppedFrames / (group.frameMs.length + group.droppedFrames),
    ),
    cpuPct: groups.map((group) => group.cpuPct),
    rssBytes: groups.map((group) => group.rssBytes),
    gpuBytes: groups.map((group) => group.gpuBytes),
    layerP95Ms: groups.map((group) => p95(group.actions.layerMs)),
    pickP95Ms: groups.map((group) => p95(group.actions.pickMs)),
    bomP95Ms: groups.map((group) => p95(group.actions.bomMs)),
    gestureCompileCount: groups.map((group) => group.gesture.compile),
    pathParseCount: groups.map((group) => group.gesture.pathParse),
    meshUploadCount: groups.map((group) => group.gesture.meshUpload),
  };
  const cycles = scenario.lifecycle.cycles;
  const lifecycle = {
    rssGrowthBytes: cycles.map((cycle) => cycle.rssAfterBytes - cycle.rssBeforeBytes),
    gpuGrowthBytes: cycles.map((cycle) => cycle.gpuAfterBytes - cycle.gpuBeforeBytes),
    rssTrendBytesPerCycle: [linearSlope(cycles.map((cycle) => cycle.rssAfterBytes))],
    gpuTrendBytesPerCycle: [linearSlope(cycles.map((cycle) => cycle.gpuAfterBytes))],
  };
  return {
    startupMedianMs: { values: startup, estimate: median },
    startupP95Ms: { values: startup, estimate: p95 },
    ...Object.fromEntries(
      Object.entries(interactions).map(([metric, values]) => [
        metric,
        {
          values,
          estimate: [
            "cpuPct",
            "rssBytes",
            "gpuBytes",
            "layerP95Ms",
            "pickP95Ms",
            "bomP95Ms",
          ].includes(metric)
            ? p95
            : metric === "dropP95Ratio"
              ? (items) => quantile(items, 0.95)
              : metric === "dropP99Ratio"
                ? (items) => quantile(items, 0.99)
                : median,
        },
      ]),
    ),
    ...Object.fromEntries(
      Object.entries(lifecycle).map(([metric, values]) => [metric, { values, estimate: p95 }]),
    ),
  };
}

function rawMetricInput(dataset, scenarioName, metric) {
  const scenario = dataset.scenarios[scenarioName];
  if (metric.startsWith("startup")) return scenario.startup;
  if (["rssGrowthBytes", "rssTrendBytesPerCycle"].includes(metric)) {
    return scenario.lifecycle.cycles.map(({ rssBeforeBytes, rssAfterBytes }) => ({
      rssBeforeBytes,
      rssAfterBytes,
    }));
  }
  if (["gpuGrowthBytes", "gpuTrendBytesPerCycle"].includes(metric)) {
    return scenario.lifecycle.cycles.map(({ gpuBeforeBytes, gpuAfterBytes }) => ({
      gpuBeforeBytes,
      gpuAfterBytes,
    }));
  }
  const groups = scenario.interaction.groups;
  if (metric.startsWith("frame")) return groups.map(({ frameMs }) => frameMs);
  if (metric.startsWith("drop"))
    return groups.map(({ frameMs, droppedFrames }) => ({
      frameCount: frameMs.length,
      droppedFrames,
    }));
  if (metric === "cpuPct") return groups.map(({ cpuPct }) => cpuPct);
  if (metric === "rssBytes") return groups.map(({ rssBytes }) => rssBytes);
  if (metric === "gpuBytes") return groups.map(({ gpuBytes }) => gpuBytes);
  if (metric === "layerP95Ms") return groups.map(({ actions }) => actions.layerMs);
  if (metric === "pickP95Ms") return groups.map(({ actions }) => actions.pickMs);
  if (metric === "bomP95Ms") return groups.map(({ actions }) => actions.bomMs);
  if (metric === "gestureCompileCount") return groups.map(({ gesture }) => gesture.compile);
  if (metric === "pathParseCount") return groups.map(({ gesture }) => gesture.pathParse);
  if (metric === "meshUploadCount") return groups.map(({ gesture }) => gesture.meshUpload);
  return groups;
}

function linearSlope(values) {
  const meanX = (values.length - 1) / 2;
  const meanY = values.reduce((sum, value) => sum + value, 0) / values.length;
  let numerator = 0;
  let denominator = 0;
  values.forEach((value, index) => {
    numerator += (index - meanX) * (value - meanY);
    denominator += (index - meanX) ** 2;
  });
  return denominator === 0 ? 0 : numerator / denominator;
}

function statusRank(status) {
  return { pass: 0, inconclusive: 1, fail: 2 }[status];
}

function renderReport({ status, baseline, candidate, items, error = null }) {
  const lines = [
    "# NSH performance comparison",
    "",
    `- Status: **${status.toUpperCase()}**`,
    `- Schema: ${SCHEMA_VERSION}`,
    `- Roles: ${baseline?.role ?? "unavailable"} / ${candidate?.role ?? "unavailable"}`,
    `- Baseline commit: \`${baseline ? (exactCommit(baseline) ?? "unavailable") : "unavailable"}\``,
    `- Candidate commit: \`${candidate ? (exactCommit(candidate) ?? "unavailable") : "unavailable"}\``,
  ];
  if (error) return `${lines.join("\n")}\n\n**Invalid input:** ${error}\n`;
  lines.push(
    `- Device / OS / GPU / backend: ${baseline.metadata.device} / ${baseline.metadata.os} / ${baseline.metadata.gpu.name} / ${baseline.metadata.backend}`,
    `- Refresh / DPR / viewport: ${baseline.metadata.refreshHz} Hz / ${baseline.metadata.dpr} / ${baseline.metadata.viewport.width}×${baseline.metadata.viewport.height}`,
    `- Input SHA-256: \`${baseline.metadata.inputSha256}\``,
    `- Cache: ${baseline.metadata.cache}; Release: ${baseline.metadata.release}`,
    `- Sampling: ${baseline.metadata.sampling.method}; bootstrap=${baseline.metadata.sampling.bootstrapReplicates}; confidence=${baseline.metadata.sampling.confidence}`,
    "",
    "Sample labels:",
    ...SCENARIOS.map(
      (name) =>
        `- ${name}: ${baseline.scenarios[name].sample.label} (${baseline.scenarios[name].sample.kind})`,
    ),
    "",
    "| Scenario | Metric | Baseline | Candidate | Delta | 95% CI for delta | Decision | Reason |",
    "| --- | --- | ---: | ---: | ---: | ---: | --- | --- |",
  );
  for (const item of items) {
    lines.push(
      `| ${item.scenario} | ${item.metric} | ${formatNumber(item.baseline)} | ${formatNumber(item.candidate)} | ${formatNumber(item.delta)} | [${formatNumber(item.ci.low)}, ${formatNumber(item.ci.high)}] | ${item.decision} | ${item.reason} |`,
    );
  }
  return `${lines.join("\n")}\n`;
}

/** Compare two already parsed captures. Throws on invalid data or metadata mismatch. */
export function compareDatasets(baseline, candidate) {
  const baselineValue = validateDataset(baseline);
  const candidateValue = validateDataset(candidate);
  if (baselineValue.role !== "baseline" || candidateValue.role !== "candidate")
    invalid("captures must be passed as baseline then candidate");
  if (stableJson(baselineValue.metadata) !== stableJson(candidateValue.metadata))
    invalid("baseline and candidate metadata must match exactly");
  for (const scenario of SCENARIOS) {
    if (
      stableJson(baselineValue.scenarios[scenario].sample) !==
      stableJson(candidateValue.scenarios[scenario].sample)
    ) {
      invalid(`sample label/kind differs for ${scenario}`);
    }
  }
  const items = [];
  for (const scenario of SCENARIOS) {
    const left = metricValues(baselineValue, scenario);
    const right = metricValues(candidateValue, scenario);
    for (const metric of Object.keys(left)) {
      items.push(
        compareMetric({
          scenario,
          metric,
          baseline: left[metric].values,
          candidate: right[metric].values,
          estimate: left[metric].estimate,
          sampling: baselineValue.metadata.sampling,
          forbidPositive: metric.endsWith("TrendBytesPerCycle"),
          rawEqual:
            stableJson(rawMetricInput(baselineValue, scenario, metric)) ===
            stableJson(rawMetricInput(candidateValue, scenario, metric)),
        }),
      );
    }
  }
  const status = items.reduce(
    (current, item) => (statusRank(item.decision) > statusRank(current) ? item.decision : current),
    "pass",
  );
  return {
    status,
    items,
    report: renderReport({ status, baseline: baselineValue, candidate: candidateValue, items }),
  };
}

// Short import name for capture/CI callers. Paths are accepted as a convenience;
// callers that already parsed JSON can pass the two objects directly.
export function compare(baseline, candidate) {
  return compareDatasets(
    typeof baseline === "string" ? readJson(baseline) : baseline,
    typeof candidate === "string" ? readJson(candidate) : candidate,
  );
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    invalid(`${path}: ${error.message}`);
  }
}

export function main(argv = process.argv.slice(2)) {
  if (argv.length < 2 || argv.length > 3) {
    console.error(
      "Usage: node scripts/nsh-performance.mjs baseline.json candidate.json [report.md]",
    );
    return EXIT_CODES.invalid;
  }
  try {
    const result = compareDatasets(readJson(argv[0]), readJson(argv[1]));
    if (argv[2]) writeFileSync(isAbsolute(argv[2]) ? argv[2] : resolve(argv[2]), result.report);
    process.stdout.write(result.report);
    return EXIT_CODES[result.status];
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const report = renderReport({
      status: "invalid",
      error: message,
      baseline: null,
      candidate: null,
      items: [],
    });
    process.stdout.write(report);
    console.error(message);
    return EXIT_CODES.invalid;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) process.exitCode = main();

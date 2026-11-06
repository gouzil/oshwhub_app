import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { setTimeout } from "node:timers/promises";
import { fileURLToPath } from "node:url";

const serial = process.env.NSH_ANDROID_SERIAL;
assert.ok(serial, "Set NSH_ANDROID_SERIAL to the test emulator/device");
const adb = process.env.ADB || "adb";
const call = (...args) => execFileSync(adb, ["-s", serial, ...args], { encoding: "utf8" });
const output = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../.superpowers/sdd/nsh-migration-plan/android-gpu",
);
mkdirSync(output, { recursive: true });
const snapshot = () => {
  call("shell", "uiautomator", "dump", "/data/local/tmp/oshwhub-surface.xml");
  return call("shell", "cat", "/data/local/tmp/oshwhub-surface.xml");
};
async function waitFor(pattern) {
  let xml;
  for (let attempt = 0; attempt < 20; attempt++) {
    xml = snapshot();
    if (pattern.test(xml)) return xml;
    await setTimeout(250);
  }
  writeFileSync(`${output}/failure.xml`, xml);
  assert.fail(`Timed out waiting for ${pattern}; see ${output}/failure.xml`);
}
function tap(xml, label) {
  const node = xml.match(new RegExp(`<node[^>]*(?:text|content-desc)="${label}"[^>]*>`));
  assert.ok(node, `missing ${label}`);
  const bounds = node[0].match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);
  assert.ok(bounds, `missing ${label} bounds`);
  const [, x, y, right, bottom] = bounds.map(Number);
  assert.ok(right > x && bottom > y, `${label} has no visible area`);
  call(
    "shell",
    "input",
    "tap",
    `${Math.round((x + right) / 2)}`,
    `${Math.round((y + bottom) / 2)}`,
  );
}

call("shell", "am", "force-stop", "com.gouzil.oshwhub.surfaceprobe");
call(
  "shell",
  "am",
  "start",
  "-n",
  "com.gouzil.oshwhub.surfaceprobe/com.longbridge.nsh.AppActivity",
);
let xml = await waitFor(/Ready.*backend:.*(?:Vulkan|Gl)/);
writeFileSync(`${output}/ready.xml`, xml);
tap(xml, "Resize GPU");
const density = Number(call("shell", "wm", "density").match(/\d+/g).at(-1));
xml = await waitFor(new RegExp(`Ready.*width: ${Math.ceil((480 * density) / 160)}`));
writeFileSync(`${output}/resized.xml`, xml);
tap(xml, "Resize GPU");
xml = await waitFor(new RegExp(`Ready.*width: ${Math.ceil((320 * density) / 160)}`));
for (let cycle = 0; cycle < 20; cycle++) {
  tap(xml, "Toggle GPU");
  xml = snapshot();
  assert.doesNotMatch(xml, /text="Overlay"/);
  tap(xml, "Toggle GPU");
  xml = await waitFor(/Ready.*backend:.*(?:Vulkan|Gl)/);
  assert.match(xml, /text="Overlay"/);
}
writeFileSync(`${output}/remounted.xml`, xml);
tap(xml, "Show PCB");
xml = await waitFor(/个器件/);
writeFileSync(`${output}/pcb.xml`, xml);
const screenshot = execFileSync(adb, ["-s", serial, "exec-out", "screencap", "-p"]);
writeFileSync(`${output}/pcb.png`, screenshot);
tap(xml, "放大三维场景");
xml = snapshot();
const zoomed = execFileSync(adb, ["-s", serial, "exec-out", "screencap", "-p"]);
assert.notDeepEqual(zoomed, screenshot, "PCB zoom must update displayed pixels");
writeFileSync(`${output}/pcb-zoomed.png`, zoomed);
const canvas = xml.match(
  /<node[^>]*content-desc="三维画布"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/,
);
assert.ok(canvas, "missing PCB gesture area");
const [, left, top, right, bottom] = canvas.map(Number);
const cx = Math.round((left + right) / 2),
  cy = Math.round((top + bottom) / 2);
call("shell", "input", "swipe", `${cx}`, `${cy}`, `${cx + 80}`, `${cy + 40}`, "300");
snapshot();
const rotated = execFileSync(adb, ["-s", serial, "exec-out", "screencap", "-p"]);
assert.notDeepEqual(rotated, zoomed, "PCB drag must update displayed pixels");
writeFileSync(`${output}/pcb-rotated.png`, rotated);

tap(snapshot(), "Retained GPU");
xml = await waitFor(/Retained Ready/);
function screenPixels() {
  const raw = execFileSync(adb, ["-s", serial, "exec-out", "screencap"], {
    maxBuffer: 64 * 1024 * 1024,
  });
  const width = raw.readUInt32LE(0),
    height = raw.readUInt32LE(4),
    format = raw.readUInt32LE(8);
  assert.ok(format === 1 || format === 2, `unsupported screencap pixel format ${format}`);
  const start = raw.length - width * height * 4;
  assert.ok(start >= 12 && start <= 64, "invalid raw screencap layout");
  return { width, pixels: raw.subarray(start) };
}
function redPixels() {
  const { pixels } = screenPixels();
  let red = 0;
  for (let at = 0; at < pixels.length; at += 4)
    if (pixels[at] > 70 && pixels[at + 1] < 40 && pixels[at + 2] < 40) red++;
  return red;
}
assert.equal(redPixels(), 0, "retained surface initially has no board");
tap(xml, "Hide retained GPU");
xml = await waitFor(/Show retained GPU/);
tap(xml, "Load retained board");
xml = snapshot();
tap(xml, "Show retained GPU");
await setTimeout(600);
writeFileSync(
  `${output}/retained-restored.png`,
  execFileSync(adb, ["-s", serial, "exec-out", "screencap", "-p"]),
);
assert.ok(
  redPixels() > 100,
  "show must draw the board updated while its SurfaceTexture was retained",
);
tap(snapshot(), "Retained GPU");
tap(snapshot(), "Show drawing");
xml = await waitFor(/content-desc="二维画布"/);
writeFileSync(`${output}/drawing.xml`, xml);
writeFileSync(
  `${output}/drawing.png`,
  execFileSync(adb, ["-s", serial, "exec-out", "screencap", "-p"]),
);
const drawing = xml.match(
  /<node[^>]*content-desc="二维画布"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/,
);
assert.ok(drawing, "missing 2D gesture area");
const [, dx, dy, dr, db] = drawing.map(Number);
const inset = Math.min((24 * density) / 160, (dr - dx) * 0.15, (db - dy) * 0.15);
const fit = Math.min((dr - dx - inset * 2) / 120, (db - dy - inset * 2) / 80);
function checkDrawing(zoom) {
  const { width, pixels } = screenPixels();
  const at = (x, y) => {
    const column = Math.round((dx + dr) / 2 + (x - 60) * fit * zoom);
    const row = Math.round((dy + db) / 2 + (y - 40) * fit * zoom);
    assert.ok(column >= dx && column < dr && row >= dy && row < db);
    return (row * width + column) * 4;
  };
  for (const [x, y, expected, label] of [
    [15, 50, [136, 63, 19], "translucent copper"],
    [25, 50, [17, 24, 39], "nonzero hole"],
    [103, 60, [0, 255, 0], "SVG background"],
    [69, 62, [255, 255, 255], "SVG embedded PNG"],
  ]) {
    const index = at(x, y);
    for (let channel = 0; channel < 3; channel++)
      assert.ok(
        Math.abs(pixels[index + channel] - expected[channel]) <= 2,
        `${label} channel ${channel}: ${pixels[index + channel]}`,
      );
  }
  let textPixels = 0;
  const first = at(60, 35),
    last = at(82, 53);
  const left = (first / 4) % width,
    right = (last / 4) % width;
  for (let row = Math.floor(first / 4 / width); row <= Math.floor(last / 4 / width); row++)
    for (let column = left; column <= right; column++) {
      const index = (row * width + column) * 4;
      if (pixels[index] > 220 && pixels[index + 1] > 220 && pixels[index + 2] > 220) textPixels++;
    }
  assert.ok(textPixels > 50, "SVG text must reach the native canvas");
}
checkDrawing(1);
tap(xml, "放大二维画布");
xml = await waitFor(/text="125%"/);
checkDrawing(1.25);
const centerX = Math.round((dx + dr) / 2),
  centerY = Math.round((dy + db) / 2);
call(
  "shell",
  "input",
  "swipe",
  `${centerX}`,
  `${centerY}`,
  `${Math.round(centerX - 24 * fit * 1.25)}`,
  `${Math.round(centerY - 20 * fit * 1.25)}`,
  "300",
);
for (let step = 0; step < 7; step++) tap(xml, "放大二维画布");
xml = await waitFor(/text="596%"/);
const deep = screenPixels();
let peak = 0;
for (let x = centerX - 40; x <= centerX + 40; x++) {
  const index = (centerY * deep.width + x) * 4;
  peak = Math.max(peak, Math.min(deep.pixels[index], deep.pixels[index + 2]));
}
writeFileSync(
  `${output}/drawing-deep.png`,
  execFileSync(adb, ["-s", serial, "exec-out", "screencap", "-p"]),
);
assert.ok(peak > 240, `deep zoom must recover the SVG's thin white stroke, got ${peak}`);
console.info(
  "Android GPU ready, resize, overlay, 20 mount/dispose cycles, PCB zoom/drag, retained hide/update/show and 2D alpha/hole/SVG/PNG/zoom passed; pixels saved for inspection.",
);

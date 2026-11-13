import assert from "node:assert/strict";
import { setTimeout } from "node:timers/promises";

export async function waitForHierarchy(snapshot, pattern) {
  let diagnostic = "No hierarchy returned";
  const deadline = Date.now() + 60_000;
  for (let attempt = 0; attempt < 20 && Date.now() < deadline; attempt++) {
    try {
      const xml = snapshot();
      diagnostic = xml;
      assert.match(xml, /<hierarchy\b[^>]*>[\s\S]*<\/hierarchy>\s*$/);
      if (pattern.test(xml)) return xml;
    } catch (error) {
      diagnostic = error.stack;
    }
    if (attempt < 19) await setTimeout(250);
  }
  assert.fail(`Timed out waiting for ${pattern}; last observation:\n${diagnostic}`);
}

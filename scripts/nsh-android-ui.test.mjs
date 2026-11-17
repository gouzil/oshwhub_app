import assert from "node:assert/strict";
import test from "node:test";
import { waitForHierarchy } from "./nsh-android-ui.mjs";

const ready = '<hierarchy><node text="Ready backend: Vulkan"/></hierarchy>';

test("Android readiness retries missing roots and waits for the requested state", async () => {
  const observations = [
    new Error("cat: hierarchy.xml: No such file or directory"),
    "ERROR: null root node returned by UiTestAutomationBridge.",
    '<hierarchy><node text="Loading"/></hierarchy>',
    ready,
  ];
  const xml = await waitForHierarchy(() => {
    const observation = observations.shift();
    if (observation instanceof Error) throw observation;
    return observation;
  }, /Ready.*Vulkan/);
  assert.equal(xml, ready);
});

test("Android readiness rejects incomplete XML even when it contains the requested text", async () => {
  const observations = ['<hierarchy><node text="Ready backend: Vulkan"/>', ready];
  assert.equal(await waitForHierarchy(() => observations.shift(), /Ready.*Vulkan/), ready);
});

test(
  "Android readiness fails within its retry budget and retains the last error",
  { timeout: 15_000 },
  async () => {
    await assert.rejects(
      waitForHierarchy(() => {
        throw new Error("device offline");
      }, /Ready/),
      /Timed out.*Ready[\s\S]*device offline/,
    );
  },
);

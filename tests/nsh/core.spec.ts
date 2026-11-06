import { expect, test } from "@playwright/test";

type Probe = {
  fetch_probe(url: string, limit: number, mode: string): Promise<string>;
  command(input: string): Promise<string>;
  start_probe(url: string): void;
  cancel_probe(): void;
  validate_pick(scene: string): string;
};
declare global {
  interface Window {
    coreProbe: Probe;
  }
}

const endpoint = "https://oshwhub.com/api/nsh-probe";
const headers = { "access-control-allow-origin": "*", "content-type": "application/json" };

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(async () => {
    const path = "/probe.js";
    const probe = await import(path);
    await probe.default();
    window.coreProbe = probe;
  });
});

test("browser core rejects unsafe URLs and zero download limits before fetch", async ({ page }) => {
  let requests = 0;
  await page.route("https://**/*", async (route) => {
    requests++;
    await route.abort();
  });
  const codes = await page.evaluate(async (url) => {
    const bad = ["http://oshwhub.com/a", "https://evil.test/a", "https://user@oshwhub.com/a"];
    const result = await Promise.all(bad.map((u) => window.coreProbe.fetch_probe(u, 32, "bytes")));
    result.push(await window.coreProbe.fetch_probe(url, 0, "bytes"));
    return result.map((s) => JSON.parse(s).code);
  }, endpoint);
  expect(codes).toEqual(["INVALID_URL", "INVALID_URL", "INVALID_URL", "INVALID_LIMIT"]);
  expect(requests).toBe(0);
});

test("actual browser fetch keeps JSON, status and MIME validation", async ({ page }) => {
  for (const [status, contentType, body, code] of [
    [200, "application/json", '{"value":7}', null],
    [418, "application/json", "{}", "BLOCKED"],
    [429, "application/json", "{}", "RATE_LIMITED"],
    [200, "text/html", "<html>blocked</html>", "BLOCKED"],
    [200, "application/json", "{broken", "INVALID_RESPONSE"],
  ] as const) {
    await page.route(endpoint, (route) =>
      route.fulfill({ status, headers: { ...headers, "content-type": contentType }, body }),
    );
    const response = JSON.parse(
      await page.evaluate((url) => window.coreProbe.fetch_probe(url, 32, "json"), endpoint),
    );
    if (code) expect(response.code).toBe(code);
    else expect(response.data).toEqual({ value: 7 });
    await page.unroute(endpoint);
  }
});

test("redirects are rejected before the destination is requested", async ({ page }) => {
  let destinationRequests = 0;
  await page.route("https://evil.test/**", async (route) => {
    destinationRequests++;
    await route.abort();
  });
  await page.route(endpoint, (route) =>
    route.fulfill({ status: 302, headers: { ...headers, location: "https://evil.test/private" } }),
  );
  const result = JSON.parse(
    await page.evaluate((url) => window.coreProbe.fetch_probe(url, 32, "bytes"), endpoint),
  );
  expect(result.code).toBe("REDIRECT_BLOCKED");
  expect(destinationRequests).toBe(0);
});

test("JSON POST uses the browser preflight and preserves its typed payload", async ({ page }) => {
  let payload: unknown;
  await page.route(endpoint, async (route) => {
    const cors = {
      ...headers,
      "access-control-allow-methods": "POST, OPTIONS",
      "access-control-allow-headers": "content-type",
    };
    if (route.request().method() === "OPTIONS") {
      await route.fulfill({ status: 204, headers: cors });
    } else {
      payload = route.request().postDataJSON();
      await route.fulfill({ status: 200, headers: cors, body: '{"accepted":true}' });
    }
  });
  const response = JSON.parse(
    await page.evaluate((url) => window.coreProbe.fetch_probe(url, 32, "post"), endpoint),
  );
  expect(payload).toEqual({ probe: true });
  expect(response.data).toEqual({ accepted: true });
});

test("both declared and streamed bodies are bounded", async ({ page }) => {
  await page.route(endpoint, (route) =>
    route.fulfill({ status: 200, headers, body: "x".repeat(33) }),
  );
  const declared = JSON.parse(
    await page.evaluate((url) => window.coreProbe.fetch_probe(url, 32, "bytes"), endpoint),
  );
  expect(declared.code).toBe("RESPONSE_TOO_LARGE");
  // A synthetic stream controls chunk boundaries; route.fulfill buffers its body.
  const streamed = await page.evaluate(async (url) => {
    const fetch = window.fetch;
    window.fetch = async () =>
      new Response(
        new ReadableStream({
          start(controller) {
            controller.enqueue(new Uint8Array(32));
            controller.enqueue(new Uint8Array(1));
            controller.close();
          },
        }),
      );
    try {
      return JSON.parse(await window.coreProbe.fetch_probe(url, 32, "bytes"));
    } finally {
      window.fetch = fetch;
    }
  }, endpoint);
  expect(streamed.code).toBe("RESPONSE_TOO_LARGE");
});

test("deadline covers the pending response and cancellation aborts fetch", async ({ page }) => {
  await page.clock.install();
  await page.route(endpoint, () => new Promise(() => {}));
  const requested = page.waitForRequest(endpoint);
  const timed = page.evaluate((url) => window.coreProbe.fetch_probe(url, 32, "bytes"), endpoint);
  await requested;
  await page.clock.fastForward(30_001);
  expect(JSON.parse(await timed).code).toBe("TIMEOUT");

  const started = page.waitForRequest(endpoint);
  await page.evaluate((url) => window.coreProbe.start_probe(url), endpoint);
  await started;
  const failed = page.waitForEvent("requestfailed", {
    predicate: (request) => request.url() === endpoint,
  });
  await page.evaluate(() => window.coreProbe.cancel_probe());
  expect((await failed).failure()?.errorText).toContain("ERR_ABORTED");
});

test("WASM compiles and picks a scene and builds the indexed 3D board", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const scene = JSON.parse(await window.coreProbe.command('{"command":"demo"}'));
    const solid = JSON.parse(
      await window.coreProbe.command(JSON.stringify({ command: "pcb3d", sceneId: scene.data.id })),
    );
    return { scene, solid, bad: window.coreProbe.validate_pick(scene.data.id) };
  });
  expect(result.scene.ok).toBe(true);
  expect(result.scene.data.objectCount).toBeGreaterThan(0);
  expect(result.solid.ok).toBe(true);
  expect(result.solid.data).toBeTruthy();
  expect(result.bad).not.toBe("null");
});

import { afterEach, describe, expect, it, vi } from "vitest";
import { BridgeError, call, openExternal } from "./index.web";

afterEach(() => vi.unstubAllGlobals());

describe("desktop web bridge", () => {
  it("dispatches through the isolated API and preserves Rust business errors", async () => {
    const dispatch = vi.fn().mockResolvedValue(
      JSON.stringify({
        ok: false,
        error: { code: "scene_expired", message: "expired", requestId: "rust-id" },
      }),
    );
    vi.stubGlobal("window", { oshwhubDesktop: { dispatch } });
    await expect(call({ command: "health" })).rejects.toEqual(
      new BridgeError("scene_expired", "expired", "rust-id"),
    );
    expect(dispatch).toHaveBeenCalledWith('{"command":"health"}');
  });

  it("keeps browser preview fetch and external-link fallback", async () => {
    const open = vi.fn();
    const fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ ok: true, data: { protocol: 1 }, requestId: "dev" }),
    });
    vi.stubGlobal("window", { open });
    vi.stubGlobal("fetch", fetch);
    await expect(call({ command: "health" })).resolves.toEqual({ protocol: 1 });
    expect(fetch).toHaveBeenCalledWith(
      "http://127.0.0.1:4318/bridge",
      expect.objectContaining({ method: "POST", body: '{"command":"health"}' }),
    );
    await openExternal("https://example.com/");
    expect(open).toHaveBeenCalledWith("https://example.com/", "_blank", "noopener,noreferrer");
  });

  it("uses only the desktop external-link API and rejects other schemes", async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("window", { oshwhubDesktop: { openExternal: open } });
    await openExternal("https://example.com/");
    expect(open).toHaveBeenCalledWith("https://example.com/");
    await expect(openExternal("file:///tmp/private")).rejects.toMatchObject({
      code: "invalid_url",
    });
    expect(open).toHaveBeenCalledTimes(1);
  });
});

import { describe, expect, it } from "vitest";
import { BridgeError, decode } from "./protocol";

describe("Rust bridge envelope", () => {
  it("returns a successful core payload without transforming business fields", () => {
    expect(decode('{"ok":true,"data":{"protocol":1},"requestId":"a"}')).toEqual({ protocol: 1 });
  });
  it("preserves actionable core error code and correlation ID", () => {
    try {
      decode({
        ok: false,
        error: { code: "rate_limited", message: "请稍后重试", requestId: "req-7" },
      });
      throw new Error("expected failure");
    } catch (error) {
      expect(error).toBeInstanceOf(BridgeError);
      expect(error).toMatchObject({
        code: "rate_limited",
        requestId: "req-7",
        message: "请稍后重试",
      });
    }
  });
  it.each([
    "{",
    { ok: true },
    null,
    { ok: false, error: {} },
  ])("rejects malformed native responses", (value) => {
    expect(() => decode(value)).toThrow(BridgeError);
  });
});

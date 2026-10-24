import { describe, expect, it } from "vitest";
import { parseAuthCallback, passportLoginUrl } from "./auth";

describe("OSHWHub Passport callback contract", () => {
  it("builds the official Passport URL with the fixed appId", () => {
    const url = new URL(passportLoginUrl());
    expect(url.origin + url.pathname).toBe("https://passport.jlc.com/login");
    expect(url.searchParams.get("appId")).toBe("JLC_OSHWHUB");
    expect(url.searchParams.get("redirectUrl")).toBe("oshwhub://auth/callback");
    expect(url.searchParams.get("backCode")).toBe("1");
  });

  it("returns only a safe callback code", () => {
    expect(parseAuthCallback("oshwhub://auth/callback?code=abc-123_A")).toBe("abc-123_A");
    for (const url of [
      "https://auth/callback?code=abc",
      "oshwhub://evil/callback?code=abc",
      "oshwhub://user@auth/callback?code=abc",
      "oshwhub://auth/wrong?code=abc",
      "oshwhub://auth/callback?code=abc%20123",
      "oshwhub://auth/callback?code=abc&code=def",
      "oshwhub://auth/callback",
      `oshwhub://auth/callback?code=${"a".repeat(513)}`,
    ]) {
      expect(parseAuthCallback(url)).toBeNull();
    }
  });
});

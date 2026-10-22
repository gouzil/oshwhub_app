import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { androidTools, targetPlan } from "./build-rust.mjs";

describe("native Rust build targets", () => {
  it("includes Android phone and emulator ABIs with matching Rust triples", () => {
    assert.deepEqual(targetPlan("android"), [
      { triple: "aarch64-linux-android", abi: "arm64-v8a" },
      { triple: "x86_64-linux-android", abi: "x86_64" },
    ]);
  });
  it("keeps physical iOS and simulator libraries separate", () => {
    assert.deepEqual(
      targetPlan("ios").map((item) => item.triple),
      ["aarch64-apple-ios", "aarch64-apple-ios-sim", "x86_64-apple-ios"],
    );
  });
  it("rejects unknown platform rather than executing an arbitrary target", () => {
    assert.throws(() => targetPlan("../../bad"), /Unsupported platform/);
  });
  it("selects each NDK host and Windows compiler wrappers", () => {
    assert.match(
      androidTools("/ndk", "aarch64-linux-android", "darwin").cc,
      /darwin-x86_64\/bin\/aarch64-linux-android24-clang$/,
    );
    assert.match(
      androidTools("/ndk", "x86_64-linux-android", "linux").cc,
      /linux-x86_64\/bin\/x86_64-linux-android24-clang$/,
    );
    const windows = androidTools("/ndk", "aarch64-linux-android", "win32");
    assert.match(windows.cc, /windows-x86_64\/bin\/aarch64-linux-android24-clang.cmd$/);
    assert.match(windows.ar, /windows-x86_64\/bin\/llvm-ar.exe$/);
    assert.throws(
      () => androidTools("/ndk", "aarch64-linux-android", "freebsd"),
      /Unsupported NDK host/,
    );
  });
});

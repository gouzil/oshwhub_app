import assert from "node:assert/strict";
import { describe, it } from "node:test";
import withAndroidArchitectures from "../apps/mobile/plugins/withAndroidArchitectures.cjs";

async function applyMod(name, modResults, applications = 1) {
  let config = { name: "OSHWHub", slug: "oshwhub" };
  for (let i = 0; i < applications; i++) config = withAndroidArchitectures(config);
  assert.equal(typeof config.mods.android[name], "function");
  return (await config.mods.android[name]({ ...config, modRequest: {}, modResults })).modResults;
}

describe("Android workspace config plugin", () => {
  it("keeps the supported ABIs once and preserves other Gradle properties", async () => {
    const result = await applyMod(
      "gradleProperties",
      [
        { type: "property", key: "reactNativeArchitectures", value: "armeabi-v7a" },
        { type: "property", key: "hermesEnabled", value: "true" },
      ],
      2,
    );
    assert.deepEqual(result, [
      { type: "property", key: "hermesEnabled", value: "true" },
      { type: "property", key: "reactNativeArchitectures", value: "arm64-v8a,x86_64" },
    ]);
  });

  it("tracks shared bundle inputs without changing the Expo app root", async () => {
    const original =
      'def projectRoot = rootDir.getParentFile().getAbsolutePath()\nreact {\n    root = file("../..")\n}\n';
    const result = await applyMod("appBuildGradle", {
      language: "groovy",
      contents: original,
    });
    assert.ok(result.contents.startsWith(original));
    const injected = result.contents.slice(original.length);
    assert.match(injected, /startsWith\("createBundle"\)/);
    assert.match(injected, /endsWith\("JsAndAssets"\)/);
    assert.match(injected, /inputs\.files\(fileTree\(new File\(rootDir, "\.\.\/\.\.\/\.\."\)\)/);
    for (const directory of ["packages", "modules"]) {
      for (const extension of ["js", "jsx", "ts", "tsx", "cjs", "mjs", "json"]) {
        assert.ok(injected.includes(`"${directory}/**/*.${extension}"`));
      }
    }
    assert.ok(injected.includes('"assets/**"'));
    for (const excluded of ["node_modules", "build", ".gradle"]) {
      assert.ok(injected.includes(`"**/${excluded}/**"`));
    }
    assert.match(
      injected,
      /withPathSensitivity\(org\.gradle\.api\.tasks\.PathSensitivity\.RELATIVE\)/,
    );
    assert.doesNotMatch(injected, /\breact\s*\{|\broot\s*=/);
  });

  it("does not duplicate bundle inputs across repeated plugin applications", async () => {
    const first = await applyMod("appBuildGradle", {
      language: "groovy",
      contents: "android { }\n",
    });
    const repeated = await applyMod("appBuildGradle", first, 2);
    assert.equal(repeated.contents, first.contents);
    assert.equal(repeated.contents.match(/oshwhub-shared-bundle-inputs/g).length, 2);
  });
});

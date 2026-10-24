const path = require("node:path");
const fs = require("node:fs/promises");
const { createHash } = require("node:crypto");

const binary = `oshwhub-desktop-core${process.platform === "win32" ? ".exe" : ""}`;
const coreDirectory = path.join(__dirname, "electron/bin", `${process.platform}-${process.arch}`);
module.exports = {
  appId: "com.gouzil.oshwhub",
  productName: "OSHWHub",
  asar: true,
  directories: { output: "release" },
  files: [
    "dist/**/*",
    "electron/*.mjs",
    "electron/preload.cjs",
    "package.json",
    "!node_modules/**/*",
  ],
  extraResources: [{ from: coreDirectory, to: "core", filter: [binary] }],
  mac: {
    target: ["dmg", "zip"],
    category: "public.app-category.developer-tools",
    icon: "electron/icons/icon.icns",
    identity: null,
  },
  win: { target: ["nsis"], icon: "electron/icons/icon.ico" },
  linux: {
    executableName: "oshwhub",
    target: ["AppImage", "deb"],
    category: "Development",
    icon: "electron/icons/icon.png",
    maintainer: "OSHWHub contributors",
  },
  // biome-ignore lint/suspicious/noTemplateCurlyInString: electron-builder expands these macros.
  deb: { artifactName: "oshwhub_${version}_${arch}.${ext}" },
  beforePack: async (context) => {
    const arch = { 1: "x64", 3: "arm64" }[context.arch];
    if (context.electronPlatformName !== process.platform || arch !== process.arch) {
      throw new Error("Build desktop packages on the target OS and architecture");
    }
    const { hostTarget } = await import("../../scripts/desktop-build.mjs");
    const metadata = JSON.parse(await fs.readFile(path.join(coreDirectory, "target.json"), "utf8"));
    const hash = createHash("sha256")
      .update(await fs.readFile(path.join(coreDirectory, binary)))
      .digest("hex");
    if (
      metadata.platform !== process.platform ||
      metadata.arch !== arch ||
      metadata.target !== hostTarget() ||
      metadata.sha256 !== hash
    ) {
      throw new Error(
        "Rust sidecar target metadata does not match this desktop package; rebuild it",
      );
    }
  },
};

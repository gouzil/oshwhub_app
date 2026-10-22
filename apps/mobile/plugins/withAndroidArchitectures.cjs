const { withAppBuildGradle, withGradleProperties } = require("expo/config-plugins");

const sharedBundleInputs = `// @generated begin oshwhub-shared-bundle-inputs
tasks.matching { it.name.startsWith("createBundle") && it.name.endsWith("JsAndAssets") }.configureEach {
    inputs.files(fileTree(new File(rootDir, "../../..")) {
        include "packages/**/*.js", "packages/**/*.jsx", "packages/**/*.ts", "packages/**/*.tsx"
        include "packages/**/*.cjs", "packages/**/*.mjs", "packages/**/*.json"
        include "modules/**/*.js", "modules/**/*.jsx", "modules/**/*.ts", "modules/**/*.tsx"
        include "modules/**/*.cjs", "modules/**/*.mjs", "modules/**/*.json"
        include "assets/**"
        exclude "**/node_modules/**", "**/build/**", "**/.gradle/**"
    }).withPropertyName("oshwhubSharedBundleInputs").withPathSensitivity(org.gradle.api.tasks.PathSensitivity.RELATIVE)
}
// @generated end oshwhub-shared-bundle-inputs`;

module.exports = (config) => {
  config = withGradleProperties(config, (mod) => {
    mod.modResults = mod.modResults.filter((item) => item.key !== "reactNativeArchitectures");
    mod.modResults.push({
      type: "property",
      key: "reactNativeArchitectures",
      value: "arm64-v8a,x86_64",
    });
    return mod;
  });
  return withAppBuildGradle(config, (mod) => {
    if (mod.modResults.language !== "groovy") {
      throw new Error("OSHWHub Android bundle inputs require a Groovy app/build.gradle");
    }
    const contents = mod.modResults.contents.replace(
      /(?:\r?\n)?\/\/ @generated begin oshwhub-shared-bundle-inputs[\s\S]*?\/\/ @generated end oshwhub-shared-bundle-inputs(?:\r?\n)?/g,
      "",
    );
    mod.modResults.contents = `${contents.trimEnd()}\n\n${sharedBundleInputs}\n`;
    return mod;
  });
};

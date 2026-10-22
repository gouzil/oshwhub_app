const { withGradleProperties } = require("expo/config-plugins");

module.exports = (config) =>
  withGradleProperties(config, (mod) => {
    mod.modResults = mod.modResults.filter((item) => item.key !== "reactNativeArchitectures");
    mod.modResults.push({
      type: "property",
      key: "reactNativeArchitectures",
      value: "arm64-v8a,x86_64",
    });
    return mod;
  });

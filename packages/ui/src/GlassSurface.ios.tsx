import { GlassView, isGlassEffectAPIAvailable, isLiquidGlassAvailable } from "expo-glass-effect";
import { useEffect, useState } from "react";
import { AccessibilityInfo, View, type ViewProps } from "react-native";

const available = isLiquidGlassAvailable() && isGlassEffectAPIAvailable();

export function GlassSurface({ style, ...props }: ViewProps) {
  const [reduceTransparency, setReduceTransparency] = useState(true);
  useEffect(() => {
    if (!available) return;
    let active = true;
    void AccessibilityInfo.isReduceTransparencyEnabled().then(
      (reduce) => {
        if (active) setReduceTransparency(reduce);
      },
      () => {},
    );
    const subscription = AccessibilityInfo.addEventListener(
      "reduceTransparencyChanged",
      setReduceTransparency,
    );
    return () => {
      active = false;
      subscription.remove();
    };
  }, []);
  if (!available || reduceTransparency) {
    return (
      <View {...props} testID="navigation-glass" style={[style, { backgroundColor: "#f6f9f7" }]} />
    );
  }
  return (
    <GlassView
      {...props}
      testID="navigation-glass"
      style={style}
      glassEffectStyle="clear"
      colorScheme="light"
    />
  );
}

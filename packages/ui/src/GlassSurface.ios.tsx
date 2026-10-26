import {
  isGlassEffectAPIAvailable,
  isLiquidGlassAvailable,
  GlassView as NativeGlassView,
} from "expo-glass-effect";
import { useEffect, useState } from "react";
import { AccessibilityInfo, View, type ViewProps } from "react-native";
import { useUniwind, withUniwind } from "uniwind";
import { cn } from "./lib/utils";

const available = isLiquidGlassAvailable() && isGlassEffectAPIAvailable();
const GlassView = withUniwind(NativeGlassView);

export function GlassSurface({ className, ...props }: ViewProps) {
  const { theme } = useUniwind();
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
    return <View {...props} testID="navigation-glass" className={cn(className, "bg-surface")} />;
  }
  return (
    <GlassView
      {...props}
      testID="navigation-glass"
      className={className}
      glassEffectStyle="clear"
      colorScheme={theme}
    />
  );
}

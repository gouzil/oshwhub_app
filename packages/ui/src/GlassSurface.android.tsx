import { AndroidGlassView as NativeAndroidGlassView } from "expo-android-glass-view";
import { View, type ViewProps } from "react-native";
import { useCSSVariable, useUniwind, withUniwind } from "uniwind";
import { cn } from "./lib/utils";
import { useReducedMotion } from "./useReducedMotion";

const AndroidGlassView = withUniwind(NativeAndroidGlassView);

export function GlassSurface({ className, ...props }: ViewProps) {
  const { theme } = useUniwind();
  const surface = useCSSVariable("--color-surface") as string;
  const reducedMotion = useReducedMotion();
  if (reducedMotion) {
    return <View {...props} testID="navigation-glass" className={cn(className, "bg-surface")} />;
  }
  return (
    <AndroidGlassView
      {...props}
      testID="navigation-glass"
      className={className}
      blurRadius={12}
      refractionHeight={10}
      refractionAmount={18}
      surfaceColor={theme === "dark" ? "#151e2ccc" : "#ffffff45"}
      fallbackColor={surface}
      shadow={false}
    />
  );
}

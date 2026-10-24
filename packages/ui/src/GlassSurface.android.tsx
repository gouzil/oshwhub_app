import { AndroidGlassView as NativeAndroidGlassView } from "expo-android-glass-view";
import { View, type ViewProps } from "react-native";
import { withUniwind } from "uniwind";
import { cn } from "./lib/utils";
import { useReducedMotion } from "./useReducedMotion";

const AndroidGlassView = withUniwind(NativeAndroidGlassView);

export function GlassSurface({ className, ...props }: ViewProps) {
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
      surfaceColor="#ffffff45"
      fallbackColor="#f6f9f7"
      shadow={false}
    />
  );
}

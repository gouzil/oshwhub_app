import { AndroidGlassView } from "expo-android-glass-view";
import { View, type ViewProps } from "react-native";
import { useReducedMotion } from "./useReducedMotion";

export function GlassSurface({ style, ...props }: ViewProps) {
  const reducedMotion = useReducedMotion();
  if (reducedMotion) {
    return (
      <View {...props} testID="navigation-glass" style={[style, { backgroundColor: "#f6f9f7" }]} />
    );
  }
  return (
    <AndroidGlassView
      {...props}
      testID="navigation-glass"
      style={style}
      blurRadius={12}
      refractionHeight={10}
      refractionAmount={18}
      surfaceColor="#ffffff45"
      fallbackColor="#f6f9f7"
      shadow={false}
    />
  );
}

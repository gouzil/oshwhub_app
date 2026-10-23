import { Platform, View, type ViewProps } from "react-native";

export function GlassSurface({ style, ...props }: ViewProps) {
  return (
    <View
      {...props}
      nativeID="navigation-glass"
      testID="navigation-glass"
      style={[Platform.OS !== "web" && { backgroundColor: "#f6f9f7" }, style]}
    />
  );
}

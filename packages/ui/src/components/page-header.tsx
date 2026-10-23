import { Pressable, StyleSheet, Text, View } from "react-native";
import { color } from "../theme";

export function PageHeader({ title, onBack }: { title: string; onBack: () => void }) {
  return (
    <View style={common.pageHeader}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="返回上一页"
        onPress={onBack}
        style={({ pressed }) => [common.backButton, pressed && common.pressed]}
      >
        <Text style={common.backArrow}>‹</Text>
        <Text style={common.backText}>返回</Text>
      </Pressable>
      <Text numberOfLines={1} accessibilityRole="header" style={common.pageHeaderTitle}>
        {title}
      </Text>
    </View>
  );
}

const common = StyleSheet.create({
  pageHeader: {
    minHeight: 62,
    paddingHorizontal: 12,
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    borderBottomWidth: 1,
    borderBottomColor: color.line,
  },
  backButton: {
    minHeight: 44,
    minWidth: 64,
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
    paddingRight: 8,
    justifyContent: "center",
    borderRadius: 10,
  },
  backArrow: { fontSize: 34, lineHeight: 38, color: color.green },
  backText: { fontSize: 13, color: color.green },
  pageHeaderTitle: { flex: 1, minWidth: 0, fontSize: 15, fontWeight: "600", color: color.ink },
  pressed: { opacity: 0.72 },
});

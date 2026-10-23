import { ActivityIndicator, StyleSheet, Text, View } from "react-native";
import { color } from "../theme";
import { ActionButton as Button } from "./action-button";
import { BoardMark } from "./board-mark";

export function ErrorPanel({ error, retry }: { error: Error; retry: () => void }) {
  const details = error as Error & { code?: string; requestId?: string };
  return (
    <View style={common.errorPanel} accessibilityRole="alert">
      <Text style={common.stateTitle}>暂时无法加载</Text>
      <Text selectable style={common.stateBody}>
        {error.message}
      </Text>
      {(details.code || details.requestId) && (
        <Text selectable style={common.errorCode}>
          {[details.code, details.requestId].filter(Boolean).join(" · ")}
        </Text>
      )}
      <Button label="重新加载" secondary onPress={retry} />
    </View>
  );
}

export function Loading({ label = "正在加载" }: { label?: string }) {
  return (
    <View style={common.state}>
      <ActivityIndicator color={color.green} accessibilityLabel={label} />
      <Text style={common.stateBody}>{label}</Text>
    </View>
  );
}

export function Empty({ title, detail }: { title: string; detail: string }) {
  return (
    <View style={common.state}>
      <BoardMark small />
      <Text style={common.stateTitle}>{title}</Text>
      <Text style={common.stateBody}>{detail}</Text>
    </View>
  );
}

const common = StyleSheet.create({
  state: {
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: 24,
    paddingVertical: 48,
    gap: 14,
    flexGrow: 1,
  },
  stateTitle: { fontSize: 18, fontWeight: "600", color: color.ink, textAlign: "center" },
  stateBody: { fontSize: 14, color: color.muted, lineHeight: 23, textAlign: "center" },
  errorPanel: {
    borderWidth: 1,
    borderColor: "#efe2dc",
    backgroundColor: "#fffaf7",
    borderRadius: 16,
    padding: 24,
    marginVertical: 20,
    gap: 14,
    alignItems: "center",
  },
  errorCode: { color: "#946a58", fontSize: 11, lineHeight: 18, textAlign: "center" },
});

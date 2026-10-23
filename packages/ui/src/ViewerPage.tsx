import type { Scene } from "@oshwhub/bridge";
import { PcbViewer } from "@oshwhub/pcb-viewer";
import { Component, type ReactNode, useMemo } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { color, ErrorPanel, Loading, useCommand } from "./common";

export type ViewerRoute =
  | { demo: true }
  | { demo: false; projectId: string; documentId: string; title: string };

export function ViewerPage({ route, onBack }: { route: ViewerRoute; onBack: () => void }) {
  const command = useMemo(
    () =>
      route.demo
        ? { command: "demo" as const }
        : { command: "pcb" as const, projectId: route.projectId, documentId: route.documentId },
    [route],
  );
  const { value: scene, loading, error, retry } = useCommand<Scene>(command);
  return (
    <View style={styles.root}>
      <View style={styles.header}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="返回上一页"
          onPress={onBack}
          style={({ pressed }) => [styles.back, pressed && styles.pressed]}
        >
          <Text style={styles.backText}>返回</Text>
        </Pressable>
        <View style={styles.document}>
          <Text accessibilityRole="header" numberOfLines={1} style={styles.title}>
            {scene?.name ?? (route.demo ? "PCB 示例板" : route.title || "PCB 查看器")}
          </Text>
          <Text numberOfLines={1} style={styles.subtitle}>
            {route.demo ? "本地自制示例" : route.title || "PCB 工作区"}
          </Text>
        </View>
        <Text style={styles.badge}>2D · 只读</Text>
      </View>
      {loading ? (
        <Loading label={route.demo ? "正在准备本地示例" : "正在下载并解析 PCB"} />
      ) : error ? (
        <ScrollView contentContainerStyle={styles.error}>
          <ErrorPanel error={error} retry={retry} />
        </ScrollView>
      ) : (
        scene && (
          <ViewerBoundary key={`${scene.id}-${scene.revision}`} retry={retry}>
            <PcbViewer scene={scene} />
          </ViewerBoundary>
        )
      )}
    </View>
  );
}

class ViewerBoundary extends Component<
  { children: ReactNode; retry: () => void },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    return this.state.error ? (
      <ScrollView contentContainerStyle={styles.error}>
        <ErrorPanel
          error={this.state.error}
          retry={() => {
            this.setState({ error: null });
            this.props.retry();
          }}
        />
      </ScrollView>
    ) : (
      this.props.children
    );
  }
}

const styles = StyleSheet.create({
  root: { flex: 1, minHeight: 0, backgroundColor: "#0b121a" },
  header: {
    flexDirection: "row",
    alignItems: "center",
    minHeight: 60,
    paddingHorizontal: 12,
    paddingVertical: 6,
    gap: 12,
    backgroundColor: "#1b242f",
    borderBottomWidth: 1,
    borderBottomColor: "#ffffff14",
  },
  back: {
    minWidth: 44,
    minHeight: 44,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 10,
  },
  pressed: { backgroundColor: "#ffffff14" },
  backText: { color: "#b7d5f8", fontSize: 13, fontWeight: "500" },
  document: { flex: 1, minWidth: 0, gap: 3 },
  title: { color: "#edf2f7", fontSize: 14, fontWeight: "600" },
  subtitle: { color: "#9aaaba", fontSize: 10 },
  badge: {
    color: "#b2c0ce",
    fontSize: 10,
    backgroundColor: "#ffffff0a",
    paddingHorizontal: 10,
    paddingVertical: 7,
    borderRadius: 7,
  },
  error: { padding: 20, flexGrow: 1, justifyContent: "center", backgroundColor: color.canvas },
});

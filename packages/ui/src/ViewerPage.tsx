import type { Scene } from "@oshwhub/bridge";
import { PcbViewer } from "@oshwhub/pcb-viewer";
import { Component, type ReactNode, useMemo } from "react";
import { ScrollView, StyleSheet, Text, View } from "react-native";
import { color, ErrorPanel, Loading, PageHeader, useCommand } from "./common";

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
      <PageHeader
        title={scene?.name ?? (route.demo ? "PCB 示例板" : route.title || "PCB 查看器")}
        onBack={onBack}
      />
      {route.demo && (
        <View style={styles.demoNotice}>
          <Text style={styles.demoText}>本地自制示例 · 用于体验查看器交互</Text>
        </View>
      )}
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
  root: { flex: 1, minHeight: 0 },
  demoNotice: {
    paddingHorizontal: 16,
    paddingVertical: 9,
    backgroundColor: "#f2f7e8",
    borderBottomWidth: 1,
    borderBottomColor: "#e6eddb",
  },
  demoText: { color: "#718547", fontSize: 11, lineHeight: 18 },
  error: { padding: 20, flexGrow: 1, justifyContent: "center", backgroundColor: color.canvas },
});

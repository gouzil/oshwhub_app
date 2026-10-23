import type { Scene } from "@oshwhub/bridge";
import { Pcb3dViewer, PcbViewer } from "@oshwhub/pcb-viewer";
import { useMemo, useState } from "react";
import { Platform, Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { ErrorPanel, Loading } from "../components/states";
import { TabBar } from "../components/tab-bar";
import { ViewerBoundary } from "../components/viewer-boundary";
import { useCommand } from "../hooks/use-command";
import { color } from "../theme";

export type ViewerRoute =
  | { demo: true }
  | { demo: false; projectId: string; documentId: string; title: string };

export function ViewerPage({ route, onBack }: { route: ViewerRoute; onBack: () => void }) {
  const [mode, setMode] = useState<"2d" | "3d">("2d");
  const [three, setThree] = useState(false);
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
            {route.demo ? "本地自制示例 · 只读" : `${route.title || "PCB 工作区"} · 只读`}
          </Text>
        </View>
        <TabBar
          items={[
            { value: "2d", label: "2D", accessibilityLabel: "二维模式" },
            { value: "3d", label: "3D", accessibilityLabel: "三维模式" },
          ]}
          value={mode}
          onValueChange={(next) => {
            setMode(next);
            if (next === "3d") setThree(true);
          }}
          disabled={loading || !scene}
          accessibilityLabel="PCB 查看模式"
          variant="dark"
          className="shrink-0 gap-1 rounded-xl bg-[#14221e] p-1"
        />
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
            <View style={styles.workspace}>
              <View
                ref={(node) => {
                  if (Platform.OS === "web" && node)
                    (node as unknown as HTMLElement).inert = mode !== "2d";
                }}
                style={[StyleSheet.absoluteFill, mode !== "2d" && styles.hidden]}
                pointerEvents={mode === "2d" ? "auto" : "none"}
                aria-hidden={mode !== "2d"}
                accessibilityElementsHidden={mode !== "2d"}
                importantForAccessibility={mode === "2d" ? "auto" : "no-hide-descendants"}
              >
                <PcbViewer scene={scene} />
              </View>
              {three && (
                <View
                  ref={(node) => {
                    if (Platform.OS === "web" && node)
                      (node as unknown as HTMLElement).inert = mode !== "3d";
                  }}
                  style={[StyleSheet.absoluteFill, mode !== "3d" && styles.hidden]}
                  pointerEvents={mode === "3d" ? "auto" : "none"}
                  aria-hidden={mode !== "3d"}
                  accessibilityElementsHidden={mode !== "3d"}
                  importantForAccessibility={mode === "3d" ? "auto" : "no-hide-descendants"}
                >
                  <Pcb3dViewer sceneId={scene.id} name={scene.name} active={mode === "3d"} />
                </View>
              )}
            </View>
          </ViewerBoundary>
        )
      )}
    </View>
  );
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
  workspace: { flex: 1, minHeight: 0 },
  hidden: { opacity: 0 },
  error: { padding: 20, flexGrow: 1, justifyContent: "center", backgroundColor: color.canvas },
});

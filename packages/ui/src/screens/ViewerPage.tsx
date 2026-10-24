import type { Scene } from "@oshwhub/bridge";
import { Pcb3dViewer, PcbViewer } from "@oshwhub/pcb-viewer";
import { useMemo, useState } from "react";
import { Platform, Pressable, ScrollView, View } from "react-native";
import { ErrorPanel, Loading } from "../components/states";
import { TabBar } from "../components/tab-bar";
import { Text } from "../components/ui/text";
import { ViewerBoundary } from "../components/viewer-boundary";
import { useCommand } from "../hooks/use-command";
import { cn } from "../lib/utils";

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
    <View className="min-h-0 flex-1 bg-viewer">
      <View className="min-h-[60px] flex-row items-center gap-3 border-b border-[#ffffff14] bg-viewer-panel px-3 py-1.5">
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="返回上一页"
          onPress={onBack}
          className="min-h-11 min-w-11 items-center justify-center rounded-[10px] active:bg-[#ffffff14]"
        >
          <Text className="text-[13px] font-medium text-[#b7d5f8]">返回</Text>
        </Pressable>
        <View className="min-w-0 flex-1 gap-[3px]">
          <Text
            accessibilityRole="header"
            numberOfLines={1}
            className="text-[14px] font-semibold text-[#edf2f7]"
          >
            {scene?.name ?? (route.demo ? "PCB 示例板" : route.title || "PCB 查看器")}
          </Text>
          <Text numberOfLines={1} className="text-[10px] text-[#9aaaba]">
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
        <ScrollView contentContainerClassName="grow justify-center bg-canvas p-5">
          <ErrorPanel error={error} retry={retry} />
        </ScrollView>
      ) : (
        scene && (
          <ViewerBoundary key={`${scene.id}-${scene.revision}`} retry={retry}>
            <View className="min-h-0 flex-1">
              <View
                ref={(node) => {
                  if (Platform.OS === "web" && node)
                    (node as unknown as HTMLElement).inert = mode !== "2d";
                }}
                className={cn("absolute inset-0", mode !== "2d" && "opacity-0")}
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
                  className={cn("absolute inset-0", mode !== "3d" && "opacity-0")}
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

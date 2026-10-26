import type { Scene } from "@oshwhub/bridge";
import { Pcb3dViewer, PcbViewer, SchematicViewer } from "@oshwhub/pcb-viewer";
import { useEffect, useMemo, useState } from "react";
import { Platform, Pressable, ScrollView, View } from "react-native";
import { ErrorPanel, Loading } from "../components/states";
import { TabBar } from "../components/tab-bar";
import { Text } from "../components/ui/text";
import { ViewerBoundary } from "../components/viewer-boundary";
import { useCommand } from "../hooks/use-command";
import { cn } from "../lib/utils";

export type ViewerMode = "pcb2d" | "schematic" | "3d";
export type ViewerRoute =
  | { demo: true }
  | {
      demo: false;
      projectId: string;
      pcbDocumentId: string | null;
      schematicDocumentId: string | null;
      title: string;
      initialMode?: "pcb2d" | "schematic";
    };

const DEMO_COMMAND = { command: "demo" as const };

export function ViewerPage({ route, onBack }: { route: ViewerRoute; onBack: () => void }) {
  const [mode, setMode] = useState<ViewerMode>(() => {
    if (route.demo) return "pcb2d";
    if (!route.pcbDocumentId) return "schematic";
    return route.initialMode === "schematic" ? "schematic" : "pcb2d";
  });
  const [three, setThree] = useState(false);
  useEffect(() => {
    if (
      !route.demo &&
      route.schematicDocumentId &&
      (!route.pcbDocumentId || route.initialMode === "schematic")
    ) {
      setMode("schematic");
    } else {
      setMode("pcb2d");
    }
  }, [route]);
  const schematicMode = mode === "schematic";
  const command = useMemo(() => {
    if (route.demo) return DEMO_COMMAND;
    if (schematicMode && route.schematicDocumentId) {
      return {
        command: "schematic" as const,
        projectId: route.projectId,
        documentId: route.schematicDocumentId,
      };
    }
    return {
      command: "pcb" as const,
      projectId: route.projectId,
      documentId: route.pcbDocumentId ?? route.schematicDocumentId ?? "",
    };
  }, [schematicMode, route]);
  const { value: scene, loading, error, retry } = useCommand<Scene>(command);
  const items = useMemo(() => {
    const values: { value: ViewerMode; label: string; accessibilityLabel: string }[] = [];
    if (route.demo || route.pcbDocumentId) {
      values.push({ value: "pcb2d", label: "2D", accessibilityLabel: "二维模式" });
    }
    if (!route.demo && route.schematicDocumentId) {
      values.push({ value: "schematic", label: "原理图", accessibilityLabel: "原理图模式" });
    }
    if (route.demo || route.pcbDocumentId) {
      values.push({ value: "3d", label: "3D", accessibilityLabel: "三维模式" });
    }
    return values;
  }, [route]);
  const label = mode === "schematic" ? "原理图" : mode === "3d" ? "PCB 三维" : "PCB";
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
            {scene?.name ?? (route.demo ? "PCB 示例板" : `${route.title || label} 查看器`)}
          </Text>
          <Text numberOfLines={1} className="text-[10px] text-[#9aaaba]">
            {route.demo ? "本地自制示例 · 只读" : `${route.title || label} · 只读`}
          </Text>
        </View>
        <TabBar
          items={items}
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
        <Loading label={route.demo ? "正在准备本地示例" : `正在下载并解析 ${label}`} />
      ) : error ? (
        <ScrollView contentContainerClassName="grow justify-center bg-canvas p-5">
          <ErrorPanel error={error} retry={retry} />
        </ScrollView>
      ) : (
        scene && (
          <ViewerBoundary key={`${scene.id}-${scene.revision}`} retry={retry}>
            <View className="min-h-0 flex-1">
              {mode !== "schematic" && (
                <View
                  ref={(node) => {
                    if (Platform.OS === "web" && node)
                      (node as unknown as HTMLElement).inert = mode !== "pcb2d";
                  }}
                  className={cn("absolute inset-0", mode !== "pcb2d" && "opacity-0")}
                  pointerEvents={mode === "pcb2d" ? "auto" : "none"}
                  aria-hidden={mode !== "pcb2d"}
                  accessibilityElementsHidden={mode !== "pcb2d"}
                  importantForAccessibility={mode === "pcb2d" ? "auto" : "no-hide-descendants"}
                >
                  <PcbViewer scene={scene} />
                </View>
              )}
              {mode === "schematic" && (
                <View className="absolute inset-0">
                  <SchematicViewer scene={scene} />
                </View>
              )}
              {three && mode !== "schematic" && (
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

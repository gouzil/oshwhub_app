import { call, type Model3d, type Scene3d } from "@oshwhub/bridge";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ActivityIndicator,
  type GestureResponderEvent,
  PanResponder,
  PixelRatio,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
  type ViewInstance,
} from "react-native";
import { type Camera3d, fitCamera, orbitCamera, panCamera, zoomCamera } from "./camera3d";
import { GlSurface } from "./GlSurface";
import { identity, MAX_GPU_BYTES, Renderer3d } from "./gl3d";

type Point = { x: number; y: number };
type Progress = { loading: boolean; loaded: number; done: number; failed: number };
const initialProgress: Progress = { loading: false, loaded: 0, done: 0, failed: 0 };
const touches = (event: GestureResponderEvent): Point[] =>
  Array.from(event.nativeEvent.touches).map((t) => ({ x: t.locationX, y: t.locationY }));
const middle = (p: Point[]) =>
  p.length > 1 ? { x: (p[0].x + p[1].x) / 2, y: (p[0].y + p[1].y) / 2 } : p[0];
const distance = (p: Point[]) =>
  p.length > 1 ? Math.max(1, Math.hypot(p[0].x - p[1].x, p[0].y - p[1].y)) : 1;

export function Pcb3dViewer({
  sceneId,
  name,
  active,
}: {
  sceneId: string;
  name: string;
  active: boolean;
}) {
  const [attempt, setAttempt] = useState(0);
  const [source, setSource] = useState<{ attempt: number; data: Scene3d } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [contextVersion, setContextVersion] = useState(0);
  const [progress, setProgress] = useState(initialProgress);
  const [boardReady, setBoardReady] = useState(false);
  const [pan, setPan] = useState(false);
  const [components, setComponents] = useState(true);
  const [info, setInfo] = useState(false);
  const renderer = useRef<Renderer3d | null>(null);
  const camera = useRef<Camera3d | null>(null);
  const frame = useRef<number | null>(null);
  const surface = useRef<ViewInstance>(null);
  const current = useRef({ active, components, size, pan });
  current.current = { active, components, size, pan };
  const retryModels = useRef<(() => void) | null>(null);
  const gesture = useRef<{ points: Point[]; camera: Camera3d; pan: boolean } | null>(null);
  const fail = useCallback((message: string) => {
    console.warn("pcb.3d.render_failed", { message });
    setError(message);
  }, []);
  const schedule = useCallback(() => {
    if (frame.current !== null || !current.current.active) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      if (!current.current.active || !renderer.current || !camera.current) return;
      try {
        const { width, height } = current.current.size;
        renderer.current.draw(
          camera.current,
          current.current.components,
          Platform.OS === "web"
            ? undefined
            : {
                width: Math.max(1, Math.round(width * PixelRatio.get())),
                height: Math.max(1, Math.round(height * PixelRatio.get())),
              },
        );
      } catch (error) {
        fail(error instanceof Error ? error.message : "三维绘制失败。");
      }
    });
  }, [fail]);
  const onContext = useCallback((gl: WebGLRenderingContext) => {
    const next = new Renderer3d(gl);
    renderer.current = next;
    setContextVersion((v) => v + 1);
    return () => {
      if (frame.current !== null) {
        cancelAnimationFrame(frame.current);
        frame.current = null;
      }
      next.dispose();
      if (renderer.current === next) renderer.current = null;
    };
  }, []);

  useEffect(() => {
    let alive = true;
    setSource(null);
    setError(null);
    setProgress(initialProgress);
    setBoardReady(false);
    call<Scene3d>({ command: "pcb3d", sceneId }).then(
      (data) => {
        if (alive) setSource({ attempt, data });
      },
      (error) => {
        if (alive) fail(error instanceof Error ? error.message : "三维场景暂时无法加载。");
      },
    );
    return () => {
      alive = false;
    };
  }, [sceneId, attempt, fail]);

  useEffect(() => {
    const drawing = renderer.current;
    if (!source || source.attempt !== attempt || !drawing || !contextVersion) return;
    let alive = true;
    const data = source.data;
    const failed = new Set<string>();
    let loaded = 0;
    try {
      drawing.add(data.board, [identity]);
      setBoardReady(true);
      camera.current = fitCamera(
        data.board.bounds,
        current.current.size.width / Math.max(1, current.current.size.height),
      );
      schedule();
    } catch (error) {
      fail(error instanceof Error ? error.message : "三维板体无法绘制。");
      return;
    }
    const load = async (keys: string[]) => {
      let index = 0,
        done = 0;
      setProgress({ loading: keys.length > 0, loaded, done: 0, failed: failed.size });
      const worker = async () => {
        while (alive && index < keys.length) {
          const key = keys[index++];
          try {
            if (drawing.bytes >= MAX_GPU_BYTES) throw new Error("三维缓存已达到上限。");
            const model = await call<Model3d>({ command: "model3d", sceneId, key });
            if (!alive) return;
            if (model.key !== key) throw new Error("器件模型与当前请求不匹配。");
            drawing.add(
              model.mesh,
              model.instances.map((i) => i.matrix),
              true,
              model.meshKey,
            );
            failed.delete(key);
            loaded += model.instances.length;
            schedule();
          } catch (error) {
            if (!alive) return;
            failed.add(key);
            console.warn("pcb.3d.model_failed", {
              key,
              message: error instanceof Error ? error.message : "模型加载失败",
            });
          }
          done++;
          if (alive)
            setProgress({ loading: done < keys.length, loaded, done, failed: failed.size });
        }
      };
      await Promise.all([worker(), worker(), worker()]);
      if (alive) {
        setProgress({ loading: false, loaded, done: keys.length, failed: failed.size });
        console.info("pcb.3d.ready", {
          sceneId,
          loaded,
          failed: failed.size,
          gpuBytes: drawing.bytes,
        });
      }
    };
    retryModels.current = () => void load([...failed]);
    void load(data.modelKeys);
    return () => {
      alive = false;
      retryModels.current = null;
    };
  }, [source, attempt, contextVersion, sceneId, fail, schedule]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Visible size and controls invalidate the demand-driven canvas.
  useEffect(() => {
    schedule();
  }, [active, components, size, schedule]);
  const update = useCallback(
    (next: Camera3d) => {
      camera.current = next;
      schedule();
    },
    [schedule],
  );
  const zoom = useCallback(
    (factor: number) => {
      if (camera.current) update(zoomCamera(camera.current, factor));
    },
    [update],
  );
  const reset = useCallback(() => {
    const bounds = renderer.current?.bounds;
    if (bounds)
      update(
        fitCamera(bounds, current.current.size.width / Math.max(1, current.current.size.height)),
      );
  }, [update]);
  const side = (bottom: boolean) => {
    if (camera.current)
      update({ ...camera.current, yaw: -Math.PI / 2, pitch: bottom ? -Math.PI / 2 : Math.PI / 2 });
  };
  const responder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => true,
        onMoveShouldSetPanResponder: () => true,
        onPanResponderGrant: (event) => {
          if (camera.current)
            gesture.current = {
              points: touches(event),
              camera: camera.current,
              pan: current.current.pan,
            };
        },
        onPanResponderMove: (event) => {
          const points = touches(event),
            start = gesture.current;
          if (!points.length || !start || !start.points.length || !camera.current) return;
          if (points.length !== start.points.length) {
            gesture.current = { points, camera: camera.current, pan: current.current.pan };
            return;
          }
          const a = middle(start.points),
            b = middle(points);
          let next = start.camera;
          if (points.length > 1) next = zoomCamera(next, distance(points) / distance(start.points));
          update(
            points.length > 1 || start.pan
              ? panCamera(next, b.x - a.x, b.y - a.y, current.current.size.height)
              : orbitCamera(next, b.x - a.x, b.y - a.y),
          );
        },
        onPanResponderRelease: () => {
          gesture.current = null;
        },
        onPanResponderTerminate: () => {
          gesture.current = null;
        },
        onPanResponderTerminationRequest: () => false,
      }),
    [update],
  );

  useEffect(() => {
    if (Platform.OS !== "web") return;
    const element = surface.current as unknown as HTMLElement | null;
    if (!element) return;
    const wheel = (event: WheelEvent) => {
      if (!current.current.active) return;
      event.preventDefault();
      zoom(Math.exp(-Math.max(-200, Math.min(200, event.deltaY)) * 0.003));
    };
    const keyboard = (event: KeyboardEvent) => {
      if (!current.current.active || !camera.current) return;
      if (event.key === "0") reset();
      else if (event.key === "+" || event.key === "=") zoom(1.25);
      else if (event.key === "-") zoom(0.8);
      else if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
        const dx = event.key === "ArrowLeft" ? -20 : event.key === "ArrowRight" ? 20 : 0;
        const dy = event.key === "ArrowUp" ? -20 : event.key === "ArrowDown" ? 20 : 0;
        update(
          event.shiftKey
            ? panCamera(camera.current, dx, dy, current.current.size.height)
            : orbitCamera(camera.current, dx, dy),
        );
      } else return;
      event.preventDefault();
    };
    const contextMenu = (event: Event) => event.preventDefault();
    element.addEventListener("wheel", wheel, { passive: false });
    element.addEventListener("keydown", keyboard);
    element.addEventListener("contextmenu", contextMenu);
    return () => {
      element.removeEventListener("wheel", wheel);
      element.removeEventListener("keydown", keyboard);
      element.removeEventListener("contextmenu", contextMenu);
    };
  }, [zoom, reset, update]);

  const data = source?.attempt === attempt ? source.data : null;
  const compact = size.height < 400;
  return (
    <View style={styles.root}>
      <View
        ref={surface}
        testID="pcb-3d-surface"
        collapsable={false}
        focusable
        accessible={active}
        accessibilityRole="image"
        accessibilityLabel={`${name}，三维 PCB 画布。单指旋转，双指平移缩放。方向键旋转，Shift 加方向键平移，加减号缩放，0 适应窗口。`}
        accessibilityActions={[
          { name: "zoomIn", label: "放大" },
          { name: "zoomOut", label: "缩小" },
          { name: "rotateLeft", label: "向左旋转" },
          { name: "rotateRight", label: "向右旋转" },
          { name: "reset", label: "适应窗口" },
        ]}
        onAccessibilityAction={({ nativeEvent }) => {
          if (nativeEvent.actionName === "zoomIn") zoom(1.25);
          else if (nativeEvent.actionName === "zoomOut") zoom(0.8);
          else if (
            nativeEvent.actionName === "rotateLeft" ||
            nativeEvent.actionName === "rotateRight"
          ) {
            if (camera.current)
              update(
                orbitCamera(camera.current, nativeEvent.actionName === "rotateLeft" ? -30 : 30, 0),
              );
          } else reset();
        }}
        style={styles.surface}
        onLayout={({ nativeEvent }) =>
          setSize({ width: nativeEvent.layout.width, height: nativeEvent.layout.height })
        }
        {...responder.panHandlers}
      >
        {size.width > 0 && size.height > 0 && (
          <GlSurface
            key={attempt}
            width={size.width}
            height={size.height}
            onContext={onContext}
            onError={fail}
          />
        )}
      </View>
      {!error && (
        <>
          <View style={styles.tools}>
            <View style={styles.group}>
              <Tool label="旋转三维视图" selected={!pan} onPress={() => setPan(false)}>
                旋转
              </Tool>
              <Tool label="平移三维视图" selected={pan} onPress={() => setPan(true)}>
                平移
              </Tool>
              <Tool label="适应三维窗口" onPress={reset}>
                适应
              </Tool>
            </View>
            <View style={styles.group}>
              <Tool label="查看 PCB 顶面" onPress={() => side(false)}>
                顶面
              </Tool>
              <Tool label="查看 PCB 底面" onPress={() => side(true)}>
                底面
              </Tool>
              <Tool
                label="显示器件模型"
                selected={components}
                onPress={() => setComponents((v) => !v)}
              >
                器件
              </Tool>
            </View>
          </View>
          <View style={styles.status} pointerEvents="none">
            {!data || !boardReady ? (
              <>
                <ActivityIndicator color="#9bc8ff" />
                <Text style={styles.caption}>正在构建三维板体</Text>
              </>
            ) : progress.loading ? (
              <>
                <ActivityIndicator size="small" color="#9bc8ff" />
                <Text style={styles.caption}>
                  加载器件 {progress.done} / {data.modelKeys.length}
                </Text>
              </>
            ) : (
              <Text style={styles.caption}>
                {progress.failed
                  ? `${progress.failed} 类模型加载失败，可在信息中重试`
                  : "3D 视图已就绪"}
              </Text>
            )}
            {data && (
              <Text style={styles.caption}>
                已显示 {progress.loaded} / {data.componentCount} 个器件
              </Text>
            )}
          </View>
          <View style={styles.information}>
            <Tool label="三维查看信息" selected={info} onPress={() => setInfo((v) => !v)}>
              信息{data?.warnings.length || progress.failed ? " ·" : ""}
            </Tool>
          </View>
          <View style={styles.zoom}>
            <Tool label="缩小三维 PCB" onPress={() => zoom(0.8)}>
              −
            </Tool>
            <Tool label="放大三维 PCB" onPress={() => zoom(1.25)}>
              +
            </Tool>
          </View>
          {info && (
            <View
              style={[
                styles.panel,
                { maxHeight: Math.max(100, size.height - (compact ? 90 : 160)) },
              ]}
            >
              <View style={styles.panelHeader}>
                <Text accessibilityRole="header" style={styles.title}>
                  三维预览
                </Text>
                <Tool label="关闭三维信息" onPress={() => setInfo(false)}>
                  关闭
                </Tool>
              </View>
              <ScrollView contentContainerStyle={styles.panelContent}>
                <Text style={styles.detail}>
                  拖动旋转 · 双指平移 / 缩放{compact ? "" : " · 滚轮缩放"}
                </Text>
                {data && (
                  <>
                    <Text style={styles.detail}>板厚：{data.thickness} mm</Text>
                    <Text style={styles.detail}>
                      器件模型：{progress.loaded} / {data.componentCount} 个已显示，
                      {data.boundCount} 个已绑定
                    </Text>
                    {data.componentCount > data.boundCount && (
                      <Text style={styles.detail}>未绑定模型的器件暂不显示实体。</Text>
                    )}
                    {data.warnings.map((warning) => (
                      <Text key={warning} style={styles.detail}>
                        {warning}
                      </Text>
                    ))}
                  </>
                )}
                {progress.failed > 0 && (
                  <>
                    <Text style={styles.detail}>
                      部分官方模型未能下载或超过缓存上限。板体仍可正常操作。
                    </Text>
                    <Tool
                      label="重试失败的器件模型"
                      disabled={progress.loading}
                      onPress={() => retryModels.current?.()}
                    >
                      重试器件
                    </Tool>
                  </>
                )}
              </ScrollView>
            </View>
          )}
        </>
      )}
      {error && (
        <View style={styles.errorPanel}>
          <Text accessibilityRole="alert" style={styles.error}>
            {error}
          </Text>
          <Tool label="重新载入三维视图" onPress={() => setAttempt((v) => v + 1)}>
            重新载入
          </Tool>
          <Text style={styles.caption}>也可以切回二维继续查看。</Text>
        </View>
      )}
    </View>
  );
}

function Tool({
  label,
  children,
  onPress,
  selected,
  disabled,
}: {
  label: string;
  children: React.ReactNode;
  onPress: () => void;
  selected?: boolean;
  disabled?: boolean;
}) {
  return (
    <Pressable
      accessibilityRole={selected === undefined ? "button" : "checkbox"}
      accessibilityLabel={label}
      accessibilityState={{ checked: selected, disabled }}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        styles.tool,
        selected && styles.selected,
        pressed && styles.pressed,
        disabled && styles.disabled,
      ]}
    >
      <Text style={styles.toolText}>{children}</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, minHeight: 0, backgroundColor: "#0b121a" },
  surface: { ...StyleSheet.absoluteFill, overflow: "hidden" },
  tools: {
    position: "absolute",
    top: 12,
    left: 12,
    right: 12,
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 8,
    justifyContent: "space-between",
  },
  group: {
    flexDirection: "row",
    backgroundColor: "#1b242ff2",
    borderColor: "#ffffff1a",
    borderWidth: 1,
    borderRadius: 12,
    padding: 3,
  },
  tool: {
    minWidth: 44,
    minHeight: 44,
    paddingHorizontal: 10,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 9,
  },
  toolText: { fontSize: 12, color: "#d7e4f3", fontWeight: "500" },
  selected: { backgroundColor: "#344d6b" },
  pressed: { backgroundColor: "#405570" },
  disabled: { opacity: 0.45 },
  status: {
    position: "absolute",
    bottom: 72,
    left: 16,
    right: 16,
    gap: 5,
    flexDirection: "row",
    flexWrap: "wrap",
    alignItems: "center",
  },
  caption: { color: "#a5b7ca", fontSize: 11 },
  information: {
    position: "absolute",
    left: 12,
    bottom: 12,
    backgroundColor: "#1b242ff2",
    borderWidth: 1,
    borderColor: "#ffffff1a",
    borderRadius: 12,
  },
  zoom: {
    position: "absolute",
    right: 12,
    bottom: 12,
    backgroundColor: "#1b242ff2",
    borderWidth: 1,
    borderColor: "#ffffff1a",
    borderRadius: 12,
    flexDirection: "row",
    padding: 3,
  },
  panel: {
    position: "absolute",
    left: 12,
    right: 12,
    bottom: 72,
    maxWidth: 360,
    backgroundColor: "#1b242ff5",
    borderWidth: 1,
    borderColor: "#ffffff20",
    borderRadius: 14,
    overflow: "hidden",
  },
  panelHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingLeft: 16,
    paddingRight: 4,
    borderBottomWidth: 1,
    borderBottomColor: "#ffffff12",
  },
  title: { color: "#e1ebf7", fontSize: 14, fontWeight: "600" },
  panelContent: { padding: 16, gap: 12 },
  detail: { color: "#b8c8da", fontSize: 12, lineHeight: 19 },
  errorPanel: {
    position: "absolute",
    left: 20,
    right: 20,
    top: "30%",
    padding: 20,
    alignItems: "center",
    gap: 12,
    backgroundColor: "#1b242ff2",
    borderRadius: 14,
  },
  error: { color: "#ffb8ab", fontSize: 14, lineHeight: 22 },
});

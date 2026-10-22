import { call, type PickResult, type Scene } from "@oshwhub/bridge";
import {
  Canvas,
  type DataSource,
  Group,
  Picture,
  Skia,
  useTypeface,
} from "@shopify/react-native-skia";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ActivityIndicator,
  type GestureResponderEvent,
  PanResponder,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  useWindowDimensions,
  View,
  type ViewInstance,
} from "react-native";
import { useDerivedValue, useSharedValue } from "react-native-reanimated";
import { defaultFontSource } from "./fontSource";
import { buildPictures, type LayerPicture } from "./pictures";
import {
  fitViewport,
  gestureViewport,
  isTap,
  type Point,
  type Viewport,
  worldPoint,
  zoomAt,
} from "./viewport";

type Props = {
  scene: Scene;
  onPick?: (result: PickResult | null) => void;
  fontSource?: DataSource;
};
type Gesture = {
  points: Point[];
  view: Viewport;
  first: Point;
  last: Point;
  pointers: number;
  moved: boolean;
};
const touchPoints = (event: GestureResponderEvent): Point[] =>
  Array.from(event.nativeEvent.touches).map((touch) => ({
    x: touch.locationX,
    y: touch.locationY,
  }));

export function PcbViewer({ scene, onPick, fontSource = defaultFontSource }: Props) {
  const compact = useWindowDimensions().height < 500;
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [compiled, setCompiled] = useState<{
    scene: Scene;
    pictures: LayerPicture[];
    warnings: string[];
  } | null>(null);
  const [renderError, setRenderError] = useState<string | null>(null);
  const [fontFailure, setFontFailure] = useState<{ source: DataSource; message: string } | null>(
    null,
  );
  const fontError = fontFailure?.source === fontSource ? fontFailure.message : null;
  const onFontError = useCallback(
    (error: Error) => setFontFailure({ source: fontSource, message: error.message }),
    [fontSource],
  );
  // CanvasKit font.getTypeface() exposes a raw pointer; retain the directly loaded Typeface.
  const typeface = useTypeface(scene.texts.length ? fontSource : null, onFontError) ?? undefined;
  const [visible, setVisible] = useState(
    () => new Set(scene.layers.filter((layer) => layer.visible).map((layer) => layer.id)),
  );
  const [selection, setSelection] = useState<PickResult | null>(null);
  const [pickError, setPickError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(100);
  const surface = useRef<ViewInstance>(null);
  const current = useRef<Viewport>({ scale: 1, x: 0, y: 0 });
  const fitted = useRef<Viewport>({ scale: 1, x: 0, y: 0 });
  const dimensions = useRef(size);
  dimensions.current = size;
  const visibleRef = useRef(visible);
  visibleRef.current = visible;
  const gesture = useRef<Gesture | null>(null);
  const pickVersion = useRef(0);
  const scale = useSharedValue(1);
  const offsetX = useSharedValue(0);
  const offsetY = useSharedValue(0);
  // Vite has no worklet transform, so the Web mapper needs explicit dependencies.
  const matrix = useDerivedValue(() => {
    const result = Skia.Matrix();
    result.translate(offsetX.value, offsetY.value);
    result.scale(scale.value, scale.value);
    return result;
  }, [scale, offsetX, offsetY]);
  const applyViewport = useCallback(
    (next: Viewport) => {
      current.current = next;
      scale.value = next.scale;
      offsetX.value = next.x;
      offsetY.value = next.y;
    },
    [scale, offsetX, offsetY],
  );
  const updateZoom = useCallback(
    () => setZoom(Math.round((current.current.scale / fitted.current.scale) * 100)),
    [],
  );
  const reset = useCallback(() => {
    applyViewport(fitted.current);
    updateZoom();
  }, [applyViewport, updateZoom]);
  const zoomBy = useCallback(
    (factor: number, anchor?: Point) => {
      const { width, height } = dimensions.current;
      applyViewport(
        zoomAt(
          current.current,
          current.current.scale * factor,
          anchor ?? { x: width / 2, y: height / 2 },
          fitted.current.scale / 20,
          fitted.current.scale * 128,
        ),
      );
      updateZoom();
    },
    [applyViewport, updateZoom],
  );

  useEffect(() => {
    setRenderError(null);
    if (scene.texts.length && !typeface) return;
    try {
      const result = buildPictures(scene, typeface);
      // Canvas can retain these handles past effect cleanup; Skia releases them when unreachable.
      setCompiled({ scene, ...result });
    } catch (error) {
      setRenderError(error instanceof Error ? error.message : "PCB 绘制失败。");
    }
    setVisible(new Set(scene.layers.filter((layer) => layer.visible).map((layer) => layer.id)));
    setSelection(null);
    setPickError(null);
    pickVersion.current += 1;
    return () => {
      pickVersion.current += 1;
    };
  }, [scene, typeface]);

  useEffect(() => {
    if (!size.width || !size.height) return;
    fitted.current = fitViewport(scene.bounds, size.width, size.height);
    reset();
  }, [scene.bounds, size.width, size.height, reset]);

  const pick = useCallback(
    async (point: Point) => {
      const version = ++pickVersion.current;
      const world = worldPoint(current.current, point);
      try {
        const result = await call<PickResult | null>({
          command: "pick",
          sceneId: scene.id,
          x: world.x,
          y: world.y,
          tolerance: 8 / current.current.scale,
          layers: [...visibleRef.current],
        });
        if (version !== pickVersion.current) return;
        const selected = result && visibleRef.current.has(result.layer) ? result : null;
        setSelection(selected);
        setPickError(null);
        onPick?.(selected);
      } catch (error) {
        if (version === pickVersion.current)
          setPickError(error instanceof Error ? error.message : "暂时无法拾取图元。");
      }
    },
    [scene.id, onPick],
  );

  const responder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => true,
        onMoveShouldSetPanResponder: () => true,
        onPanResponderGrant: (event) => {
          const points = touchPoints(event);
          if (!points.length) return;
          pickVersion.current += 1;
          gesture.current = {
            points,
            view: current.current,
            first: points[0],
            last: points[0],
            pointers: points.length,
            moved: false,
          };
        },
        onPanResponderMove: (event) => {
          const points = touchPoints(event);
          const start = gesture.current;
          if (!start || !points.length) return;
          start.last = points[0];
          start.pointers = Math.max(start.pointers, points.length);
          start.moved ||= !isTap(start.first, start.last, start.pointers);
          if (points.length !== start.points.length) {
            start.points = points;
            start.view = current.current;
            return;
          }
          applyViewport(
            gestureViewport(
              start.view,
              start.points,
              points,
              fitted.current.scale / 20,
              fitted.current.scale * 128,
            ),
          );
        },
        onPanResponderRelease: () => {
          const start = gesture.current;
          gesture.current = null;
          updateZoom();
          if (start && !start.moved && isTap(start.first, start.last, start.pointers))
            void pick(start.last);
        },
        onPanResponderTerminate: () => {
          gesture.current = null;
          updateZoom();
        },
        onPanResponderTerminationRequest: () => false,
      }),
    [applyViewport, pick, updateZoom],
  );

  useEffect(() => {
    if (Platform.OS !== "web") return;
    const element = surface.current as unknown as HTMLElement | null;
    if (!element?.addEventListener) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      pickVersion.current += 1;
      const rect = element.getBoundingClientRect();
      zoomBy(Math.exp(-Math.max(-200, Math.min(200, event.deltaY)) * 0.003), {
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
      });
    };
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === "+" || event.key === "=") zoomBy(1.25);
      else if (event.key === "-") zoomBy(0.8);
      else if (event.key === "0") reset();
      else if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
        const { x, y, scale: currentScale } = current.current;
        applyViewport({
          scale: currentScale,
          x: x + (event.key === "ArrowLeft" ? 30 : event.key === "ArrowRight" ? -30 : 0),
          y: y + (event.key === "ArrowUp" ? 30 : event.key === "ArrowDown" ? -30 : 0),
        });
      } else return;
      event.preventDefault();
    };
    element.addEventListener("wheel", wheel, { passive: false });
    element.addEventListener("keydown", keyboard);
    return () => {
      element.removeEventListener("wheel", wheel);
      element.removeEventListener("keydown", keyboard);
    };
  }, [zoomBy, reset, applyViewport]);

  return (
    <View style={styles.root}>
      <View style={styles.toolbar}>
        <View style={styles.zoomTools}>
          <Tool label="缩小 PCB" text="−" onPress={() => zoomBy(0.8)} />
          <Text style={styles.zoomLabel} accessibilityLiveRegion="polite">
            {zoom}%
          </Text>
          <Tool label="放大 PCB" text="+" onPress={() => zoomBy(1.25)} />
          <Tool label="适应窗口，快捷键 0" text="适应" onPress={reset} />
        </View>
        <Text style={styles.count}>{scene.objectCount.toLocaleString()} 个图元</Text>
      </View>
      <View style={[styles.canvasContainer, compact && styles.canvasCompact]}>
        <View
          ref={surface}
          collapsable={false}
          focusable
          accessible
          accessibilityRole="image"
          accessibilityLabel={`${scene.name}，PCB 画布。拖动平移，双指或滚轮缩放。键盘加减号缩放，方向键平移，0 适应窗口。`}
          accessibilityActions={[
            { name: "zoomIn", label: "放大" },
            { name: "zoomOut", label: "缩小" },
            { name: "reset", label: "适应窗口" },
          ]}
          onAccessibilityAction={({ nativeEvent }) => {
            if (nativeEvent.actionName === "zoomIn") zoomBy(1.25);
            else if (nativeEvent.actionName === "zoomOut") zoomBy(0.8);
            else reset();
          }}
          onLayout={({ nativeEvent }) =>
            setSize({ width: nativeEvent.layout.width, height: nativeEvent.layout.height })
          }
          style={styles.surface}
          {...responder.panHandlers}
        >
          {compiled && compiled.scene === scene && !renderError && !fontError ? (
            <Canvas style={StyleSheet.absoluteFill} pointerEvents="none">
              <Group matrix={matrix}>
                {compiled.pictures
                  .filter(({ layer }) => visible.has(layer.id))
                  .map(({ layer, picture }) => (
                    <Picture key={layer.id} picture={picture} />
                  ))}
              </Group>
            </Canvas>
          ) : (
            <View style={styles.canvasMessage}>
              {renderError || fontError ? (
                <Text style={styles.error} accessibilityRole="alert">
                  {renderError || fontError}
                </Text>
              ) : (
                <ActivityIndicator color="#56d6aa" accessibilityLabel="正在构建 PCB 绘制缓存" />
              )}
            </View>
          )}
          <View style={styles.hint} pointerEvents="none">
            <Text style={styles.hintText}>拖动平移 · 双指 / 滚轮缩放 · 点选图元</Text>
          </View>
        </View>
        {selection && (
          <View style={styles.selection}>
            <View style={styles.selectionTitle}>
              <Text selectable style={styles.selectionName}>
                {selection.kind} · {selection.id}
              </Text>
              <Tool
                text="×"
                label="关闭图元属性"
                onPress={() => {
                  setSelection(null);
                  onPick?.(null);
                }}
              />
            </View>
            <ScrollView style={styles.properties}>
              {selection.net && (
                <Text selectable style={styles.property}>
                  网络：{selection.net}
                </Text>
              )}
              {Object.entries(selection.properties).map(([key, value]) => (
                <Text selectable key={key} style={styles.property}>
                  {key}：{value}
                </Text>
              ))}
            </ScrollView>
          </View>
        )}
      </View>
      <View style={[styles.layerSection, compact && styles.layerCompact]}>
        {!compact && <Text style={styles.sectionLabel}>图层</Text>}
        <ScrollView
          horizontal
          showsHorizontalScrollIndicator={false}
          contentContainerStyle={styles.layerRow}
        >
          {scene.layers
            .slice()
            .sort((a, b) => Number(b.visible) - Number(a.visible) || a.order - b.order)
            .map((layer) => (
              <Pressable
                key={layer.id}
                accessibilityRole="checkbox"
                aria-checked={visible.has(layer.id)}
                accessibilityLabel={layer.name}
                onPress={() => {
                  setVisible((previous) => {
                    const next = new Set(previous);
                    if (next.has(layer.id)) next.delete(layer.id);
                    else next.add(layer.id);
                    return next;
                  });
                  pickVersion.current += 1;
                  setSelection(null);
                  onPick?.(null);
                }}
                style={[styles.layer, !visible.has(layer.id) && styles.layerOff]}
              >
                <View style={[styles.dot, { backgroundColor: layer.color }]} />
                <Text style={styles.layerText}>{layer.name}</Text>
                <Text style={styles.check}>{visible.has(layer.id) ? "✓" : "−"}</Text>
              </Pressable>
            ))}
        </ScrollView>
      </View>
      {pickError && (
        <Text accessibilityRole="alert" style={styles.pickError}>
          {pickError}
        </Text>
      )}
      {(scene.warnings.length > 0 || (compiled?.warnings.length ?? 0) > 0) && (
        <ScrollView style={[styles.warnings, compact && styles.warningsCompact]}>
          {[...scene.warnings, ...(compiled?.warnings ?? [])].map((warning, i) => (
            <Text key={`${i}-${warning}`} style={styles.warning}>
              {warning}
            </Text>
          ))}
        </ScrollView>
      )}
      {!compact && (
        <Text style={styles.metrics}>
          载入 {Math.round(scene.metrics.loadMs)} ms · 解码 {Math.round(scene.metrics.parseMs)} ms ·
          场景 {Math.round(scene.metrics.sceneMs)} ms
        </Text>
      )}
    </View>
  );
}

function Tool({ label, text, onPress }: { label: string; text: string; onPress: () => void }) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={label}
      onPress={onPress}
      style={({ pressed }) => [styles.tool, pressed && styles.pressed]}
    >
      <Text style={styles.toolText}>{text}</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, minHeight: 0, backgroundColor: "#fff" },
  toolbar: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: 12,
    paddingVertical: 6,
    gap: 8,
    flexWrap: "wrap",
    borderBottomWidth: 1,
    borderBottomColor: "#e9eeec",
  },
  zoomTools: {
    flexDirection: "row",
    alignItems: "center",
    gap: 2,
    flexWrap: "wrap",
    flexShrink: 1,
  },
  tool: {
    minWidth: 44,
    minHeight: 44,
    paddingHorizontal: 10,
    justifyContent: "center",
    alignItems: "center",
    borderRadius: 10,
  },
  pressed: { backgroundColor: "#e9f5ef" },
  toolText: { color: "#183e32", fontSize: 21, fontWeight: "500" },
  zoomLabel: {
    color: "#52665e",
    minWidth: 56,
    textAlign: "center",
    fontSize: 13,
    fontVariant: ["tabular-nums"],
  },
  count: { fontSize: 12, color: "#66786f" },
  canvasContainer: { flex: 1, minHeight: 80 },
  canvasCompact: { minHeight: 48 },
  surface: { flex: 1, minHeight: 0, overflow: "hidden", backgroundColor: "#0b121a" },
  canvasMessage: {
    ...StyleSheet.absoluteFill,
    alignItems: "center",
    justifyContent: "center",
    padding: 24,
  },
  error: { color: "#ffb2a5", fontSize: 14, lineHeight: 22 },
  hint: { position: "absolute", bottom: 12, left: 12, right: 12, alignItems: "center" },
  hintText: {
    color: "#b0c6bc",
    fontSize: 11,
    lineHeight: 18,
    textAlign: "center",
    backgroundColor: "#0b121acc",
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
  },
  layerSection: { borderBottomWidth: 1, borderBottomColor: "#edf0ee", paddingTop: 10 },
  layerCompact: { paddingTop: 0 },
  sectionLabel: { paddingHorizontal: 16, fontSize: 12, fontWeight: "600", color: "#5a6b62" },
  layerRow: { paddingHorizontal: 12, paddingBottom: 10, paddingTop: 4, gap: 8 },
  layer: {
    minHeight: 44,
    flexDirection: "row",
    alignItems: "center",
    gap: 7,
    paddingHorizontal: 12,
    borderWidth: 1,
    borderColor: "#dae8e0",
    borderRadius: 10,
    backgroundColor: "#f3f8f5",
  },
  layerOff: { backgroundColor: "#fafbfa", borderColor: "#ecf0ed", opacity: 0.6 },
  dot: { width: 9, height: 9, borderRadius: 5 },
  layerText: { fontSize: 12, color: "#31473d" },
  check: { fontSize: 12, color: "#21835d" },
  pickError: { paddingHorizontal: 16, paddingVertical: 8, fontSize: 12, color: "#ad4937" },
  selection: {
    position: "absolute",
    top: 8,
    right: 12,
    width: 300,
    maxWidth: "92%",
    maxHeight: "85%",
    borderRadius: 10,
    backgroundColor: "#fff",
    paddingHorizontal: 16,
    paddingBottom: 10,
    borderWidth: 1,
    borderColor: "#e9eeec",
  },
  selectionTitle: { flexDirection: "row", alignItems: "center", gap: 8 },
  selectionName: { flex: 1, fontSize: 13, color: "#1f4635", fontWeight: "600" },
  properties: { flexGrow: 0 },
  property: { fontSize: 12, lineHeight: 20, color: "#52665d" },
  warnings: { flexGrow: 0, maxHeight: 84, backgroundColor: "#fff8eb" },
  warningsCompact: { maxHeight: 44 },
  warning: {
    paddingHorizontal: 16,
    paddingVertical: 5,
    color: "#805e1c",
    fontSize: 12,
    lineHeight: 18,
  },
  metrics: { fontSize: 10, color: "#819087", paddingHorizontal: 16, paddingVertical: 7 },
});

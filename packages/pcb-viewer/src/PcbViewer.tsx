import { call, type PickResult, type Scene } from "@oshwhub/bridge";
import {
  Canvas,
  type DataSource,
  Group,
  Image,
  Skia,
  useTypeface,
} from "@shopify/react-native-skia";
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
  useWindowDimensions,
  View,
  type ViewInstance,
} from "react-native";
import { useDerivedValue, useSharedValue } from "react-native-reanimated";
import { defaultFontSource } from "./fontSource";
import { type BoardRaster, buildPictures, type LayerPicture, rasterize } from "./pictures";
import { coversView, detailBounds, planRaster } from "./raster";
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
type RasterCache = { pictures: LayerPicture[]; base: BoardRaster; detail: BoardRaster | null };
const touchPoints = (event: GestureResponderEvent): Point[] =>
  Array.from(event.nativeEvent.touches).map((touch) => ({
    x: touch.locationX,
    y: touch.locationY,
  }));

export function PcbViewer({ scene, onPick, fontSource = defaultFontSource }: Props) {
  const { width, height } = useWindowDimensions();
  const compact = height < 500;
  const bottomPanel = width < 600 && !compact;
  const [panel, setPanel] = useState<"layers" | "info" | "selection" | null>(null);
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
  const layers = useMemo(
    () =>
      scene.layers
        .slice()
        .sort((a, b) => Number(b.visible) - Number(a.visible) || a.order - b.order),
    [scene.layers],
  );
  const [pickError, setPickError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(100);
  const [cache, setCache] = useState<RasterCache | null>(null);
  const cacheRef = useRef<RasterCache | null>(null);
  const [rasterWarning, setRasterWarning] = useState<string | null>(null);
  const refinement = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const surface = useRef<ViewInstance>(null);
  const current = useRef<Viewport>({ scale: 1, x: 0, y: 0 });
  const fitted = useRef<Viewport>({ scale: 1, x: 0, y: 0 });
  const dimensions = useRef(size);
  dimensions.current = size;
  const visibleRef = useRef(visible);
  visibleRef.current = visible;
  const gesture = useRef<Gesture | null>(null);
  const pickVersion = useRef(0);
  const camera = useSharedValue<Viewport>(current.current);
  // Vite has no worklet transform, so the Web mapper needs explicit dependencies.
  const matrix = useDerivedValue(() => {
    const result = Skia.Matrix();
    const view = camera.value;
    result.translate(view.x, view.y);
    result.scale(view.scale, view.scale);
    return result;
  }, [camera]);
  const cancelRefinement = useCallback(() => clearTimeout(refinement.current), []);
  const refine = useCallback(() => {
    cancelRefinement();
    refinement.current = setTimeout(() => {
      const source = cacheRef.current;
      const { width, height } = dimensions.current;
      if (!source || !width || !height || gesture.current) return;
      const view = current.current;
      const density = view.scale * PixelRatio.get();
      const baseDensity = Math.min(
        source.base.pixelWidth / source.base.width,
        source.base.pixelHeight / source.base.height,
      );
      if (density <= baseDensity * 1.1) {
        if (source.detail) {
          const next = { ...source, detail: null };
          cacheRef.current = next;
          setCache(next);
        }
        return;
      }
      const plan = planRaster(detailBounds(view, width, height), density);
      if (
        source.detail &&
        source.detail.pixelWidth / source.detail.width >= (plan.pixelWidth / plan.width) * 0.9 &&
        coversView(source.detail, view, width, height)
      )
        return;
      try {
        const next = { ...source, detail: rasterize(source.pictures, plan) };
        cacheRef.current = next;
        setCache(next);
        setRasterWarning(null);
      } catch (error) {
        const message = error instanceof Error ? error.message : "PCB 细节缓存构建失败。";
        console.warn("pcb.raster.refine_failed", { sceneId: scene.id, message });
        setRasterWarning(`${message} 当前保留全板预览，可继续操作。`);
      }
    }, 180);
  }, [cancelRefinement, scene.id]);
  const applyViewport = useCallback(
    (next: Viewport) => {
      current.current = next;
      camera.value = next;
      refine();
    },
    [camera, refine],
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
    setPanel(null);
    setPickError(null);
    pickVersion.current += 1;
    return () => {
      pickVersion.current += 1;
    };
  }, [scene, typeface]);

  const pictures = useMemo(
    () =>
      compiled?.scene === scene
        ? compiled.pictures.filter(({ layer }) => visible.has(layer.id))
        : null,
    [compiled, scene, visible],
  );
  useEffect(() => {
    if (!pictures) return;
    try {
      const bounds = scene.bounds;
      const span = Math.max(bounds.maxX - bounds.minX, bounds.maxY - bounds.minY, 1);
      const margin = span * 0.1;
      const plan = planRaster(
        {
          minX: bounds.minX - margin,
          minY: bounds.minY - margin,
          maxX: bounds.maxX + margin,
          maxY: bounds.maxY + margin,
        },
        2048 / (span + margin * 2),
      );
      const next = { pictures, base: rasterize(pictures, plan), detail: null };
      cacheRef.current = next;
      setCache(next);
      setRenderError(null);
      setRasterWarning(null);
      refine();
    } catch (error) {
      setRenderError(error instanceof Error ? error.message : "PCB 绘制缓存构建失败。");
    }
    return () => {
      cacheRef.current = null;
      cancelRefinement();
    };
  }, [pictures, scene.bounds, refine, cancelRefinement]);

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
        setPanel((previous) =>
          selected ? "selection" : previous === "selection" ? null : previous,
        );
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
          cancelRefinement();
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
          refine();
          if (start && !start.moved && isTap(start.first, start.last, start.pointers))
            void pick(start.last);
        },
        onPanResponderTerminate: () => {
          gesture.current = null;
          updateZoom();
          refine();
        },
        onPanResponderTerminationRequest: () => false,
      }),
    [applyViewport, pick, updateZoom, refine, cancelRefinement],
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

  const drawing = useMemo(
    () =>
      cache && (
        <Canvas style={StyleSheet.absoluteFill} pointerEvents="none">
          <Group matrix={matrix}>
            {[cache.base, cache.detail].map(
              (raster) =>
                raster && (
                  <Image
                    key={raster === cache.base ? "overview" : "detail"}
                    image={raster.image}
                    x={raster.x}
                    y={raster.y}
                    width={raster.width}
                    height={raster.height}
                    fit="fill"
                  />
                ),
            )}
          </Group>
        </Canvas>
      ),
    [cache, matrix],
  );
  const warnings = [
    ...scene.warnings,
    ...(compiled?.warnings ?? []),
    ...(rasterWarning ? [rasterWarning] : []),
  ];
  const closeSelection = () => {
    setSelection(null);
    setPanel(null);
    onPick?.(null);
  };
  return (
    <View style={styles.root}>
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
        {cache && cache.pictures === pictures && !renderError && !fontError ? (
          drawing
        ) : (
          <View style={styles.canvasMessage}>
            {renderError || fontError ? (
              <Text style={styles.error} accessibilityRole="alert">
                {renderError || fontError}
              </Text>
            ) : (
              <ActivityIndicator color="#9bc8ff" accessibilityLabel="正在构建 PCB 绘制缓存" />
            )}
          </View>
        )}
      </View>
      <View style={[styles.tools, compact && styles.compactTools]}>
        <View style={[styles.toolGroup, compact && styles.compactToolGroup]}>
          <Tool
            label="图层"
            text="图层"
            expanded={panel === "layers"}
            onPress={() => setPanel(panel === "layers" ? null : "layers")}
          />
          <View style={[styles.separator, compact && styles.compactSeparator]} />
          <Tool label="适应窗口，快捷键 0" text="适应" onPress={reset} />
        </View>
        <View style={styles.toolGroup}>
          <Tool
            label="查看器信息"
            text={warnings.length ? `信息 · ${warnings.length}` : "信息"}
            expanded={panel === "info"}
            warning={warnings.length > 0}
            onPress={() => setPanel(panel === "info" ? null : "info")}
          />
        </View>
      </View>
      {!compact && (
        <>
          <View style={styles.orientation} pointerEvents="none">
            <Text style={styles.orientationTitle}>俯视</Text>
            <Text style={styles.caption}>二维 PCB</Text>
          </View>
          <View style={styles.hint} pointerEvents="none">
            <Text style={styles.hintText}>拖动平移 · 双指 / 滚轮缩放 · 点选图元</Text>
          </View>
        </>
      )}
      <View style={styles.status} pointerEvents="none">
        <Text style={styles.caption}>{scene.objectCount.toLocaleString()} 个图元</Text>
        {!compact && (
          <Text style={styles.caption}>
            {visible.size} / {scene.layers.length} 图层
          </Text>
        )}
      </View>
      <View style={styles.zoomTools}>
        <Tool label="缩小 PCB" text="−" onPress={() => zoomBy(0.8)} />
        <Text style={styles.zoomLabel} accessibilityLiveRegion="polite">
          {zoom}%
        </Text>
        <Tool label="放大 PCB" text="+" onPress={() => zoomBy(1.25)} />
      </View>
      {pickError && (
        <Text accessibilityRole="alert" style={styles.pickError}>
          {pickError}
        </Text>
      )}
      {panel && (
        <View
          style={[styles.panel, bottomPanel && styles.bottomPanel, compact && styles.compactPanel]}
        >
          <View style={styles.panelHeader}>
            <View style={styles.panelHeading}>
              <Text
                accessibilityRole="header"
                selectable
                numberOfLines={2}
                style={styles.panelTitle}
              >
                {panel === "layers"
                  ? "图层"
                  : panel === "info"
                    ? "查看器信息"
                    : `${selection?.kind} · ${selection?.id}`}
              </Text>
              {panel === "layers" && (
                <Text style={styles.caption}>
                  {visible.size} / {scene.layers.length} 层可见
                </Text>
              )}
            </View>
            <Tool
              label={
                panel === "layers"
                  ? "关闭图层面板"
                  : panel === "info"
                    ? "关闭查看器信息"
                    : "关闭图元属性"
              }
              text="关闭"
              onPress={panel === "selection" ? closeSelection : () => setPanel(null)}
            />
          </View>
          <ScrollView
            style={styles.panelScroll}
            contentContainerStyle={
              panel === "layers" ? undefined : [styles.details, compact && styles.compactDetails]
            }
          >
            {panel === "layers" ? (
              layers.map((layer) => (
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
                  style={({ pressed }) => [
                    styles.layer,
                    visible.has(layer.id) && styles.layerOn,
                    pressed && styles.pressed,
                  ]}
                >
                  <View style={[styles.dot, { backgroundColor: layer.color }]} />
                  <Text style={[styles.layerText, !visible.has(layer.id) && styles.layerOff]}>
                    {layer.name}
                  </Text>
                  <Text style={[styles.visibility, visible.has(layer.id) && styles.visibilityOn]}>
                    {visible.has(layer.id) ? "显示" : "隐藏"}
                  </Text>
                </Pressable>
              ))
            ) : panel === "info" ? (
              <>
                <Text selectable style={styles.selectionName}>
                  {scene.name}
                </Text>
                {[
                  ["场景图元", scene.objectCount.toLocaleString()],
                  ["可见图层", `${visible.size} / ${scene.layers.length}`],
                  ["载入耗时", `${Math.round(scene.metrics.loadMs)} ms`],
                  ["解码耗时", `${Math.round(scene.metrics.parseMs)} ms`],
                  ["场景构建", `${Math.round(scene.metrics.sceneMs)} ms`],
                  ...(cache
                    ? [
                        [
                          "全板预览",
                          `${cache.base.pixelWidth} × ${cache.base.pixelHeight} · ${Math.round(cache.base.elapsedMs)} ms`,
                        ],
                      ]
                    : []),
                  ...(cache?.detail
                    ? [
                        [
                          "视区细节",
                          `${cache.detail.pixelWidth} × ${cache.detail.pixelHeight} · ${Math.round(cache.detail.elapsedMs)} ms`,
                        ],
                      ]
                    : []),
                ].map(([label, value]) => (
                  <View key={label} style={styles.infoRow}>
                    <Text style={styles.caption}>{label}</Text>
                    <Text selectable style={styles.infoValue}>
                      {value}
                    </Text>
                  </View>
                ))}
                {warnings.length > 0 && (
                  <Text style={styles.sectionLabel}>显示提示 · {warnings.length}</Text>
                )}
                {warnings.map((warning, i) => (
                  <Text key={`${i}-${warning}`} style={styles.warning}>
                    {warning}
                  </Text>
                ))}
              </>
            ) : (
              selection && (
                <>
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
                </>
              )
            )}
          </ScrollView>
        </View>
      )}
    </View>
  );
}

function Tool({
  label,
  text,
  onPress,
  expanded,
  warning = false,
}: {
  label: string;
  text: string;
  onPress: () => void;
  expanded?: boolean;
  warning?: boolean;
}) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={label}
      aria-expanded={expanded}
      onPress={onPress}
      style={({ pressed }) => [
        styles.tool,
        expanded && styles.toolActive,
        pressed && styles.pressed,
      ]}
    >
      <Text style={[styles.toolText, warning && styles.warningText, expanded && styles.activeText]}>
        {text}
      </Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, minHeight: 0, backgroundColor: "#0b121a" },
  surface: { ...StyleSheet.absoluteFill, overflow: "hidden", backgroundColor: "#0b121a" },
  tools: { position: "absolute", top: 16, left: 12, gap: 10 },
  compactTools: { top: 12, flexDirection: "row" },
  toolGroup: {
    padding: 4,
    borderRadius: 13,
    backgroundColor: "#202b39f5",
    borderWidth: 1,
    borderColor: "#ffffff14",
    boxShadow: "0 4px 18px #00000026",
  },
  compactToolGroup: { flexDirection: "row", alignItems: "center" },
  separator: { height: 1, backgroundColor: "#ffffff12", margin: 4 },
  compactSeparator: { width: 1, height: 24 },
  tool: {
    minWidth: 44,
    minHeight: 44,
    paddingHorizontal: 10,
    borderRadius: 9,
    justifyContent: "center",
    alignItems: "center",
  },
  toolText: { color: "#d5dfe9", fontSize: 13, fontWeight: "500" },
  toolActive: { backgroundColor: "#70b5ff26" },
  activeText: { color: "#a9d3ff" },
  warningText: { color: "#edc17d" },
  pressed: { backgroundColor: "#ffffff18" },
  zoomTools: {
    position: "absolute",
    right: 12,
    bottom: 12,
    flexDirection: "row",
    alignItems: "center",
    padding: 3,
    backgroundColor: "#202b39f5",
    borderRadius: 13,
    borderWidth: 1,
    borderColor: "#ffffff14",
  },
  zoomLabel: {
    color: "#d5dfe9",
    minWidth: 48,
    textAlign: "center",
    fontSize: 12,
    fontVariant: ["tabular-nums"],
  },
  orientation: {
    position: "absolute",
    top: 16,
    right: 16,
    alignItems: "center",
    gap: 4,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#ffffff14",
    backgroundColor: "#202b39cc",
    padding: 12,
  },
  orientationTitle: { color: "#e1e8f0", fontSize: 13, fontWeight: "600" },
  caption: { color: "#9eadbd", fontSize: 11, lineHeight: 18 },
  status: { position: "absolute", left: 16, bottom: 16, gap: 1 },
  hint: { position: "absolute", bottom: 80, left: 12, right: 12, alignItems: "center" },
  hintText: {
    color: "#95a6b6",
    fontSize: 10,
    lineHeight: 18,
    textAlign: "center",
    backgroundColor: "#0b121acc",
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
  },
  canvasMessage: {
    ...StyleSheet.absoluteFill,
    alignItems: "center",
    justifyContent: "center",
    padding: 24,
  },
  error: { color: "#ffb2a5", fontSize: 14, lineHeight: 22 },
  pickError: {
    position: "absolute",
    top: 12,
    left: 92,
    right: 12,
    padding: 12,
    backgroundColor: "#4b2828",
    borderRadius: 10,
    fontSize: 12,
    color: "#ffc9bc",
  },
  panel: {
    position: "absolute",
    top: 16,
    right: 12,
    bottom: 80,
    width: 288,
    maxHeight: 520,
    borderRadius: 16,
    backgroundColor: "#202b39fa",
    borderWidth: 1,
    borderColor: "#ffffff1f",
    overflow: "hidden",
    boxShadow: "0 8px 32px #00000040",
  },
  bottomPanel: { top: "auto", left: 12, right: 12, width: "auto", maxHeight: "56%" },
  compactPanel: { top: 12, bottom: 70 },
  panelHeader: {
    minHeight: 52,
    flexDirection: "row",
    alignItems: "center",
    paddingLeft: 16,
    paddingRight: 6,
    gap: 8,
    borderBottomWidth: 1,
    borderBottomColor: "#ffffff14",
  },
  panelHeading: { flex: 1, paddingVertical: 10 },
  panelTitle: { color: "#edf2f7", fontSize: 13, fontWeight: "600", lineHeight: 20 },
  panelScroll: { flexGrow: 0, flexShrink: 1 },
  details: { padding: 16, gap: 10 },
  compactDetails: { paddingVertical: 8 },
  sectionLabel: { color: "#edc17d", fontSize: 12, fontWeight: "600", marginTop: 8 },
  layer: {
    minHeight: 48,
    flexDirection: "row",
    alignItems: "center",
    gap: 10,
    paddingHorizontal: 16,
    paddingVertical: 10,
    borderBottomWidth: 1,
    borderBottomColor: "#ffffff0a",
  },
  layerOn: { backgroundColor: "#70b5ff08" },
  dot: { width: 10, height: 10, borderRadius: 5, borderWidth: 1, borderColor: "#ffffff40" },
  layerText: { flex: 1, color: "#e0e8ef", fontSize: 12, lineHeight: 20 },
  layerOff: { color: "#98a7b7" },
  visibility: { fontSize: 10, color: "#98a7b7" },
  visibilityOn: { color: "#a9d3ff" },
  selectionName: { color: "#edf2f7", fontSize: 13, fontWeight: "600", lineHeight: 20 },
  property: { color: "#c0cddb", fontSize: 12, lineHeight: 20 },
  infoRow: { flexDirection: "row", justifyContent: "space-between", gap: 12 },
  infoValue: { color: "#d5dfe9", fontSize: 12, lineHeight: 18, fontVariant: ["tabular-nums"] },
  warning: { color: "#edc17d", fontSize: 12, lineHeight: 20 },
});

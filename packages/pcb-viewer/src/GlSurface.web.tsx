import { useEffect, useRef } from "react";
import type { GlSurfaceProps } from "./GlSurface";

export function GlSurface({ width, height, onContext, onError }: GlSurfaceProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    let cleanup: (() => void) | undefined;
    const lost = (event: Event) => {
      event.preventDefault();
      onError("三维图形上下文已丢失，请重新载入三维视图。");
    };
    element.addEventListener("webglcontextlost", lost);
    const gl = element.getContext("webgl", {
      alpha: false,
      antialias: true,
      preserveDrawingBuffer: false,
    });
    try {
      if (!gl) throw new Error("当前设备未启用 WebGL，可继续使用二维模式。");
      cleanup = onContext(gl);
    } catch (error) {
      onError(error instanceof Error ? error.message : "无法创建三维画布。");
    }
    return () => {
      element.removeEventListener("webglcontextlost", lost);
      cleanup?.();
    };
  }, [onContext, onError]);
  const ratio = Math.min(globalThis.devicePixelRatio || 1, 2, 2400 / Math.max(width, height, 1));
  return (
    <canvas
      ref={canvas}
      tabIndex={-1}
      width={Math.max(1, Math.round(width * ratio))}
      height={Math.max(1, Math.round(height * ratio))}
      style={{
        position: "absolute",
        inset: 0,
        width: "100%",
        height: "100%",
        pointerEvents: "none",
      }}
      aria-hidden="true"
    />
  );
}

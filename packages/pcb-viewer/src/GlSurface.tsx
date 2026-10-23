import { GLView } from "expo-gl";
import { useCallback, useEffect, useRef } from "react";
import { StyleSheet } from "react-native";

export type GlSurfaceProps = {
  width: number;
  height: number;
  onContext: (gl: WebGLRenderingContext) => () => void;
  onError: (message: string) => void;
};

export function GlSurface({ onContext, onError }: GlSurfaceProps) {
  const cleanup = useRef<(() => void) | null>(null);
  useEffect(() => () => cleanup.current?.(), []);
  const ready = useCallback(
    (gl: Parameters<NonNullable<React.ComponentProps<typeof GLView>["onContextCreate"]>>[0]) => {
      try {
        cleanup.current = onContext(gl as unknown as WebGLRenderingContext);
      } catch (error) {
        onError(error instanceof Error ? error.message : "无法创建三维画布。");
      }
    },
    [onContext, onError],
  );
  return <GLView style={StyleSheet.absoluteFill} onContextCreate={ready} msaaSamples={0} />;
}

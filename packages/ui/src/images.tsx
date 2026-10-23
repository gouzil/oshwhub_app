import { useRef, useState } from "react";
import {
  Image,
  PixelRatio,
  Pressable,
  type StyleProp,
  StyleSheet,
  Text,
  View,
  type ViewStyle,
} from "react-native";
import { Skeleton, SkeletonBlock } from "./loading";
import { imagePreviewUrl, safeUrl } from "./model";

export function RemoteImage({
  url,
  label,
  enabled = true,
  aspectRatio = 1.6,
  natural = false,
  resizeMode = "contain",
  style,
}: {
  url: string;
  label: string;
  enabled?: boolean;
  aspectRatio?: number;
  natural?: boolean;
  resizeMode?: "contain" | "cover";
  style?: StyleProp<ViewStyle>;
}) {
  const [width, setWidth] = useState(0);
  const [fallbackFor, setFallbackFor] = useState<string | null>(null);
  const [failedFor, setFailedFor] = useState<string | null>(null);
  const [loadedFor, setLoadedFor] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [dimensions, setDimensions] = useState<{ uri: string; ratio: number } | null>(null);
  const original = safeUrl(url);
  const preview = imagePreviewUrl(original, PixelRatio.getPixelSizeForLayoutSize(width));
  const uri = fallbackFor === preview ? original : preview;
  const requestKey = `${uri}-${attempt}`;
  const currentRequest = useRef(requestKey);
  currentRequest.current = requestKey;
  const failed = failedFor === requestKey || !original;
  const loaded = loadedFor === requestKey;
  const ratio = natural && dimensions?.uri === original ? dimensions.ratio : aspectRatio;
  const updateRatio = (imageWidth: number, imageHeight: number) => {
    if (
      currentRequest.current === requestKey &&
      original &&
      Number.isFinite(imageWidth) &&
      Number.isFinite(imageHeight) &&
      imageWidth > 0 &&
      imageHeight > 0
    )
      setDimensions({ uri: original, ratio: imageWidth / imageHeight });
  };
  return (
    <View
      onLayout={(event) => setWidth(Math.round(event.nativeEvent.layout.width))}
      style={[styles.frame, { aspectRatio: ratio }, style]}
    >
      {failed ? (
        <View style={styles.error}>
          <Text style={styles.errorText}>图片暂时无法显示</Text>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={`重新加载图片：${label}`}
            onPress={() => {
              setFallbackFor(null);
              setAttempt((value) => value + 1);
            }}
            style={styles.retry}
          >
            <Text style={styles.retryText}>重新加载图片</Text>
          </Pressable>
        </View>
      ) : (
        <>
          {enabled && width > 0 && uri && (
            <Image
              key={requestKey}
              source={{ uri }}
              accessibilityLabel={label}
              resizeMode={resizeMode}
              resizeMethod="resize"
              onError={() => {
                if (currentRequest.current !== requestKey) return;
                console.warn("image.load_failed", {
                  host: new URL(uri).hostname,
                  preview: uri !== original,
                });
                if (uri !== original) setFallbackFor(preview);
                else setFailedFor(requestKey);
              }}
              onLoad={({ nativeEvent }) => {
                if (currentRequest.current !== requestKey) return;
                setLoadedFor(requestKey);
                if (natural) {
                  const source = nativeEvent.source;
                  if (source) updateRatio(source.width, source.height);
                  else Image.getSize(uri, updateRatio, () => {});
                }
              }}
              style={[StyleSheet.absoluteFill, !loaded && { opacity: 0 }]}
            />
          )}
          {!loaded &&
            (enabled ? (
              <Skeleton label={`正在加载图片：${label}`} style={StyleSheet.absoluteFill}>
                <SkeletonBlock style={{ flex: 1, height: "100%" }} />
              </Skeleton>
            ) : (
              <SkeletonBlock style={StyleSheet.absoluteFill} />
            ))}
        </>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  frame: { width: "100%", backgroundColor: "#f5f8f5", borderRadius: 10, overflow: "hidden" },
  error: { flex: 1, alignItems: "center", justifyContent: "center", gap: 10, padding: 16 },
  errorText: { color: "#738079", fontSize: 13 },
  retry: { minHeight: 44, padding: 12, justifyContent: "center" },
  retryText: { color: "#07845d", fontSize: 14 },
});

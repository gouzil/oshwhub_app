import { useRef, useState } from "react";
import { Image, PixelRatio, type StyleProp, View, type ViewStyle } from "react-native";
import { Button } from "./components/ui/button";
import { Text } from "./components/ui/text";
import { cn } from "./lib/utils";
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
  className,
}: {
  url: string;
  label: string;
  enabled?: boolean;
  aspectRatio?: number;
  natural?: boolean;
  resizeMode?: "contain" | "cover";
  style?: StyleProp<ViewStyle>;
  className?: string;
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
      className={cn("w-full overflow-hidden rounded-[10px] bg-canvas", className)}
      style={[{ aspectRatio: ratio }, style]}
    >
      {failed ? (
        <View className="flex-1 items-center justify-center gap-2.5 p-4">
          <Text className="text-[13px] text-muted-foreground">图片暂时无法显示</Text>
          <Button
            variant="link"
            accessibilityLabel={`重新加载图片：${label}`}
            onPress={() => {
              setFallbackFor(null);
              setAttempt((value) => value + 1);
            }}
            className="min-h-11 p-3"
          >
            <Text className="text-sm font-normal">重新加载图片</Text>
          </Button>
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
              className={cn("absolute inset-0", !loaded && "opacity-0")}
            />
          )}
          {!loaded &&
            (enabled ? (
              <Skeleton label={`正在加载图片：${label}`} className="absolute inset-0">
                <SkeletonBlock className="h-full flex-1" />
              </Skeleton>
            ) : (
              <SkeletonBlock className="absolute inset-0 h-auto" />
            ))}
        </>
      )}
    </View>
  );
}

import { type Command, call, openExternal } from "@oshwhub/bridge";
import { useEffect, useState } from "react";
import { ActivityIndicator, Pressable, StyleSheet, Text, View, type ViewStyle } from "react-native";
import { RemoteImage } from "./images";
import { safeUrl } from "./model";

export const color = {
  green: "#07845d",
  greenLight: "#eaf6ef",
  ink: "#192d26",
  muted: "#738079",
  line: "#e9eeeb",
  canvas: "#f7f9f7",
};

export function useCommand<T>(command: Command) {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<{ value: T | null; error: Error | null; loading: boolean }>({
    value: null,
    error: null,
    loading: true,
  });
  // biome-ignore lint/correctness/useExhaustiveDependencies: The attempt counter retries the same command.
  useEffect(() => {
    let active = true;
    setState({ value: null, error: null, loading: true });
    call<T>(command).then(
      (value) => {
        if (active) setState({ value, error: null, loading: false });
      },
      (error: unknown) => {
        if (active)
          setState({
            value: null,
            error: error instanceof Error ? error : new Error("暂时无法加载，请重试。"),
            loading: false,
          });
      },
    );
    return () => {
      active = false;
    };
  }, [command, attempt]);
  return { ...state, retry: () => setAttempt((n) => n + 1) };
}

export function Button({
  label,
  onPress,
  secondary = false,
  disabled = false,
  style,
}: {
  label: string;
  onPress: () => void;
  secondary?: boolean;
  disabled?: boolean;
  style?: ViewStyle;
}) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={label}
      aria-disabled={disabled}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        common.button,
        secondary && common.secondary,
        disabled && common.disabled,
        pressed && common.pressed,
        style,
      ]}
    >
      <Text style={[common.buttonText, secondary && common.secondaryText]}>{label}</Text>
    </Pressable>
  );
}

export function ErrorPanel({ error, retry }: { error: Error; retry: () => void }) {
  const details = error as Error & { code?: string; requestId?: string };
  return (
    <View style={common.errorPanel} accessibilityRole="alert">
      <Text style={common.stateTitle}>暂时无法加载</Text>
      <Text selectable style={common.stateBody}>
        {error.message}
      </Text>
      {(details.code || details.requestId) && (
        <Text selectable style={common.errorCode}>
          {[details.code, details.requestId].filter(Boolean).join(" · ")}
        </Text>
      )}
      <Button label="重新加载" secondary onPress={retry} />
    </View>
  );
}

export function Loading({ label = "正在加载" }: { label?: string }) {
  return (
    <View style={common.state}>
      <ActivityIndicator color={color.green} accessibilityLabel={label} />
      <Text style={common.stateBody}>{label}</Text>
    </View>
  );
}

export function PageHeader({ title, onBack }: { title: string; onBack: () => void }) {
  return (
    <View style={common.pageHeader}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="返回上一页"
        onPress={onBack}
        style={({ pressed }) => [common.backButton, pressed && common.pressed]}
      >
        <Text style={common.backArrow}>‹</Text>
        <Text style={common.backText}>返回</Text>
      </Pressable>
      <Text numberOfLines={1} accessibilityRole="header" style={common.pageHeaderTitle}>
        {title}
      </Text>
    </View>
  );
}

export function Empty({ title, detail }: { title: string; detail: string }) {
  return (
    <View style={common.state}>
      <BoardMark small />
      <Text style={common.stateTitle}>{title}</Text>
      <Text style={common.stateBody}>{detail}</Text>
    </View>
  );
}

export function ExternalLink({ url, label }: { url: string; label: string }) {
  const [error, setError] = useState(false);
  const target = safeUrl(url);
  if (!target) return <Text style={common.stateBody}>{label}（链接不可用）</Text>;
  return (
    <View>
      <Pressable
        accessibilityRole="link"
        accessibilityLabel={`${label}，在浏览器中打开`}
        onPress={() => {
          setError(false);
          openExternal(target).catch(() => setError(true));
        }}
        style={({ pressed }) => [common.link, pressed && common.pressed]}
      >
        <Text style={common.linkText}>
          {label} <Text accessibilityElementsHidden>↗</Text>
        </Text>
      </Pressable>
      {error && (
        <Text accessibilityRole="alert" style={common.errorCode}>
          无法打开链接，请稍后重试。
        </Text>
      )}
    </View>
  );
}

export function Cover({
  url,
  title,
  aspectRatio = 1.17,
}: {
  url: string | null;
  title: string;
  aspectRatio?: number;
}) {
  const uri = safeUrl(url);
  const fallback = (
    <View style={common.coverFallback}>
      <BoardMark />
      <Text style={common.coverCaption}>开源硬件项目</Text>
    </View>
  );
  return (
    <View style={[common.cover, { aspectRatio }]}>
      {uri ? (
        <RemoteImage url={uri} label={title} aspectRatio={aspectRatio} resizeMode="cover" />
      ) : (
        fallback
      )}
    </View>
  );
}

export function BoardMark({ small = false }: { small?: boolean }) {
  return (
    <View accessible={false} style={[common.boardMark, small && common.boardMarkSmall]}>
      <View style={common.boardChip} />
      <View style={common.boardTraceA} />
      <View style={common.boardTraceB} />
      <View style={common.boardPinA} />
      <View style={common.boardPinB} />
    </View>
  );
}

export const common = StyleSheet.create({
  pageHeader: {
    minHeight: 62,
    paddingHorizontal: 12,
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    borderBottomWidth: 1,
    borderBottomColor: color.line,
  },
  backButton: {
    minHeight: 44,
    minWidth: 64,
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
    paddingRight: 8,
    justifyContent: "center",
    borderRadius: 10,
  },
  backArrow: { fontSize: 34, lineHeight: 38, color: color.green },
  backText: { fontSize: 13, color: color.green },
  pageHeaderTitle: { flex: 1, minWidth: 0, fontSize: 15, fontWeight: "600", color: color.ink },
  button: {
    minHeight: 46,
    minWidth: 44,
    paddingHorizontal: 20,
    paddingVertical: 12,
    borderRadius: 11,
    backgroundColor: color.green,
    alignItems: "center",
    justifyContent: "center",
  },
  buttonText: { fontSize: 14, fontWeight: "600", color: "#fff", textAlign: "center" },
  secondary: { backgroundColor: color.greenLight },
  secondaryText: { color: color.green },
  disabled: { opacity: 0.45 },
  pressed: { opacity: 0.72 },
  state: {
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: 24,
    paddingVertical: 48,
    gap: 14,
    flexGrow: 1,
  },
  stateTitle: { fontSize: 18, fontWeight: "600", color: color.ink, textAlign: "center" },
  stateBody: { fontSize: 14, color: color.muted, lineHeight: 23, textAlign: "center" },
  errorPanel: {
    borderWidth: 1,
    borderColor: "#efe2dc",
    backgroundColor: "#fffaf7",
    borderRadius: 16,
    padding: 24,
    marginVertical: 20,
    gap: 14,
    alignItems: "center",
  },
  errorCode: { color: "#946a58", fontSize: 11, lineHeight: 18, textAlign: "center" },
  link: { minHeight: 44, justifyContent: "center", paddingVertical: 10 },
  linkText: { color: color.green, fontSize: 14, lineHeight: 22 },
  cover: { width: "100%", overflow: "hidden", backgroundColor: "#eaf0eb" },
  coverFallback: { flex: 1, alignItems: "center", justifyContent: "center", gap: 12 },
  coverCaption: { fontSize: 10, color: "#6e8b78", letterSpacing: 2 },
  boardMark: {
    width: 64,
    height: 48,
    borderWidth: 1.5,
    borderColor: "#a6c8b2",
    borderRadius: 7,
    transform: [{ rotate: "-7deg" }],
  },
  boardMarkSmall: { width: 58, height: 44 },
  boardChip: {
    position: "absolute",
    width: 18,
    height: 18,
    left: 20,
    top: 13,
    borderRadius: 3,
    borderWidth: 1.5,
    borderColor: "#89b39b",
    backgroundColor: "#d6e7db",
  },
  boardTraceA: {
    position: "absolute",
    left: 7,
    top: 17,
    width: 12,
    height: 1.5,
    backgroundColor: "#a6c8b2",
  },
  boardTraceB: {
    position: "absolute",
    right: 7,
    top: 26,
    width: 16,
    height: 1.5,
    backgroundColor: "#a6c8b2",
  },
  boardPinA: {
    position: "absolute",
    right: 8,
    top: 7,
    width: 4,
    height: 4,
    borderRadius: 3,
    backgroundColor: "#a6c8b2",
  },
  boardPinB: {
    position: "absolute",
    left: 8,
    bottom: 7,
    width: 4,
    height: 4,
    borderRadius: 3,
    backgroundColor: "#a6c8b2",
  },
});

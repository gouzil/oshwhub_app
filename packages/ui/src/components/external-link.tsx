import { openExternal } from "@oshwhub/bridge";
import { useState } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";
import { safeUrl } from "../model";
import { color } from "../theme";

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

const common = StyleSheet.create({
  pressed: { opacity: 0.72 },
  stateBody: { fontSize: 14, color: color.muted, lineHeight: 23, textAlign: "center" },
  errorCode: { color: "#946a58", fontSize: 11, lineHeight: 18, textAlign: "center" },
  link: { minHeight: 44, justifyContent: "center", paddingVertical: 10 },
  linkText: { color: color.green, fontSize: 14, lineHeight: 22 },
});

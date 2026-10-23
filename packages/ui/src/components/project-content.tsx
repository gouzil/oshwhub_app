import type { ContentBlock } from "@oshwhub/bridge";
import { Platform, StyleSheet, Text, View } from "react-native";
import { RemoteImage } from "../images";
import { safeUrl } from "../model";
import { color } from "../theme";
import { ExternalLink } from "./external-link";

export function ProjectContent({ block, visible }: { block: ContentBlock; visible: boolean }) {
  if (block.kind === "image") return <ContentImage block={block} visible={visible} />;
  if (block.kind === "heading")
    return (
      <Text selectable style={styles.contentHeading} accessibilityRole="header">
        {block.text}
      </Text>
    );
  if (block.kind === "code")
    return (
      <View style={styles.code}>
        <Text selectable style={styles.codeText}>
          {block.text}
        </Text>
      </View>
    );
  if (block.kind === "link" && block.url)
    return <ExternalLink url={block.url} label={block.text || "打开链接"} />;
  return (
    <Text selectable style={styles.contentText}>
      {block.text}
    </Text>
  );
}

function ContentImage({ block, visible }: { block: ContentBlock; visible: boolean }) {
  const uri = safeUrl(block.url);
  if (!uri)
    return (
      <Text style={styles.imageError}>图片暂时无法显示{block.text ? `：${block.text}` : ""}</Text>
    );
  return (
    <View style={{ gap: 4 }}>
      <RemoteImage
        url={uri}
        label={block.text || "项目说明图片"}
        enabled={visible}
        natural
        style={styles.contentImage}
      />
      <ExternalLink url={uri} label={`查看原图：${block.text || "项目说明图片"}`} />
    </View>
  );
}

const styles = StyleSheet.create({
  contentText: { color: "#42574b", fontSize: 15, lineHeight: 27 },
  contentHeading: {
    color: color.ink,
    fontWeight: "600",
    fontSize: 21,
    lineHeight: 31,
    marginTop: 12,
  },
  contentImage: { maxWidth: 960, alignSelf: "center" },
  imageError: {
    padding: 20,
    color: color.muted,
    fontSize: 13,
    backgroundColor: color.canvas,
    borderRadius: 10,
  },
  code: { padding: 16, backgroundColor: "#f2f5f2", borderRadius: 10 },
  codeText: {
    color: "#45574b",
    fontSize: 12,
    lineHeight: 21,
    fontFamily: Platform.OS === "ios" ? "Menlo" : "monospace",
  },
});

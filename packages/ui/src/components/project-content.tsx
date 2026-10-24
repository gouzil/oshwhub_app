import type { ContentBlock } from "@oshwhub/bridge";
import { Platform, View } from "react-native";
import { RemoteImage } from "../images";
import { cn } from "../lib/utils";
import { safeUrl } from "../model";
import { ExternalLink } from "./external-link";
import { Text } from "./ui/text";

export function ProjectContent({ block, visible }: { block: ContentBlock; visible: boolean }) {
  if (block.kind === "image") return <ContentImage block={block} visible={visible} />;
  if (block.kind === "heading")
    return (
      <Text
        selectable
        className="mt-3 text-[21px] font-semibold leading-[31px]"
        accessibilityRole="header"
      >
        {block.text}
      </Text>
    );
  if (block.kind === "code")
    return (
      <View className="rounded-[10px] bg-[#f2f5f2] p-4">
        <Text
          selectable
          className={cn(
            "text-xs leading-[21px] text-[#45574b]",
            Platform.OS === "ios" ? "font-[Menlo]" : "font-[monospace]",
          )}
        >
          {block.text}
        </Text>
      </View>
    );
  if (block.kind === "link" && block.url)
    return <ExternalLink url={block.url} label={block.text || "打开链接"} />;
  return (
    <Text selectable className="text-[15px] leading-[27px] text-[#42574b]">
      {block.text}
    </Text>
  );
}

function ContentImage({ block, visible }: { block: ContentBlock; visible: boolean }) {
  const uri = safeUrl(block.url);
  if (!uri)
    return (
      <Text className="rounded-[10px] bg-canvas p-5 text-[13px] text-muted-foreground">
        图片暂时无法显示{block.text ? `：${block.text}` : ""}
      </Text>
    );
  return (
    <View className="gap-1">
      <RemoteImage
        url={uri}
        label={block.text || "项目说明图片"}
        enabled={visible}
        natural
        className="max-w-[960px] self-center"
      />
      <ExternalLink url={uri} label={`查看原图：${block.text || "项目说明图片"}`} />
    </View>
  );
}

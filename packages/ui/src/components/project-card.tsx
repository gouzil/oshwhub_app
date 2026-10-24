import type { ProjectSummary } from "@oshwhub/bridge";
import { Image, Pressable, View } from "react-native";
import { safeUrl } from "../model";
import { Cover } from "./cover";
import { Badge } from "./ui/badge";
import { Text } from "./ui/text";

export function ProjectCard({
  project,
  width,
  onPress,
}: {
  project: ProjectSummary;
  width: number;
  onPress: () => void;
}) {
  const avatar = safeUrl(project.authorAvatar);
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={`${project.name}，作者 ${project.author}，${project.likes} 个喜欢。查看项目详情`}
      onPress={onPress}
      className="min-w-0 self-start active:opacity-[0.72]"
      style={{ width }}
    >
      <View className="overflow-hidden rounded-[13px] bg-canvas">
        <Cover url={project.cover} title={project.name} />
        {project.tags[0] && (
          <Badge
            variant="outline"
            className="absolute bottom-2 left-2 max-w-[85%] rounded-[5px] bg-[#ffffffed] px-[7px] py-[3px]"
          >
            <Text numberOfLines={1} className="text-[9px] text-[#355443]">
              {project.tags[0]}
            </Text>
          </Badge>
        )}
      </View>
      <Text numberOfLines={2} className="mt-2.5 min-h-[42px] text-sm font-semibold leading-[21px]">
        {project.name}
      </Text>
      <View className="mb-1.5 mt-2 flex-row items-center gap-[5px]">
        <View className="min-w-0 flex-1 flex-row items-center gap-[5px]">
          {avatar ? (
            <Image source={{ uri: avatar }} className="size-[18px] rounded-full bg-secondary" />
          ) : (
            <View className="size-[18px] items-center justify-center rounded-full bg-[#e6eee8]">
              <Text className="text-[9px] text-[#6a8c74]">
                {project.author.slice(0, 1) || "创"}
              </Text>
            </View>
          )}
          <Text numberOfLines={1} className="shrink text-[10px] text-muted-foreground">
            {project.author || "开源创作者"}
          </Text>
        </View>
        <Text numberOfLines={1} className="max-w-[48%] shrink text-[10px] text-[#819087]">
          ♡ {project.likes.toLocaleString()}
        </Text>
      </View>
    </Pressable>
  );
}

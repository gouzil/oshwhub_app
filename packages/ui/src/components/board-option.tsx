import type { Board } from "@oshwhub/bridge";
import { Pressable, View } from "react-native";
import { RemoteImage } from "../images";
import { cn } from "../lib/utils";
import { safeUrl } from "../model";
import { BoardMark } from "./board-mark";
import { Text } from "./ui/text";

export function BoardOption({
  board,
  selected,
  onPress,
}: {
  board: Board;
  selected: boolean;
  onPress: () => void;
}) {
  const thumbnail = safeUrl(board.thumbnail);
  return (
    <Pressable
      accessibilityRole="radio"
      aria-checked={selected}
      accessibilityLabel={`选择板子 ${board.name}`}
      onPress={onPress}
      className={cn(
        "min-h-[154px] w-40 gap-2 rounded-xl border border-border p-3",
        selected && "border-primary bg-[#f6fcf7]",
      )}
    >
      <View className="h-[62px] items-center justify-center overflow-hidden">
        {thumbnail ? (
          <RemoteImage url={thumbnail} label={`${board.name}预览`} aspectRatio={2.2} />
        ) : (
          <BoardMark small />
        )}
      </View>
      <Text numberOfLines={2} className="text-[13px] font-medium leading-5">
        {board.name || "未命名板子"}
      </Text>
      <Text className="text-[11px] text-muted-foreground">{board.bom.length} 种物料</Text>
    </Pressable>
  );
}

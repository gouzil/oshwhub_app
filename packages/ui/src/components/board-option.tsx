import type { Board } from "@oshwhub/bridge";
import { Pressable, StyleSheet, Text, View } from "react-native";
import { RemoteImage } from "../images";
import { safeUrl } from "../model";
import { color } from "../theme";
import { BoardMark } from "./board-mark";

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
      style={[styles.boardOption, selected && styles.boardOptionSelected]}
    >
      <View style={styles.boardThumbnail}>
        {thumbnail ? (
          <RemoteImage url={thumbnail} label={`${board.name}预览`} aspectRatio={2.2} />
        ) : (
          <BoardMark small />
        )}
      </View>
      <Text numberOfLines={2} style={styles.boardOptionName}>
        {board.name || "未命名板子"}
      </Text>
      <Text style={styles.boardOptionCount}>{board.bom.length} 种物料</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  boardOption: {
    width: 160,
    minHeight: 154,
    padding: 12,
    borderWidth: 1,
    borderColor: color.line,
    borderRadius: 12,
    gap: 8,
  },
  boardOptionSelected: { borderColor: color.green, backgroundColor: "#f6fcf7" },
  boardThumbnail: {
    height: 62,
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  },
  boardOptionName: { color: color.ink, fontSize: 13, lineHeight: 20, fontWeight: "500" },
  boardOptionCount: { color: color.muted, fontSize: 11 },
});

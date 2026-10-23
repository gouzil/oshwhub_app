import { StyleSheet, Text, View } from "react-native";
import { RemoteImage } from "../images";
import { safeUrl } from "../model";
import { BoardMark } from "./board-mark";

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

const common = StyleSheet.create({
  cover: { width: "100%", overflow: "hidden", backgroundColor: "#eaf0eb" },
  coverFallback: { flex: 1, alignItems: "center", justifyContent: "center", gap: 12 },
  coverCaption: { fontSize: 10, color: "#6e8b78", letterSpacing: 2 },
});

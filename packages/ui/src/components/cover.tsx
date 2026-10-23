import { View } from "react-native";
import { RemoteImage } from "../images";
import { safeUrl } from "../model";
import { BoardMark } from "./board-mark";
import { Text } from "./ui/text";

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
    <View className="flex-1 items-center justify-center gap-3">
      <BoardMark />
      <Text className="text-[10px] tracking-[2px] text-[#6e8b78]">开源硬件项目</Text>
    </View>
  );
  return (
    <View className="w-full overflow-hidden bg-[#eaf0eb]" style={{ aspectRatio }}>
      {uri ? (
        <RemoteImage url={uri} label={title} aspectRatio={aspectRatio} resizeMode="cover" />
      ) : (
        fallback
      )}
    </View>
  );
}

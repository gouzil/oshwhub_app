import { View } from "react-native";
import { cn } from "../lib/utils";

export function BoardMark({ small = false }: { small?: boolean }) {
  return (
    <View
      accessible={false}
      className={cn(
        "h-12 w-16 -rotate-[7deg] rounded-[7px] border-[1.5px] border-[#a6c8b2]",
        small && "h-11 w-[58px]",
      )}
    >
      <View className="absolute left-5 top-[13px] size-[18px] rounded-[3px] border-[1.5px] border-[#89b39b] bg-[#d6e7db]" />
      <View className="absolute left-[7px] top-[17px] h-[1.5px] w-3 bg-[#a6c8b2]" />
      <View className="absolute right-[7px] top-[26px] h-[1.5px] w-4 bg-[#a6c8b2]" />
      <View className="absolute right-2 top-[7px] size-1 rounded-[3px] bg-[#a6c8b2]" />
      <View className="absolute bottom-[7px] left-2 size-1 rounded-[3px] bg-[#a6c8b2]" />
    </View>
  );
}

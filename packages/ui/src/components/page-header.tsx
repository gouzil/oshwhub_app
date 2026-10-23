import { View } from "react-native";
import { Button } from "./ui/button";
import { Text } from "./ui/text";

export function PageHeader({ title, onBack }: { title: string; onBack: () => void }) {
  return (
    <View className="min-h-[62px] flex-row items-center gap-3 border-b border-border px-3">
      <Button
        variant="ghost"
        accessibilityLabel="返回上一页"
        onPress={onBack}
        className="min-h-11 min-w-16 gap-[3px] rounded-[10px] py-0 pl-0 pr-2 active:opacity-[0.72]"
      >
        <Text className="text-[34px] font-normal leading-[38px] text-primary">‹</Text>
        <Text className="text-[13px] font-normal text-primary">返回</Text>
      </Button>
      <Text
        numberOfLines={1}
        accessibilityRole="header"
        className="min-w-0 flex-1 text-[15px] font-semibold"
      >
        {title}
      </Text>
    </View>
  );
}

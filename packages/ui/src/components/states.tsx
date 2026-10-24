import { ActivityIndicator, View } from "react-native";
import { ActionButton as Button } from "./action-button";
import { BoardMark } from "./board-mark";
import { Card } from "./ui/card";
import { Text } from "./ui/text";

export function ErrorPanel({ error, retry }: { error: Error; retry: () => void }) {
  const details = error as Error & { code?: string; requestId?: string };
  return (
    <Card
      className="my-5 items-center gap-3.5 rounded-2xl border-[#efe2dc] bg-[#fffaf7] p-6 shadow-none"
      accessibilityRole="alert"
    >
      <Text className="text-center text-lg font-semibold">暂时无法加载</Text>
      <Text selectable className="text-center text-sm leading-[23px] text-muted-foreground">
        {error.message}
      </Text>
      {(details.code || details.requestId) && (
        <Text selectable className="text-center text-[11px] leading-[18px] text-[#946a58]">
          {[details.code, details.requestId].filter(Boolean).join(" · ")}
        </Text>
      )}
      <Button label="重新加载" secondary onPress={retry} />
    </Card>
  );
}

export function Loading({ label = "正在加载" }: { label?: string }) {
  return (
    <View className="grow items-center justify-center gap-3.5 px-6 py-12">
      <ActivityIndicator colorClassName="accent-primary" accessibilityLabel={label} />
      <Text className="text-center text-sm leading-[23px] text-muted-foreground">{label}</Text>
    </View>
  );
}

export function Empty({ title, detail }: { title: string; detail: string }) {
  return (
    <View className="grow items-center justify-center gap-3.5 px-6 py-12">
      <BoardMark small />
      <Text className="text-center text-lg font-semibold">{title}</Text>
      <Text className="text-center text-sm leading-[23px] text-muted-foreground">{detail}</Text>
    </View>
  );
}

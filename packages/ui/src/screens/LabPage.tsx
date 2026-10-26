import { ScrollView, View } from "react-native";
import { ActionButton as Button } from "../components/action-button";
import { BoardMark } from "../components/board-mark";
import { Badge } from "../components/ui/badge";
import { Card } from "../components/ui/card";
import { Text } from "../components/ui/text";

export function LabPage({ onOpen, bottomInset }: { onOpen: () => void; bottomInset: number }) {
  return (
    <ScrollView
      contentContainerClassName="w-full max-w-[800px] grow self-center gap-4 p-7"
      contentContainerStyle={{ paddingBottom: 28 + bottomInset }}
    >
      <Text className="mb-2.5 text-[10px] leading-6 font-bold tracking-[1.4px] text-primary">
        PCB LAB
      </Text>
      <Text className="text-[28px] leading-[37px] font-bold tracking-[-0.7px] text-foreground">
        看清每一个连接
      </Text>
      <Text className="text-[14px] leading-[25px] text-muted-foreground">
        在项目详情中选择板子，即可切换二维 / 三维，查看图层、拾取图元或旋转板体。
      </Text>
      <Card className="mt-4 gap-[18px] rounded-[20px] bg-card p-6">
        <View className="h-[120px] items-center justify-center rounded-[14px] bg-secondary">
          <BoardMark />
        </View>
        <Text className="text-[20px] leading-6 font-semibold text-foreground">
          先试试 PCB 查看器
        </Text>
        <Badge variant="secondary" className="self-start rounded-md px-2.5 py-1.5">
          <Text className="text-xs">本地自制示例</Text>
        </Badge>
        <Text className="text-[14px] leading-[25px] text-muted-foreground">
          一块用于演示交互的示例板，支持平移、缩放、图层切换与属性查看。
        </Text>
        <Button label="打开示例板" onPress={onOpen} />
        <Text className="text-[11px] leading-[19px] text-muted-foreground">
          示例由应用内置生成，不代表在线项目。
        </Text>
      </Card>
    </ScrollView>
  );
}

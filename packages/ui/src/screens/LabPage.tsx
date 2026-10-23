import { ScrollView, StyleSheet, View } from "react-native";
import { ActionButton as Button } from "../components/action-button";
import { BoardMark } from "../components/board-mark";
import { Badge } from "../components/ui/badge";
import { Card } from "../components/ui/card";
import { Text } from "../components/ui/text";
import { color } from "../theme";

export function LabPage({ onOpen, bottomInset }: { onOpen: () => void; bottomInset: number }) {
  return (
    <ScrollView contentContainerStyle={[styles.staticPage, { paddingBottom: 28 + bottomInset }]}>
      <Text style={styles.eyebrow}>PCB LAB</Text>
      <Text style={styles.title}>看清每一个连接</Text>
      <Text style={styles.staticDescription}>
        在项目详情中选择板子，即可切换二维 / 三维，查看图层、拾取图元或旋转板体。
      </Text>
      <Card className="mt-4 gap-[18px] rounded-[20px] bg-[#fbfdfb] p-6">
        <View style={styles.labDrawing}>
          <BoardMark />
        </View>
        <Text style={styles.labTitle}>先试试 PCB 查看器</Text>
        <Badge variant="secondary" className="self-start rounded-md px-2.5 py-1.5">
          <Text className="text-xs">本地自制示例</Text>
        </Badge>
        <Text style={styles.staticDescription}>
          一块用于演示交互的示例板，支持平移、缩放、图层切换与属性查看。
        </Text>
        <Button label="打开示例板" onPress={onOpen} />
        <Text style={styles.labCaption}>示例由应用内置生成，不代表在线项目。</Text>
      </Card>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  eyebrow: {
    color: color.green,
    fontSize: 10,
    fontWeight: "700",
    letterSpacing: 1.4,
    marginBottom: 10,
  },
  title: { fontSize: 28, lineHeight: 37, color: color.ink, fontWeight: "700", letterSpacing: -0.7 },
  staticPage: {
    flexGrow: 1,
    padding: 28,
    gap: 16,
    width: "100%",
    maxWidth: 800,
    alignSelf: "center",
  },
  staticDescription: { fontSize: 14, lineHeight: 25, color: color.muted },
  labDrawing: {
    height: 120,
    backgroundColor: color.greenLight,
    borderRadius: 14,
    alignItems: "center",
    justifyContent: "center",
  },
  labTitle: { fontSize: 20, fontWeight: "600", color: color.ink },
  labCaption: { fontSize: 11, lineHeight: 19, color: "#99a39c" },
});

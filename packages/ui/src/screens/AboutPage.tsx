import { ScrollView, View } from "react-native";
import { ExternalLink } from "../components/external-link";
import { Text } from "../components/ui/text";

export function AboutPage({ bottomInset }: { bottomInset: number }) {
  return (
    <ScrollView
      contentContainerClassName="w-full max-w-[800px] grow self-center gap-4 p-7"
      contentContainerStyle={{ paddingBottom: 28 + bottomInset }}
    >
      <View className="mt-5 mb-2.5 size-[58px] items-center justify-center rounded-[18px] bg-primary">
        <Text className="text-[40px] text-primary-foreground">▦</Text>
      </View>
      <Text className="text-[28px] leading-[37px] font-bold tracking-[-0.7px] text-foreground">
        OSHWHub
      </Text>
      <Text className="text-[14px] leading-[25px] text-muted-foreground">
        随时发现开源硬件，随处探索电路设计。
      </Text>
      <View className="mt-5 gap-3 border-t border-border pt-5">
        <Text className="text-[17px] font-semibold text-foreground">从创意到电路</Text>
        <Text className="text-[14px] leading-[25px] text-muted-foreground">
          浏览公开项目、阅读制作说明、查看物料清单，在二维 / 三维画布中探索
          PCB。项目内容与署名属于原作者，请遵守各项目的开源许可。
        </Text>
        <ExternalLink url="https://oshwhub.com" label="前往 OSHWHub 网站" />
      </View>
      <View className="mt-5 gap-3 border-t border-border pt-5">
        <Text className="text-[17px] font-semibold text-foreground">关于此客户端</Text>
        <Text className="text-[14px] leading-[25px] text-muted-foreground">
          社区开源客户端。当前提供公开内容浏览；账号与创作功能将在后续版本中逐步加入。
        </Text>
        <ExternalLink url="https://github.com/gouzil/oshwhub_app" label="查看项目源码" />
      </View>
      <Text className="text-[11px] leading-[19px] text-muted-foreground">
        v0.1.0 · 为每一位硬件创造者而做
      </Text>
    </ScrollView>
  );
}

import { ScrollView, StyleSheet, Text, View } from "react-native";
import { ExternalLink } from "../components/external-link";
import { color } from "../theme";

export function AboutPage({ bottomInset }: { bottomInset: number }) {
  return (
    <ScrollView contentContainerStyle={[styles.staticPage, { paddingBottom: 28 + bottomInset }]}>
      <View style={styles.aboutLogo}>
        <Text style={styles.aboutLogoText}>▦</Text>
      </View>
      <Text style={styles.title}>OSHWHub</Text>
      <Text style={styles.staticDescription}>随时发现开源硬件，随处探索电路设计。</Text>
      <View style={styles.aboutSection}>
        <Text style={styles.sectionTitle}>从创意到电路</Text>
        <Text style={styles.staticDescription}>
          浏览公开项目、阅读制作说明、查看物料清单，在二维 / 三维画布中探索
          PCB。项目内容与署名属于原作者，请遵守各项目的开源许可。
        </Text>
        <ExternalLink url="https://oshwhub.com" label="前往 OSHWHub 网站" />
      </View>
      <View style={styles.aboutSection}>
        <Text style={styles.sectionTitle}>关于此客户端</Text>
        <Text style={styles.staticDescription}>
          社区开源客户端。当前提供公开内容浏览；账号与创作功能将在后续版本中逐步加入。
        </Text>
        <ExternalLink url="https://github.com/gouzil/oshwhub_app" label="查看项目源码" />
      </View>
      <Text style={styles.labCaption}>v0.1.0 · 为每一位硬件创造者而做</Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
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
  labCaption: { fontSize: 11, lineHeight: 19, color: "#99a39c" },
  aboutLogo: {
    width: 58,
    height: 58,
    borderRadius: 18,
    backgroundColor: color.green,
    alignItems: "center",
    justifyContent: "center",
    marginTop: 20,
    marginBottom: 10,
  },
  aboutLogoText: { fontSize: 40, color: "#fff" },
  aboutSection: {
    marginTop: 20,
    paddingTop: 20,
    borderTopWidth: 1,
    borderTopColor: color.line,
    gap: 12,
  },
  sectionTitle: { fontSize: 17, fontWeight: "600", color: color.ink },
});

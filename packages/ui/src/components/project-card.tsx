import type { ProjectSummary } from "@oshwhub/bridge";
import { Image, Pressable, StyleSheet, View } from "react-native";
import { safeUrl } from "../model";
import { color } from "../theme";
import { Cover } from "./cover";
import { Badge } from "./ui/badge";
import { Text } from "./ui/text";

export function ProjectCard({
  project,
  width,
  onPress,
}: {
  project: ProjectSummary;
  width: number;
  onPress: () => void;
}) {
  const avatar = safeUrl(project.authorAvatar);
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={`${project.name}，作者 ${project.author}，${project.likes} 个喜欢。查看项目详情`}
      onPress={onPress}
      style={({ pressed }) => [styles.card, { width }, pressed && { opacity: 0.72 }]}
    >
      <View style={styles.cardCover}>
        <Cover url={project.cover} title={project.name} />
        {project.tags[0] && (
          <Badge variant="outline" style={styles.cardTag}>
            <Text numberOfLines={1} style={styles.cardTagText}>
              {project.tags[0]}
            </Text>
          </Badge>
        )}
      </View>
      <Text numberOfLines={2} style={styles.cardTitle}>
        {project.name}
      </Text>
      <View style={styles.cardMeta}>
        <View style={styles.author}>
          {avatar ? (
            <Image source={{ uri: avatar }} style={styles.avatar} />
          ) : (
            <View style={styles.avatarFallback}>
              <Text style={styles.avatarLetter}>{project.author.slice(0, 1) || "创"}</Text>
            </View>
          )}
          <Text numberOfLines={1} style={styles.authorText}>
            {project.author || "开源创作者"}
          </Text>
        </View>
        <Text numberOfLines={1} style={styles.likes}>
          ♡ {project.likes.toLocaleString()}
        </Text>
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  card: { alignSelf: "flex-start", minWidth: 0 },
  cardCover: { borderRadius: 13, overflow: "hidden", backgroundColor: color.canvas },
  cardTag: {
    position: "absolute",
    left: 8,
    bottom: 8,
    maxWidth: "85%",
    paddingVertical: 3,
    paddingHorizontal: 7,
    borderRadius: 5,
    backgroundColor: "#ffffffed",
  },
  cardTagText: { color: "#355443", fontSize: 9 },
  cardTitle: {
    fontSize: 14,
    fontWeight: "600",
    color: color.ink,
    lineHeight: 21,
    marginTop: 10,
    minHeight: 42,
  },
  cardMeta: { flexDirection: "row", gap: 5, alignItems: "center", marginTop: 8, marginBottom: 6 },
  author: { flex: 1, minWidth: 0, flexDirection: "row", alignItems: "center", gap: 5 },
  avatar: { width: 18, height: 18, borderRadius: 9, backgroundColor: color.greenLight },
  avatarFallback: {
    width: 18,
    height: 18,
    borderRadius: 9,
    backgroundColor: "#e6eee8",
    alignItems: "center",
    justifyContent: "center",
  },
  avatarLetter: { fontSize: 9, color: "#6a8c74" },
  authorText: { flexShrink: 1, fontSize: 10, color: color.muted },
  likes: { color: "#819087", fontSize: 10, maxWidth: "48%", flexShrink: 1 },
});

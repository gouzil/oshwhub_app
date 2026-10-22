import type { Board, ContentBlock, ProjectDetail } from "@oshwhub/bridge";
import { useMemo, useState } from "react";
import { Image, Platform, Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import {
  BoardMark,
  Button,
  Cover,
  color,
  Empty,
  ErrorPanel,
  ExternalLink,
  Loading,
  PageHeader,
  useCommand,
} from "./common";
import { safeUrl } from "./model";

export function ProjectPage({
  id,
  width,
  onBack,
  onPcb,
}: {
  id: string;
  width: number;
  onBack: () => void;
  onPcb: (documentId: string, name: string) => void;
}) {
  const command = useMemo(() => ({ command: "project" as const, id }), [id]);
  const { value: project, loading, error, retry } = useCommand<ProjectDetail>(command);
  const [tab, setTab] = useState<"intro" | "hardware">("intro");
  const [boardId, setBoardId] = useState<string | null>(null);
  const selected = project?.boards.find((board) => board.id === boardId) ?? project?.boards[0];
  const wide = width >= 900;
  return (
    <View style={styles.root}>
      <PageHeader title={project?.summary.name ?? "项目详情"} onBack={onBack} />
      {loading ? (
        <Loading label="正在打开项目" />
      ) : error ? (
        <ScrollView contentContainerStyle={styles.errorContent}>
          <ErrorPanel error={error} retry={retry} />
        </ScrollView>
      ) : (
        project && (
          <ScrollView contentContainerStyle={[styles.content, { padding: width < 400 ? 16 : 28 }]}>
            <View style={[styles.hero, wide && styles.heroWide]}>
              <View style={[styles.heroCover, wide && styles.heroCoverWide]}>
                <Cover
                  url={project.summary.cover}
                  title={project.summary.name}
                  aspectRatio={wide ? 1.35 : 1.7}
                />
              </View>
              <View style={styles.heroInfo}>
                <Text style={styles.projectKicker}>开源项目</Text>
                <Text selectable style={styles.title}>
                  {project.summary.name}
                </Text>
                <Text style={styles.author}>由 {project.summary.author || "开源创作者"} 分享</Text>
                {project.summary.description.length > 0 && (
                  <Text selectable style={styles.description}>
                    {project.summary.description}
                  </Text>
                )}
                <View style={styles.tags}>
                  {[...new Set(project.summary.tags)].map((tag) => (
                    <View key={tag} style={styles.tag}>
                      <Text style={styles.tagText}>{tag}</Text>
                    </View>
                  ))}
                </View>
                <Text style={styles.stats}>
                  ♡ {project.summary.likes.toLocaleString()} 喜欢{" "}
                  {project.summary.views.toLocaleString()} 次浏览
                </Text>
                <ExternalLink url={project.summary.url} label="在原网站中查看" />
              </View>
            </View>
            <View style={styles.tabs} accessibilityRole="tablist">
              {(
                [
                  { id: "intro", label: "项目介绍" },
                  { id: "hardware", label: `板子与 BOM (${project.boards.length})` },
                ] as const
              ).map((item) => (
                <Pressable
                  key={item.id}
                  accessibilityRole="tab"
                  aria-selected={item.id === tab}
                  onPress={() => setTab(item.id)}
                  style={[styles.tab, item.id === tab && styles.tabActive]}
                >
                  <Text style={[styles.tabText, item.id === tab && styles.tabTextActive]}>
                    {item.label}
                  </Text>
                </Pressable>
              ))}
            </View>
            {tab === "intro" ? (
              <View style={styles.article}>
                {project.content.length ? (
                  project.content.map((block, i) => (
                    <Content key={`${i}-${block.kind}`} block={block} />
                  ))
                ) : (
                  <Empty title="作者尚未添加项目正文" detail="可以前往原网站查看项目的最新信息。" />
                )}
                {project.attachments.length > 0 && (
                  <View style={styles.attachments}>
                    <Text style={styles.sectionTitle}>项目附件</Text>
                    {project.attachments.map((attachment, i) => (
                      <ExternalLink
                        key={`${attachment.url}-${i}`}
                        url={attachment.url}
                        label={attachment.name || "打开附件"}
                      />
                    ))}
                  </View>
                )}
                {project.boards.length > 0 && (
                  <Button label="查看板子与物料清单" secondary onPress={() => setTab("hardware")} />
                )}
              </View>
            ) : (
              <View style={styles.hardware}>
                {project.boards.length ? (
                  <>
                    <Text style={styles.sectionTitle}>选择板子</Text>
                    <ScrollView
                      horizontal
                      showsHorizontalScrollIndicator={false}
                      contentContainerStyle={styles.boardOptions}
                    >
                      {project.boards.map((board) => (
                        <BoardOption
                          key={board.id}
                          board={board}
                          selected={selected?.id === board.id}
                          onPress={() => setBoardId(board.id)}
                        />
                      ))}
                    </ScrollView>
                    {selected && (
                      <View style={styles.boardPanel}>
                        <View style={styles.boardPanelHeading}>
                          <View style={styles.boardPanelName}>
                            <Text style={styles.sectionTitle}>{selected.name || "未命名板子"}</Text>
                            <Text style={styles.boardCount}>
                              {selected.bom.length} 种物料 ·{" "}
                              {selected.bom.reduce((sum, item) => sum + item.quantity, 0)} 个元件
                            </Text>
                          </View>
                          <Button
                            label={selected.pcbId ? "查看 PCB" : "未公开 PCB 文件"}
                            disabled={!selected.pcbId}
                            onPress={() => {
                              if (selected.pcbId) onPcb(selected.pcbId, selected.name);
                            }}
                          />
                        </View>
                        <Text style={styles.bomHeading}>物料清单 / BOM</Text>
                        {selected.bom.length ? (
                          selected.bom.map((item, i) => (
                            <View key={`${item.designator}-${i}`} style={styles.bomRow}>
                              <View style={styles.bomRowHeader}>
                                <Text selectable style={styles.designator}>
                                  {item.designator || item.name || "元件"}
                                </Text>
                                <Text style={styles.quantity}>× {item.quantity}</Text>
                              </View>
                              <Text selectable style={styles.bomText}>
                                {[item.name, item.footprint].filter(Boolean).join(" · ") ||
                                  "未提供型号 / 封装"}
                              </Text>
                              {(item.partNumber || item.supplierPart) && (
                                <Text selectable style={styles.partNumber}>
                                  {[item.partNumber, item.supplierPart].filter(Boolean).join(" / ")}
                                </Text>
                              )}
                            </View>
                          ))
                        ) : (
                          <Empty title="暂无物料清单" detail="作者尚未公开此板的 BOM。" />
                        )}
                      </View>
                    )}
                  </>
                ) : (
                  <Empty title="暂无公开板子" detail="作者尚未公开板子与物料清单。" />
                )}
              </View>
            )}
          </ScrollView>
        )
      )}
    </View>
  );
}

function BoardOption({
  board,
  selected,
  onPress,
}: {
  board: Board;
  selected: boolean;
  onPress: () => void;
}) {
  const thumbnail = safeUrl(board.thumbnail);
  return (
    <Pressable
      accessibilityRole="radio"
      aria-checked={selected}
      accessibilityLabel={`选择板子 ${board.name}`}
      onPress={onPress}
      style={[styles.boardOption, selected && styles.boardOptionSelected]}
    >
      <View style={styles.boardThumbnail}>
        {thumbnail ? (
          <Image source={{ uri: thumbnail }} resizeMode="contain" style={StyleSheet.absoluteFill} />
        ) : (
          <BoardMark small />
        )}
      </View>
      <Text numberOfLines={2} style={styles.boardOptionName}>
        {board.name || "未命名板子"}
      </Text>
      <Text style={styles.boardOptionCount}>{board.bom.length} 种物料</Text>
    </Pressable>
  );
}

function Content({ block }: { block: ContentBlock }) {
  if (block.kind === "image") return <ContentImage block={block} />;
  if (block.kind === "heading")
    return (
      <Text selectable style={styles.contentHeading} accessibilityRole="header">
        {block.text}
      </Text>
    );
  if (block.kind === "code")
    return (
      <View style={styles.code}>
        <Text selectable style={styles.codeText}>
          {block.text}
        </Text>
      </View>
    );
  if (block.kind === "link" && block.url)
    return <ExternalLink url={block.url} label={block.text || "打开链接"} />;
  return (
    <Text selectable style={styles.contentText}>
      {block.text}
    </Text>
  );
}

function ContentImage({ block }: { block: ContentBlock }) {
  const [failed, setFailed] = useState(false);
  const [ratio, setRatio] = useState(1.6);
  const uri = safeUrl(block.url);
  if (!uri || failed)
    return (
      <Text style={styles.imageError}>图片暂时无法显示{block.text ? `：${block.text}` : ""}</Text>
    );
  return (
    <Image
      source={{ uri }}
      accessibilityLabel={block.text || "项目说明图片"}
      resizeMode="contain"
      onError={() => setFailed(true)}
      onLoad={({ nativeEvent }) => {
        const { width, height } = nativeEvent.source;
        if (width > 0 && height > 0) setRatio(width / height);
      }}
      style={[styles.contentImage, { aspectRatio: ratio }]}
    />
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, minHeight: 0 },
  errorContent: { padding: 20, flexGrow: 1, justifyContent: "center" },
  content: { width: "100%", maxWidth: 1120, alignSelf: "center", paddingBottom: 40 },
  hero: { gap: 24 },
  heroWide: { flexDirection: "row", alignItems: "flex-start", gap: 32 },
  heroCover: { overflow: "hidden", borderRadius: 18, width: "100%" },
  heroCoverWide: { width: "44%" },
  heroInfo: { flex: 1, minWidth: 0, gap: 12 },
  projectKicker: { fontSize: 11, color: color.green, fontWeight: "600", letterSpacing: 1 },
  title: { color: color.ink, fontSize: 25, lineHeight: 35, fontWeight: "700" },
  author: { color: color.muted, fontSize: 13 },
  description: { color: "#56695e", fontSize: 14, lineHeight: 24 },
  tags: { flexDirection: "row", flexWrap: "wrap", gap: 6 },
  tag: {
    paddingVertical: 5,
    paddingHorizontal: 9,
    backgroundColor: color.greenLight,
    borderRadius: 6,
  },
  tagText: { color: "#568065", fontSize: 11 },
  stats: { color: "#8a978e", fontSize: 12 },
  tabs: {
    flexDirection: "row",
    borderBottomWidth: 1,
    borderBottomColor: color.line,
    marginTop: 28,
    gap: 14,
    flexWrap: "wrap",
  },
  tab: {
    minHeight: 52,
    justifyContent: "center",
    paddingHorizontal: 6,
    borderBottomWidth: 2,
    borderBottomColor: "transparent",
  },
  tabActive: { borderBottomColor: color.green },
  tabText: { color: color.muted, fontSize: 14 },
  tabTextActive: { color: color.green, fontWeight: "600" },
  article: { paddingTop: 24, gap: 18 },
  contentText: { color: "#42574b", fontSize: 15, lineHeight: 27 },
  contentHeading: {
    color: color.ink,
    fontWeight: "600",
    fontSize: 21,
    lineHeight: 31,
    marginTop: 12,
  },
  contentImage: { width: "100%", backgroundColor: "#f8faf8", borderRadius: 10 },
  imageError: {
    padding: 20,
    color: color.muted,
    fontSize: 13,
    backgroundColor: color.canvas,
    borderRadius: 10,
  },
  code: { padding: 16, backgroundColor: "#f2f5f2", borderRadius: 10 },
  codeText: {
    color: "#45574b",
    fontSize: 12,
    lineHeight: 21,
    fontFamily: Platform.OS === "ios" ? "Menlo" : "monospace",
  },
  attachments: { paddingTop: 20, borderTopWidth: 1, borderTopColor: color.line },
  sectionTitle: { color: color.ink, fontSize: 17, lineHeight: 26, fontWeight: "600" },
  hardware: { paddingTop: 24, gap: 16 },
  boardOptions: { gap: 12, paddingVertical: 4 },
  boardOption: {
    width: 160,
    minHeight: 154,
    padding: 12,
    borderWidth: 1,
    borderColor: color.line,
    borderRadius: 12,
    gap: 8,
  },
  boardOptionSelected: { borderColor: color.green, backgroundColor: "#f6fcf7" },
  boardThumbnail: {
    height: 62,
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  },
  boardOptionName: { color: color.ink, fontSize: 13, lineHeight: 20, fontWeight: "500" },
  boardOptionCount: { color: color.muted, fontSize: 11 },
  boardPanel: { borderTopWidth: 1, borderTopColor: color.line, paddingTop: 16 },
  boardPanelHeading: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 16,
    flexWrap: "wrap",
    marginBottom: 26,
  },
  boardPanelName: { flexShrink: 1, gap: 6 },
  boardCount: { color: color.muted, fontSize: 12 },
  bomHeading: { fontSize: 12, color: color.muted, letterSpacing: 0.5, marginBottom: 8 },
  bomRow: { borderBottomWidth: 1, borderBottomColor: color.line, paddingVertical: 16, gap: 6 },
  bomRowHeader: {
    flexDirection: "row",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: 12,
  },
  designator: { color: color.ink, fontSize: 14, lineHeight: 22, fontWeight: "600", flex: 1 },
  quantity: { fontSize: 13, color: color.green, lineHeight: 22 },
  bomText: { color: "#66796c", fontSize: 13, lineHeight: 22 },
  partNumber: { color: "#909e95", fontSize: 11, lineHeight: 19 },
});

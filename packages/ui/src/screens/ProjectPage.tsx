import type { ContentBlock, ProjectDetail } from "@oshwhub/bridge";
import { useMemo, useRef, useState } from "react";
import { FlatList, Platform, ScrollView, StyleSheet, Text, View } from "react-native";
import { BomList } from "../BomList";
import { ActionButton as Button } from "../components/action-button";
import { BoardOption } from "../components/board-option";
import { Cover } from "../components/cover";
import { ExternalLink } from "../components/external-link";
import { PageHeader } from "../components/page-header";
import { ProjectContent } from "../components/project-content";
import { Empty, ErrorPanel } from "../components/states";
import { TabBar } from "../components/tab-bar";
import { useCommand } from "../hooks/use-command";
import { Skeleton, SkeletonBlock } from "../loading";
import { color } from "../theme";

export function ProjectPage({
  id,
  width,
  bottomInset,
  onBack,
  onPcb,
}: {
  id: string;
  width: number;
  bottomInset: number;
  onBack: () => void;
  onPcb: (documentId: string, name: string) => void;
}) {
  const command = useMemo(() => ({ command: "project" as const, id }), [id]);
  const { value: project, loading, error, retry } = useCommand<ProjectDetail>(command);
  const [tab, setTab] = useState<"intro" | "hardware">("intro");
  const [boardId, setBoardId] = useState<string | null>(null);
  const list = useRef<FlatList<ContentBlock>>(null);
  const [readyThrough, setReadyThrough] = useState(-1);
  const viewabilityConfig = useRef({ itemVisiblePercentThreshold: 1 }).current;
  const onViewableItemsChanged = useRef(
    ({ viewableItems }: { viewableItems: { index?: number | null }[] }) => {
      const last = Math.max(-1, ...viewableItems.map((item) => item.index ?? -1));
      if (last >= 0) setReadyThrough((previous) => Math.max(previous, last + 1));
    },
  ).current;
  const selected = project?.boards.find((board) => board.id === boardId) ?? project?.boards[0];
  const wide = width >= 900;
  return (
    <View style={styles.root}>
      <PageHeader title={project?.summary.name ?? "项目详情"} onBack={onBack} />
      {loading ? (
        <ScrollView
          contentContainerStyle={[
            styles.content,
            { padding: width < 400 ? 16 : 28, paddingBottom: 40 + bottomInset },
          ]}
        >
          <Skeleton label="正在打开项目" style={{ gap: 24 }}>
            <View style={[styles.hero, wide && styles.heroWide]}>
              <SkeletonBlock
                style={[
                  styles.heroCover,
                  wide && styles.heroCoverWide,
                  { height: wide ? 260 : 190 },
                ]}
              />
              <View style={styles.heroInfo}>
                <SkeletonBlock style={{ width: "30%", height: 12 }} />
                <SkeletonBlock style={{ width: "85%", height: 30 }} />
                <SkeletonBlock style={{ width: "50%" }} />
                <SkeletonBlock />
                <SkeletonBlock style={{ width: "70%" }} />
              </View>
            </View>
            <SkeletonBlock style={{ height: 44, width: "60%" }} />
            <SkeletonBlock />
            <SkeletonBlock />
            <SkeletonBlock style={{ width: "70%" }} />
          </Skeleton>
        </ScrollView>
      ) : error ? (
        <ScrollView
          contentContainerStyle={[styles.errorContent, { paddingBottom: 40 + bottomInset }]}
        >
          <ErrorPanel error={error} retry={retry} />
        </ScrollView>
      ) : (
        project && (
          <FlatList<ContentBlock>
            ref={list}
            testID="project-content-list"
            keyboardShouldPersistTaps="handled"
            style={{ flex: 1 }}
            contentContainerStyle={[
              styles.content,
              { padding: width < 400 ? 16 : 28, paddingBottom: 40 + bottomInset },
            ]}
            data={tab === "intro" ? project.content : []}
            keyExtractor={(item, index) => `${index}-${item.kind}`}
            initialNumToRender={4}
            maxToRenderPerBatch={4}
            windowSize={3}
            removeClippedSubviews={Platform.OS === "android"}
            onViewableItemsChanged={onViewableItemsChanged}
            viewabilityConfig={viewabilityConfig}
            extraData={readyThrough}
            renderItem={({ item, index }) => (
              <View style={styles.articleBlock}>
                <ProjectContent block={item} visible={index <= readyThrough} />
              </View>
            )}
            ListHeaderComponent={
              <>
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
                    <Text style={styles.author}>
                      由 {project.summary.author || "开源创作者"} 分享
                    </Text>
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
                <TabBar
                  items={[
                    { value: "intro", label: "项目介绍" },
                    { value: "hardware", label: `板子与 BOM (${project.boards.length})` },
                  ]}
                  value={tab}
                  onValueChange={(next) => {
                    setTab(next);
                    list.current?.scrollToOffset({ offset: 0, animated: false });
                  }}
                  accessibilityLabel="项目内容"
                  variant="underline"
                  className="mt-7 flex-wrap gap-3.5 border-b border-border"
                />
                {tab === "hardware" && (
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
                                <Text style={styles.sectionTitle}>
                                  {selected.name || "未命名板子"}
                                </Text>
                                <Text style={styles.boardCount}>
                                  {selected.bom.length} 种物料 ·{" "}
                                  {selected.bom.reduce((sum, item) => sum + item.quantity, 0)}{" "}
                                  个元件
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
                              <BomList key={selected.id} items={selected.bom} wide={width >= 720} />
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
              </>
            }
            ListEmptyComponent={
              tab === "intro" ? (
                <Empty title="作者尚未添加项目正文" detail="可以前往原网站查看项目的最新信息。" />
              ) : undefined
            }
            ListFooterComponent={
              tab === "intro" ? (
                <View style={styles.article}>
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
                    <Button
                      label="查看板子与物料清单"
                      secondary
                      onPress={() => {
                        setTab("hardware");
                        list.current?.scrollToOffset({ offset: 0, animated: false });
                      }}
                    />
                  )}
                </View>
              ) : undefined
            }
          />
        )
      )}
    </View>
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
  article: { paddingTop: 24, gap: 18 },
  articleBlock: { paddingTop: 18 },
  attachments: { paddingTop: 20, borderTopWidth: 1, borderTopColor: color.line },
  sectionTitle: { color: color.ink, fontSize: 17, lineHeight: 26, fontWeight: "600" },
  hardware: { paddingTop: 24, gap: 16 },
  boardOptions: { gap: 12, paddingVertical: 4 },
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
});

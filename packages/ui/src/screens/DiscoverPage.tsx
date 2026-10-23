import { call, type FeedPage, type ProjectSummary, type Sort } from "@oshwhub/bridge";
import {
  type ComponentRef,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { FlatList, ScrollView, StyleSheet, Text, type TextInput, View } from "react-native";
import { ActionButton as Button } from "../components/action-button";
import { ProjectCard } from "../components/project-card";
import { SearchField } from "../components/search-field";
import { Empty, ErrorPanel, Loading } from "../components/states";
import { TabBar } from "../components/tab-bar";
import { Skeleton, SkeletonBlock } from "../loading";
import { FeedController, layoutFor } from "../model";
import { color } from "../theme";

const sorts: { value: Sort; label: string }[] = [
  { value: "recommend", label: "为你推荐" },
  { value: "new", label: "最新发布" },
  { value: "like", label: "最多喜欢" },
];

export function DiscoverPage({
  width,
  active,
  onProject,
  bottomInset,
}: {
  width: number;
  active: boolean;
  onProject: (id: string) => void;
  bottomInset: number;
}) {
  const layout = layoutFor(width);
  const [query, setQuery] = useState("");
  const [hasDraft, setHasDraft] = useState(false);
  const latestDraft = useRef("");
  const searchInput = useRef<ComponentRef<typeof TextInput>>(null);
  const [sort, setSort] = useState<Sort>("recommend");
  const controller = useMemo(
    () => new FeedController((request) => call<FeedPage>({ command: "feed", ...request })),
    [],
  );
  const state = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
    controller.getSnapshot,
  );
  const list = useRef<FlatList<ProjectSummary>>(null);
  useEffect(() => {
    void controller.search(query, sort);
    list.current?.scrollToOffset({ offset: 0, animated: false });
  }, [controller, query, sort]);
  const changeDraft = (text: string) => {
    latestDraft.current = text;
    setHasDraft(text.length > 0);
  };
  const search = () => setQuery(latestDraft.current.trim());
  const header = (
    <View>
      <View style={styles.discoverHeading}>
        <Text style={styles.eyebrow}>OSHWHUB · 开源硬件社区</Text>
        <Text style={styles.title}>发现下一个好想法</Text>
        <Text style={styles.subtitle}>探索、学习，与创造者一起把灵感变成现实。</Text>
      </View>
      <View style={styles.searchRow}>
        {/* Keep native edits across list rerenders; clear() is the only write to the input. */}
        <SearchField
          ref={searchInput}
          containerClassName="flex-1"
          accessibilityLabel="搜索开源硬件项目"
          placeholder="搜索项目、开发板、灵感…"
          placeholderTextColor="#8e9993"
          defaultValue={latestDraft.current}
          onChangeText={changeDraft}
          onSubmitEditing={({ nativeEvent }) => {
            changeDraft(nativeEvent.text);
            setQuery(nativeEvent.text.trim());
          }}
          showClear={hasDraft}
          clearLabel="清除搜索"
          onClear={() => {
            searchInput.current?.clear();
            changeDraft("");
            setQuery("");
          }}
        />
        <Button label="搜索" onPress={search} style={styles.searchButton} />
      </View>
      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={styles.sortRow}
        keyboardShouldPersistTaps="handled"
      >
        <TabBar items={sorts} value={sort} onValueChange={setSort} accessibilityLabel="项目排序" />
      </ScrollView>
      <View style={styles.feedHeading}>
        <Text style={styles.feedTitle}>{query ? `“${query}”的搜索结果` : "精选开源项目"}</Text>
        {state.total > 0 && (
          <Text style={styles.feedCount}>{state.total.toLocaleString()} 个项目</Text>
        )}
      </View>
    </View>
  );
  return (
    <FlatList
      key={layout.columns}
      ref={list}
      data={state.items}
      keyExtractor={(item) => item.id}
      numColumns={layout.columns}
      style={styles.feed}
      contentContainerStyle={{
        paddingHorizontal: layout.padding,
        paddingBottom: 32 + bottomInset,
        gap: 18,
      }}
      columnWrapperStyle={{ gap: layout.gap }}
      ListHeaderComponent={header}
      keyboardShouldPersistTaps="handled"
      renderItem={({ item }) => (
        <ProjectCard project={item} width={layout.cardWidth} onPress={() => onProject(item.id)} />
      )}
      onEndReachedThreshold={0.4}
      onEndReached={() => {
        if (active && state.items.length && !state.error) void controller.loadMore();
      }}
      ListEmptyComponent={
        state.loading ? (
          <Skeleton
            label="正在寻找好项目"
            style={{ flexDirection: "row", flexWrap: "wrap", gap: layout.gap }}
          >
            {["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
              .slice(0, layout.columns * 2)
              .map((slot) => (
                <View key={slot} style={{ width: layout.cardWidth, gap: 12, paddingBottom: 16 }}>
                  <SkeletonBlock style={{ height: layout.cardWidth / 1.17 }} />
                  <SkeletonBlock style={{ width: "86%", height: 20 }} />
                  <SkeletonBlock style={{ width: "60%", height: 12 }} />
                </View>
              ))}
          </Skeleton>
        ) : state.error ? (
          <ErrorPanel error={state.error} retry={() => void controller.loadMore()} />
        ) : (
          <Empty title="还没有找到相关项目" detail="试试更短的关键词，或换一个搜索方向。" />
        )
      }
      ListFooterComponent={
        state.items.length ? (
          <View style={styles.feedFooter}>
            {state.error ? (
              <ErrorPanel error={state.error} retry={() => void controller.loadMore()} />
            ) : state.loading ? (
              <Loading label="正在加载更多项目" />
            ) : state.hasMore ? (
              <Button label="加载更多项目" secondary onPress={() => void controller.loadMore()} />
            ) : (
              <Text style={styles.endNote}>已经看到全部项目了</Text>
            )}
          </View>
        ) : undefined
      }
    />
  );
}

const styles = StyleSheet.create({
  feed: { flex: 1, width: "100%", maxWidth: 1440, alignSelf: "center" },
  discoverHeading: { paddingTop: 28, paddingBottom: 22 },
  eyebrow: {
    color: color.green,
    fontSize: 10,
    fontWeight: "700",
    letterSpacing: 1.4,
    marginBottom: 10,
  },
  title: { fontSize: 28, lineHeight: 37, color: color.ink, fontWeight: "700", letterSpacing: -0.7 },
  subtitle: { marginTop: 8, color: color.muted, fontSize: 12, lineHeight: 20 },
  searchRow: { flexDirection: "row", gap: 10, alignItems: "center" },
  searchButton: { minHeight: 48, paddingHorizontal: 16 },
  sortRow: { gap: 8, paddingTop: 20, paddingBottom: 8 },
  feedHeading: {
    paddingTop: 20,
    paddingBottom: 4,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12,
    flexWrap: "wrap",
  },
  feedTitle: { fontSize: 17, fontWeight: "600", color: color.ink, flexShrink: 1 },
  feedCount: { fontSize: 11, color: "#8a978e" },
  feedFooter: { marginTop: 16, paddingBottom: 16 },
  endNote: { paddingVertical: 20, textAlign: "center", fontSize: 12, color: color.muted },
});

import { call, type FeedPage, type ProjectSummary, type Sort } from "@oshwhub/bridge";
import {
  type ComponentRef,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { FlatList, ScrollView, type TextInput, View } from "react-native";
import { ActionButton as Button } from "../components/action-button";
import { ProjectCard } from "../components/project-card";
import { SearchField } from "../components/search-field";
import { Empty, ErrorPanel, Loading } from "../components/states";
import { TabBar } from "../components/tab-bar";
import { Text } from "../components/ui/text";
import { cn } from "../lib/utils";
import { Skeleton, SkeletonBlock } from "../loading";
import { FeedController, layoutFor } from "../model";

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
      <View className="pt-7 pb-[22px]">
        <Text className="mb-2.5 text-[10px] font-bold tracking-[1.4px] text-primary">
          OSHWHUB · 开源硬件社区
        </Text>
        <Text className="text-[28px] leading-[37px] font-bold tracking-[-0.7px] text-foreground">
          发现下一个好想法
        </Text>
        <Text className="mt-2 text-[12px] leading-5 text-muted-foreground">
          探索、学习，与创造者一起把灵感变成现实。
        </Text>
      </View>
      <View className="flex-row items-center gap-2.5">
        {/* Keep native edits across list rerenders; clear() is the only write to the input. */}
        <SearchField
          ref={searchInput}
          containerClassName="flex-1"
          accessibilityLabel="搜索开源硬件项目"
          placeholder="搜索项目、开发板、灵感…"
          placeholderTextColorClassName="accent-[#8e9993]"
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
        <Button label="搜索" onPress={search} className="min-h-12 px-4" />
      </View>
      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        contentContainerClassName="gap-2 pt-5 pb-2"
        keyboardShouldPersistTaps="handled"
      >
        <TabBar items={sorts} value={sort} onValueChange={setSort} accessibilityLabel="项目排序" />
      </ScrollView>
      <View className="flex-row flex-wrap items-center justify-between gap-3 pt-5 pb-1">
        <Text className="shrink text-[17px] font-semibold text-foreground">
          {query ? `“${query}”的搜索结果` : "精选开源项目"}
        </Text>
        {state.total > 0 && (
          <Text className="text-[11px] text-[#8a978e]">{state.total.toLocaleString()} 个项目</Text>
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
      className="w-full max-w-[1440px] flex-1 self-center"
      contentContainerClassName={cn("gap-[18px]", layout.padding === 16 ? "px-4" : "px-6")}
      contentContainerStyle={{ paddingBottom: 32 + bottomInset }}
      columnWrapperClassName={cn(layout.gap === 12 ? "gap-3" : "gap-4")}
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
            className={cn("flex-row flex-wrap", layout.gap === 12 ? "gap-3" : "gap-4")}
          >
            {["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
              .slice(0, layout.columns * 2)
              .map((slot) => (
                <View key={slot} className="gap-3 pb-4" style={{ width: layout.cardWidth }}>
                  <SkeletonBlock style={{ height: layout.cardWidth / 1.17 }} />
                  <SkeletonBlock className="h-5 w-[86%]" />
                  <SkeletonBlock className="h-3 w-[60%]" />
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
          <View className="mt-4 pb-4">
            {state.error ? (
              <ErrorPanel error={state.error} retry={() => void controller.loadMore()} />
            ) : state.loading ? (
              <Loading label="正在加载更多项目" />
            ) : state.hasMore ? (
              <Button label="加载更多项目" secondary onPress={() => void controller.loadMore()} />
            ) : (
              <Text className="py-5 text-center text-[12px] text-muted-foreground">
                已经看到全部项目了
              </Text>
            )}
          </View>
        ) : undefined
      }
    />
  );
}

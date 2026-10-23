import { call, type FeedPage, type ProjectSummary, type Sort } from "@oshwhub/bridge";
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import {
  Animated,
  BackHandler,
  FlatList,
  Image,
  type LayoutChangeEvent,
  type LayoutRectangle,
  Platform,
  Pressable,
  ScrollView,
  StatusBar,
  StyleSheet,
  Text,
  TextInput,
  useWindowDimensions,
  View,
} from "react-native";
import { SafeAreaProvider, SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context";
import {
  BoardMark,
  Button,
  Cover,
  color,
  common,
  Empty,
  ErrorPanel,
  ExternalLink,
  Loading,
} from "./common";
import { GlassSurface } from "./GlassSurface";
import { Skeleton, SkeletonBlock } from "./loading";
import { FeedController, layoutFor, safeUrl } from "./model";
import { NativeTabBar } from "./NativeTabBar";
import { NavigationIcon } from "./NavigationIcon";
import { ProjectPage } from "./ProjectPage";
import { useReducedMotion } from "./useReducedMotion";
import { ViewerPage, type ViewerRoute } from "./ViewerPage";

type Tab = "discover" | "lab" | "about";
const navigation: { id: Tab; label: string; symbol: string }[] = [
  { id: "discover", label: "发现", symbol: "safari" },
  { id: "lab", label: "PCB 实验室", symbol: "cpu" },
  { id: "about", label: "关于", symbol: "info.circle" },
];
const sorts: { value: Sort; label: string }[] = [
  { value: "recommend", label: "为你推荐" },
  { value: "new", label: "最新发布" },
  { value: "like", label: "最多喜欢" },
];

export function App() {
  return (
    <SafeAreaProvider style={styles.provider}>
      <Shell />
    </SafeAreaProvider>
  );
}

function Shell() {
  const { width } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const layout = layoutFor(width);
  const [tab, setTab] = useState<Tab>("discover");
  const [projectId, setProjectId] = useState<string | null>(null);
  const [viewer, setViewer] = useState<ViewerRoute | null>(null);
  const [dockHeight, setDockHeight] = useState(70);
  const showDock = layout.sidebar === 0 && !viewer;
  const dockBottom = Math.max(insets.bottom, 12);
  const bottomInset = showDock ? dockHeight + dockBottom + 16 : 0;
  const navigate = (next: Tab) => {
    setTab(next);
    setProjectId(null);
    setViewer(null);
  };
  useEffect(() => {
    if (Platform.OS !== "android") return;
    const subscription = BackHandler.addEventListener("hardwareBackPress", () => {
      if (viewer) {
        setViewer(null);
        return true;
      }
      if (projectId) {
        setProjectId(null);
        return true;
      }
      if (tab !== "discover") {
        setTab("discover");
        return true;
      }
      return false;
    });
    return () => subscription.remove();
  }, [viewer, projectId, tab]);
  const hiddenFeed = tab !== "discover" || projectId !== null || viewer !== null;
  return (
    <SafeAreaView
      style={[styles.app, viewer && { backgroundColor: "#0b121a" }]}
      edges={showDock ? ["top", "left", "right"] : ["top", "left", "right", "bottom"]}
    >
      <StatusBar barStyle={viewer ? "light-content" : "dark-content"} />
      <View nativeID="app-shell" style={styles.shell}>
        {layout.sidebar > 0 && !viewer && (
          <View style={[styles.sidebar, { width: layout.sidebar }]}>
            <GlassSurface style={styles.sidebarGlass}>
              <View style={styles.brand}>
                <View style={styles.brandIcon}>
                  <NavigationIcon kind="cpu" color="#fff" />
                </View>
                <Text style={styles.brandName}>OSHWHub</Text>
              </View>
              <Text style={styles.brandNote}>让硬件想法发生</Text>
              <Navigation tab={tab} navigate={navigate} desktop />
              <View style={styles.sidebarFoot}>
                <View style={styles.onlineDot} />
                <Text style={styles.sidebarFootText}>公开浏览 · 无需登录</Text>
              </View>
            </GlassSurface>
          </View>
        )}
        <View style={styles.main}>
          <View
            style={styles.fill}
            pointerEvents={hiddenFeed ? "none" : "auto"}
            aria-hidden={hiddenFeed}
            importantForAccessibility={hiddenFeed ? "no-hide-descendants" : "auto"}
          >
            <Discover
              width={width}
              active={!hiddenFeed}
              onProject={setProjectId}
              bottomInset={bottomInset}
            />
          </View>
          {tab === "lab" && !viewer && (
            <View style={styles.overlay}>
              <Lab onOpen={() => setViewer({ demo: true })} bottomInset={bottomInset} />
            </View>
          )}
          {tab === "about" && (
            <View style={styles.overlay}>
              <About bottomInset={bottomInset} />
            </View>
          )}
          {projectId && (
            <View
              style={styles.overlay}
              pointerEvents={viewer ? "none" : "auto"}
              aria-hidden={!!viewer}
              importantForAccessibility={viewer ? "no-hide-descendants" : "auto"}
            >
              <ProjectPage
                id={projectId}
                width={width - layout.sidebar}
                bottomInset={bottomInset}
                onBack={() => setProjectId(null)}
                onPcb={(documentId, name) =>
                  setViewer({ projectId, documentId, title: name, demo: false })
                }
              />
            </View>
          )}
          {viewer && (
            <View style={styles.overlay}>
              <ViewerPage route={viewer} onBack={() => setViewer(null)} />
            </View>
          )}
        </View>
      </View>
      {showDock && (
        <View
          pointerEvents="box-none"
          style={[
            styles.dock,
            { bottom: dockBottom, left: insets.left + 16, right: insets.right + 16 },
          ]}
        >
          <Navigation
            tab={tab}
            navigate={navigate}
            onLayout={({ nativeEvent }) => setDockHeight(nativeEvent.layout.height)}
          />
        </View>
      )}
    </SafeAreaView>
  );
}

type NavigationProps = {
  tab: Tab;
  navigate: (tab: Tab) => void;
  desktop?: boolean;
  onLayout?: (event: LayoutChangeEvent) => void;
};

function Navigation(props: NavigationProps) {
  if (Platform.OS === "ios" && !props.desktop) {
    return (
      <NativeTabBar
        items={navigation}
        selectedIndex={navigation.findIndex((item) => item.id === props.tab)}
        onSelect={({ nativeEvent }) => {
          const item = navigation[nativeEvent.index];
          if (item) props.navigate(item.id);
        }}
        onLayout={props.onLayout}
        style={styles.nativeBottomNav}
      />
    );
  }
  return <GlassNavigation {...props} />;
}

function GlassNavigation({ tab, navigate, desktop = false, onLayout }: NavigationProps) {
  const Surface = desktop ? View : GlassSurface;
  const Lens = Platform.OS === "web" ? View : Animated.View;
  const [layouts, setLayouts] = useState<Partial<Record<Tab, LayoutRectangle>>>({});
  const bounds = layouts[tab];
  const position = useRef(new Animated.ValueXY()).current;
  const positioned = useRef(false);
  const reducedMotion = useReducedMotion();
  useEffect(() => {
    if (!bounds || Platform.OS === "web") return;
    const target = { x: bounds.x, y: bounds.y };
    if (!positioned.current || reducedMotion) {
      position.setValue(target);
      positioned.current = true;
      return;
    }
    const slide = Animated.spring(position, {
      toValue: target,
      stiffness: 300,
      damping: 30,
      mass: 1,
      useNativeDriver: true,
      isInteraction: false,
    });
    slide.start();
    return () => slide.stop();
  }, [bounds, position, reducedMotion]);
  return (
    <Surface
      style={desktop ? styles.sideNav : styles.bottomNav}
      accessibilityRole="tablist"
      onLayout={onLayout}
    >
      {bounds && (
        <Lens
          nativeID="navigation-lens"
          testID="navigation-lens"
          pointerEvents="none"
          aria-hidden
          accessibilityElementsHidden
          style={[
            styles.navLens,
            {
              width: bounds.width,
              height: bounds.height,
              borderRadius: bounds.height / 2,
              ...(Platform.OS === "web"
                ? { left: bounds.x, top: bounds.y }
                : { transform: position.getTranslateTransform() }),
            },
          ]}
        />
      )}
      {navigation.map((item) => (
        <Pressable
          key={item.id}
          accessibilityRole="tab"
          aria-selected={tab === item.id}
          accessibilityLabel={item.label}
          onPress={() => navigate(item.id)}
          onLayout={({ nativeEvent: { layout } }) =>
            setLayouts((previous) => ({ ...previous, [item.id]: layout }))
          }
          style={({ pressed }) => [
            desktop ? styles.sideNavItem : styles.bottomNavItem,
            pressed && styles.navPressed,
          ]}
        >
          <NavigationIcon kind={item.symbol} color={tab === item.id ? color.green : "#53645c"} />
          <Text
            style={[
              styles.navText,
              desktop && styles.sideNavText,
              tab === item.id && styles.navSelected,
            ]}
          >
            {item.label}
          </Text>
        </Pressable>
      ))}
    </Surface>
  );
}

function Discover({
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
  const [draft, setDraft] = useState("");
  const latestDraft = useRef("");
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
    setDraft(text);
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
        <View style={styles.searchBox}>
          <Text style={styles.searchIcon} accessibilityElementsHidden>
            ⌕
          </Text>
          <TextInput
            accessibilityLabel="搜索开源硬件项目"
            placeholder="搜索项目、开发板、灵感…"
            placeholderTextColor="#8e9993"
            value={draft}
            onChangeText={changeDraft}
            onSubmitEditing={({ nativeEvent }) => {
              changeDraft(nativeEvent.text);
              setQuery(nativeEvent.text.trim());
            }}
            returnKeyType="search"
            autoCorrect={false}
            style={styles.searchInput}
            clearButtonMode="never"
          />
          {draft.length > 0 && (
            <Pressable
              accessibilityRole="button"
              accessibilityLabel="清除搜索"
              onPress={() => {
                changeDraft("");
                setQuery("");
              }}
              style={styles.clearSearch}
            >
              <Text style={styles.clearSearchText}>×</Text>
            </Pressable>
          )}
        </View>
        <Button label="搜索" onPress={search} style={styles.searchButton} />
      </View>
      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={styles.sortRow}
        keyboardShouldPersistTaps="handled"
      >
        {sorts.map((item) => (
          <Pressable
            key={item.value}
            accessibilityRole="tab"
            aria-selected={sort === item.value}
            onPress={() => setSort(item.value)}
            style={[styles.sort, sort === item.value && styles.sortSelected]}
          >
            <Text style={[styles.sortText, sort === item.value && styles.sortTextSelected]}>
              {item.label}
            </Text>
          </Pressable>
        ))}
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

function ProjectCard({
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
      style={({ pressed }) => [styles.card, { width }, pressed && common.pressed]}
    >
      <View style={styles.cardCover}>
        <Cover url={project.cover} title={project.name} />
        {project.tags[0] && (
          <View style={styles.cardTag}>
            <Text numberOfLines={1} style={styles.cardTagText}>
              {project.tags[0]}
            </Text>
          </View>
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

function Lab({ onOpen, bottomInset }: { onOpen: () => void; bottomInset: number }) {
  return (
    <ScrollView contentContainerStyle={[styles.staticPage, { paddingBottom: 28 + bottomInset }]}>
      <Text style={styles.eyebrow}>PCB LAB</Text>
      <Text style={styles.title}>看清每一个连接</Text>
      <Text style={styles.staticDescription}>
        在项目详情中选择板子，即可查看二维 PCB、切换图层和拾取图元。
      </Text>
      <View style={styles.labCard}>
        <View style={styles.labDrawing}>
          <BoardMark />
        </View>
        <Text style={styles.labTitle}>先试试 PCB 查看器</Text>
        <View style={styles.demoBadge}>
          <Text style={styles.demoBadgeText}>本地自制示例</Text>
        </View>
        <Text style={styles.staticDescription}>
          一块用于演示交互的示例板，支持平移、缩放、图层切换与属性查看。
        </Text>
        <Button label="打开示例板" onPress={onOpen} />
        <Text style={styles.labCaption}>示例由应用内置生成，不代表在线项目。</Text>
      </View>
    </ScrollView>
  );
}

function About({ bottomInset }: { bottomInset: number }) {
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
          浏览公开项目、阅读制作说明、查看物料清单，在本地二维画布中探索
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
  provider: { flex: 1, height: "100%" },
  app: { flex: 1, backgroundColor: "#edf4ef" },
  shell: { flex: 1, flexDirection: "row", minHeight: 0 },
  fill: { flex: 1, minHeight: 0 },
  main: { flex: 1, minWidth: 0, minHeight: 0, backgroundColor: "#fff" },
  overlay: { ...StyleSheet.absoluteFill, backgroundColor: "#fff" },
  sidebar: {
    padding: 12,
  },
  sidebarGlass: {
    flex: 1,
    paddingHorizontal: 12,
    paddingTop: 24,
    paddingBottom: 20,
    borderRadius: 28,
    borderWidth: 0.5,
    borderColor: "#ffffffab",
    boxShadow: "0 8px 32px rgba(31, 52, 44, 0.08)",
  },
  brand: { flexDirection: "row", alignItems: "center", gap: 8 },
  brandIcon: {
    width: 34,
    height: 34,
    borderRadius: 10,
    backgroundColor: color.green,
    alignItems: "center",
    justifyContent: "center",
  },
  brandName: {
    fontSize: 20,
    letterSpacing: -0.6,
    fontWeight: "700",
    color: color.ink,
    flexShrink: 1,
  },
  brandNote: { color: color.muted, fontSize: 11, marginLeft: 42, marginTop: 8 },
  sideNav: { marginTop: 44, gap: 8 },
  sideNavItem: {
    minHeight: 50,
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    paddingHorizontal: 12,
    paddingVertical: 10,
    borderRadius: 25,
  },
  navLens: {
    position: "absolute",
    top: 0,
    left: 0,
    backgroundColor: Platform.OS === "web" ? undefined : "#ffffff80",
    borderWidth: 0.5,
    borderColor: "#ffffffc7",
    boxShadow: "0 2px 6px rgba(31, 52, 44, 0.06)",
  },
  navPressed: { transform: [{ scale: 0.96 }] },
  navText: { fontSize: 12, color: "#4b5f52", fontWeight: "500", flexShrink: 1 },
  sideNavText: { fontSize: 14 },
  navSelected: { color: color.green, fontWeight: "600" },
  sidebarFoot: {
    marginTop: "auto",
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingLeft: 8,
  },
  onlineDot: { width: 5, height: 5, borderRadius: 3, backgroundColor: "#80a68e" },
  sidebarFootText: { color: "#8a958e", fontSize: 10 },
  dock: {
    position: "absolute",
    alignItems: "center",
  },
  bottomNav: {
    width: "100%",
    maxWidth: 480,
    flexDirection: "row",
    borderRadius: 36,
    padding: 6,
    gap: 4,
    boxShadow: "0 8px 32px rgba(31, 52, 44, 0.12)",
  },
  nativeBottomNav: { width: "100%", maxWidth: 480, height: 74 },
  bottomNavItem: {
    flex: 1,
    minHeight: 56,
    gap: 2,
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: 4,
    paddingVertical: 4,
    borderRadius: 28,
  },
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
  searchBox: {
    flex: 1,
    flexDirection: "row",
    minWidth: 0,
    minHeight: 48,
    alignItems: "center",
    backgroundColor: "#f3f6f3",
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#eaf0eb",
  },
  searchIcon: { fontSize: 25, color: "#8c9a91", paddingLeft: 12, paddingRight: 6 },
  searchInput: {
    flex: 1,
    minWidth: 0,
    minHeight: 46,
    paddingVertical: 10,
    paddingRight: 8,
    fontSize: 13,
    color: color.ink,
    ...(Platform.OS === "web" ? { outlineWidth: 0 } : {}),
  },
  clearSearch: { minWidth: 44, minHeight: 44, alignItems: "center", justifyContent: "center" },
  clearSearchText: { fontSize: 22, color: color.muted },
  searchButton: { minHeight: 48, paddingHorizontal: 16 },
  sortRow: { gap: 8, paddingTop: 20, paddingBottom: 8 },
  sort: {
    minHeight: 44,
    paddingHorizontal: 16,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 22,
  },
  sortSelected: { backgroundColor: color.greenLight },
  sortText: { fontSize: 14, color: color.muted },
  sortTextSelected: { color: color.green, fontWeight: "700" },
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
  feedFooter: { marginTop: 16, paddingBottom: 16 },
  endNote: { paddingVertical: 20, textAlign: "center", fontSize: 12, color: color.muted },
  staticPage: {
    flexGrow: 1,
    padding: 28,
    gap: 16,
    width: "100%",
    maxWidth: 800,
    alignSelf: "center",
  },
  staticDescription: { fontSize: 14, lineHeight: 25, color: color.muted },
  labCard: {
    marginTop: 16,
    padding: 24,
    borderRadius: 20,
    borderWidth: 1,
    borderColor: color.line,
    gap: 18,
    backgroundColor: "#fbfdfb",
  },
  labDrawing: {
    height: 120,
    backgroundColor: color.greenLight,
    borderRadius: 14,
    alignItems: "center",
    justifyContent: "center",
  },
  labTitle: { fontSize: 20, fontWeight: "600", color: color.ink },
  demoBadge: {
    alignSelf: "flex-start",
    paddingHorizontal: 9,
    paddingVertical: 5,
    borderRadius: 6,
    backgroundColor: "#eaf0e3",
  },
  demoBadgeText: { color: "#647c41", fontSize: 11, fontWeight: "600" },
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

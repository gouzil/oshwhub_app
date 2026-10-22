import { call, type FeedPage, type ProjectSummary, type Sort } from "@oshwhub/bridge";
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import {
  BackHandler,
  FlatList,
  Image,
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
import { SafeAreaProvider, SafeAreaView } from "react-native-safe-area-context";
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
import { FeedController, layoutFor, safeUrl } from "./model";
import { ProjectPage } from "./ProjectPage";
import { ViewerPage, type ViewerRoute } from "./ViewerPage";

type Tab = "discover" | "lab" | "about";
const navigation: { id: Tab; label: string; icon: string }[] = [
  { id: "discover", label: "发现", icon: "◈" },
  { id: "lab", label: "PCB 实验室", icon: "▦" },
  { id: "about", label: "关于", icon: "i" },
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
  const layout = layoutFor(width);
  const [tab, setTab] = useState<Tab>("discover");
  const [projectId, setProjectId] = useState<string | null>(null);
  const [viewer, setViewer] = useState<ViewerRoute | null>(null);
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
    <SafeAreaView style={styles.app} edges={["top", "left", "right", "bottom"]}>
      <StatusBar barStyle="dark-content" />
      <View style={styles.shell}>
        {layout.sidebar > 0 && (
          <View style={[styles.sidebar, { width: layout.sidebar }]}>
            <View style={styles.brand}>
              <View style={styles.brandIcon}>
                <Text style={styles.brandSymbol}>▦</Text>
              </View>
              <Text style={styles.brandName}>OSHWHub</Text>
            </View>
            <Text style={styles.brandNote}>让硬件想法发生</Text>
            <Navigation tab={tab} navigate={navigate} desktop />
            <View style={styles.sidebarFoot}>
              <View style={styles.onlineDot} />
              <Text style={styles.sidebarFootText}>公开浏览 · 无需登录</Text>
            </View>
          </View>
        )}
        <View style={styles.main}>
          <View
            style={styles.fill}
            pointerEvents={hiddenFeed ? "none" : "auto"}
            aria-hidden={hiddenFeed}
            importantForAccessibility={hiddenFeed ? "no-hide-descendants" : "auto"}
          >
            <Discover width={width} active={!hiddenFeed} onProject={setProjectId} />
          </View>
          {tab === "lab" && !viewer && (
            <View style={styles.overlay}>
              <Lab onOpen={() => setViewer({ demo: true })} />
            </View>
          )}
          {tab === "about" && (
            <View style={styles.overlay}>
              <About />
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
      {layout.sidebar === 0 && !viewer && <Navigation tab={tab} navigate={navigate} />}
    </SafeAreaView>
  );
}

function Navigation({
  tab,
  navigate,
  desktop = false,
}: {
  tab: Tab;
  navigate: (tab: Tab) => void;
  desktop?: boolean;
}) {
  return (
    <View style={desktop ? styles.sideNav : styles.bottomNav} accessibilityRole="tablist">
      {navigation.map((item) => (
        <Pressable
          key={item.id}
          accessibilityRole="tab"
          aria-selected={tab === item.id}
          accessibilityLabel={item.label}
          onPress={() => navigate(item.id)}
          style={({ pressed }) => [
            desktop ? styles.sideNavItem : styles.bottomNavItem,
            desktop && tab === item.id && styles.sideNavSelected,
            pressed && common.pressed,
          ]}
        >
          <Text
            style={[styles.navIcon, tab === item.id && styles.navSelected]}
            accessibilityElementsHidden
          >
            {item.icon}
          </Text>
          <Text style={[styles.navText, tab === item.id && styles.navSelected]}>{item.label}</Text>
        </Pressable>
      ))}
    </View>
  );
}

function Discover({
  width,
  active,
  onProject,
}: {
  width: number;
  active: boolean;
  onProject: (id: string) => void;
}) {
  const layout = layoutFor(width);
  const [query, setQuery] = useState("");
  const [draft, setDraft] = useState("");
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
  const search = () => setQuery(draft.trim());
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
            onChangeText={setDraft}
            onSubmitEditing={search}
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
                setDraft("");
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
      contentContainerStyle={{ paddingHorizontal: layout.padding, paddingBottom: 32, gap: 18 }}
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
          <Loading label="正在寻找好项目" />
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

function Lab({ onOpen }: { onOpen: () => void }) {
  return (
    <ScrollView contentContainerStyle={styles.staticPage}>
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

function About() {
  return (
    <ScrollView contentContainerStyle={styles.staticPage}>
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
  app: { flex: 1, backgroundColor: "#fff" },
  shell: { flex: 1, flexDirection: "row", minHeight: 0 },
  fill: { flex: 1, minHeight: 0 },
  main: { flex: 1, minWidth: 0, minHeight: 0, backgroundColor: "#fff" },
  overlay: { ...StyleSheet.absoluteFill, backgroundColor: "#fff" },
  sidebar: {
    paddingHorizontal: 18,
    paddingTop: 32,
    paddingBottom: 20,
    borderRightWidth: 1,
    borderRightColor: color.line,
  },
  brand: { flexDirection: "row", alignItems: "center", gap: 10, paddingHorizontal: 6 },
  brandIcon: {
    width: 34,
    height: 34,
    borderRadius: 10,
    backgroundColor: color.green,
    alignItems: "center",
    justifyContent: "center",
  },
  brandSymbol: { color: "#fff", fontSize: 27 },
  brandName: {
    fontSize: 20,
    letterSpacing: -0.6,
    fontWeight: "700",
    color: color.ink,
    flexShrink: 1,
  },
  brandNote: { color: color.muted, fontSize: 11, marginLeft: 51, marginTop: 8 },
  sideNav: { marginTop: 44, gap: 8 },
  sideNavItem: {
    minHeight: 50,
    flexDirection: "row",
    alignItems: "center",
    gap: 12,
    paddingHorizontal: 16,
    borderRadius: 12,
  },
  sideNavSelected: { backgroundColor: color.greenLight },
  navIcon: { color: "#839087", fontSize: 24, minWidth: 24, textAlign: "center" },
  navText: { fontSize: 12, color: color.muted, fontWeight: "500", flexShrink: 1 },
  navSelected: { color: color.green, fontWeight: "700" },
  sidebarFoot: {
    marginTop: "auto",
    flexDirection: "row",
    alignItems: "center",
    gap: 7,
    paddingLeft: 12,
  },
  onlineDot: { width: 5, height: 5, borderRadius: 3, backgroundColor: "#80a68e" },
  sidebarFootText: { color: "#8a958e", fontSize: 10 },
  bottomNav: {
    flexDirection: "row",
    borderTopWidth: 1,
    borderTopColor: color.line,
    backgroundColor: "#fff",
    paddingVertical: 5,
  },
  bottomNavItem: {
    flex: 1,
    minHeight: 54,
    gap: 2,
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: 4,
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

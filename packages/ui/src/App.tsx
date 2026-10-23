import { useEffect, useState } from "react";
import {
  BackHandler,
  Platform,
  StatusBar,
  StyleSheet,
  Text,
  useWindowDimensions,
  View,
} from "react-native";
import { SafeAreaProvider, SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context";
import { Uniwind } from "uniwind";
import { Navigation, type Tab } from "./components/navigation";
import { GlassSurface } from "./GlassSurface";
import { layoutFor } from "./model";
import { NavigationIcon } from "./NavigationIcon";
import { AboutPage } from "./screens/AboutPage";
import { DiscoverPage } from "./screens/DiscoverPage";
import { LabPage } from "./screens/LabPage";
import { ProjectPage } from "./screens/ProjectPage";
import { ViewerPage, type ViewerRoute } from "./screens/ViewerPage";
import { color } from "./theme";

Uniwind.setTheme("light");

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
            <DiscoverPage
              width={width}
              active={!hiddenFeed}
              onProject={setProjectId}
              bottomInset={bottomInset}
            />
          </View>
          {tab === "lab" && !viewer && (
            <View style={styles.overlay}>
              <LabPage onOpen={() => setViewer({ demo: true })} bottomInset={bottomInset} />
            </View>
          )}
          {tab === "about" && (
            <View style={styles.overlay}>
              <AboutPage bottomInset={bottomInset} />
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
});

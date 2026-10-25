import { useEffect, useLayoutEffect, useState } from "react";
import { BackHandler, Platform, StatusBar, useWindowDimensions, View } from "react-native";
import {
  SafeAreaProvider as NativeSafeAreaProvider,
  SafeAreaView as NativeSafeAreaView,
  useSafeAreaInsets,
} from "react-native-safe-area-context";
import { Uniwind, useUniwind, withUniwind } from "uniwind";
import { Navigation, type Tab } from "./components/navigation";
import { Text } from "./components/ui/text";
import { GlassSurface } from "./GlassSurface";
import { cn } from "./lib/utils";
import { layoutFor } from "./model";
import { NavigationIcon } from "./NavigationIcon";
import { AboutPage } from "./screens/AboutPage";
import { DiscoverPage } from "./screens/DiscoverPage";
import { LabPage } from "./screens/LabPage";
import { ProjectPage } from "./screens/ProjectPage";
import { SettingsPage } from "./screens/SettingsPage";
import { ViewerPage, type ViewerRoute } from "./screens/ViewerPage";
import { resolveThemeColor, resolveThemeMode, type ThemeColor, type ThemeMode } from "./theme";
import { readThemeColor, readThemeMode, writeThemeColor, writeThemeMode } from "./theme-storage";

const SafeAreaProvider = withUniwind(NativeSafeAreaProvider);
const SafeAreaView = withUniwind(NativeSafeAreaView);

export function App() {
  return (
    <SafeAreaProvider className="h-full flex-1">
      <Shell />
    </SafeAreaProvider>
  );
}

function Shell() {
  const { theme: colorScheme } = useUniwind();
  const [themeMode, setThemeMode] = useState(() => {
    try {
      return resolveThemeMode(readThemeMode());
    } catch {
      return resolveThemeMode(null);
    }
  });
  const [modeError, setModeError] = useState<string | null>(null);
  useLayoutEffect(() => {
    Uniwind.setTheme(themeMode);
  }, [themeMode]);
  const changeThemeMode = (mode: ThemeMode) => {
    setThemeMode(mode);
    try {
      writeThemeMode(mode);
      setModeError(null);
    } catch {
      setModeError("外观已应用，但无法保存；下次打开时可能恢复原来的模式。");
    }
  };
  const [themeColor, setThemeColor] = useState(() => {
    try {
      return resolveThemeColor(readThemeColor());
    } catch {
      return resolveThemeColor(null);
    }
  });
  const [themeError, setThemeError] = useState<string | null>(null);
  useLayoutEffect(() => {
    Uniwind.updateCSSVariables("light", {
      "--color-primary": themeColor.primary,
      "--color-secondary": themeColor.light,
      "--color-secondary-foreground": themeColor.primary,
      "--color-accent": themeColor.light,
      "--color-accent-foreground": themeColor.primary,
      "--color-ring": themeColor.primary,
    });
    Uniwind.updateCSSVariables("dark", {
      "--color-primary": themeColor.darkPrimary,
      "--color-secondary": themeColor.dark,
      "--color-secondary-foreground": themeColor.darkPrimary,
      "--color-accent": themeColor.dark,
      "--color-accent-foreground": themeColor.darkPrimary,
      "--color-ring": themeColor.darkPrimary,
    });
  }, [themeColor]);
  const changeThemeColor = (theme: ThemeColor) => {
    setThemeColor(theme);
    try {
      writeThemeColor(theme.id);
      setThemeError(null);
    } catch {
      setThemeError("颜色已应用，但无法保存；下次打开时可能恢复原来的颜色。");
    }
  };
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
      className={cn("flex-1 bg-surface", viewer && "bg-viewer")}
      edges={showDock ? ["top", "left", "right"] : ["top", "left", "right", "bottom"]}
    >
      <StatusBar barStyle={viewer || colorScheme === "dark" ? "light-content" : "dark-content"} />
      <View
        nativeID="app-shell"
        className="min-h-0 flex-1 flex-row web:min-[1024px]:[background:radial-gradient(ellipse_at_0%_0%,var(--color-secondary)_0,transparent_55%),radial-gradient(ellipse_at_5%_85%,var(--color-muted)_0,transparent_45%),var(--color-surface)]"
      >
        {layout.sidebar > 0 && !viewer && (
          <View className="p-3" style={{ width: layout.sidebar }}>
            <GlassSurface className="flex-1 rounded-[28px] border-[0.5px] border-[#ffffffab] dark:border-border px-3 pt-6 pb-5 shadow-[0_8px_32px_rgba(32,45,64,0.08)]">
              <View className="flex-row items-center gap-2">
                <View className="size-[34px] items-center justify-center rounded-[10px] bg-primary">
                  <NavigationIcon
                    kind="cpu"
                    strokeClassName="border-primary-foreground"
                    fillClassName="bg-primary-foreground"
                  />
                </View>
                <Text className="shrink text-[20px] font-bold tracking-[-0.6px] text-foreground">
                  OSHWHub
                </Text>
              </View>
              <Text className="mt-2 ml-[42px] text-[11px] text-muted-foreground">
                让硬件想法发生
              </Text>
              <Navigation tab={tab} navigate={navigate} desktop />
              <View className="mt-auto flex-row items-center gap-1.5 pl-2">
                <View className="size-[5px] rounded-[3px] bg-primary" />
                <Text className="text-[10px] text-muted-foreground">公开浏览 · 无需登录</Text>
              </View>
            </GlassSurface>
          </View>
        )}
        <View className="min-h-0 min-w-0 flex-1 bg-background">
          <View
            className="min-h-0 flex-1"
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
            <View className="absolute inset-0 bg-background">
              <LabPage onOpen={() => setViewer({ demo: true })} bottomInset={bottomInset} />
            </View>
          )}
          {tab === "about" && (
            <View className="absolute inset-0 bg-background">
              <AboutPage bottomInset={bottomInset} />
            </View>
          )}
          {tab === "settings" && (
            <View className="absolute inset-0 bg-background">
              <SettingsPage
                bottomInset={bottomInset}
                themeMode={themeMode}
                onThemeMode={changeThemeMode}
                modeError={modeError}
                themeColor={themeColor}
                onThemeColor={changeThemeColor}
                error={themeError}
              />
            </View>
          )}
          {projectId && (
            <View
              className="absolute inset-0 bg-background"
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
            <View className="absolute inset-0 bg-background">
              <ViewerPage route={viewer} onBack={() => setViewer(null)} />
            </View>
          )}
        </View>
      </View>
      {showDock && (
        <View
          pointerEvents="box-none"
          className="absolute items-center"
          style={{ bottom: dockBottom, left: insets.left + 16, right: insets.right + 16 }}
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

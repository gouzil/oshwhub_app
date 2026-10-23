import { useEffect, useRef, useState } from "react";
import {
  Animated,
  type LayoutChangeEvent,
  type LayoutRectangle,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { GlassSurface } from "../GlassSurface";
import { NativeTabBar } from "../NativeTabBar";
import { NavigationIcon } from "../NavigationIcon";
import { color } from "../theme";
import { useReducedMotion } from "../useReducedMotion";

export type Tab = "discover" | "lab" | "about";

const navigation: { id: Tab; label: string; symbol: string }[] = [
  { id: "discover", label: "发现", symbol: "safari" },
  { id: "lab", label: "PCB 实验室", symbol: "cpu" },
  { id: "about", label: "关于", symbol: "info.circle" },
];

type NavigationProps = {
  tab: Tab;
  navigate: (tab: Tab) => void;
  desktop?: boolean;
  onLayout?: (event: LayoutChangeEvent) => void;
};

export function Navigation(props: NavigationProps) {
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

const styles = StyleSheet.create({
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
});

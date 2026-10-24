import { useEffect, useRef, useState } from "react";
import {
  Animated,
  type LayoutChangeEvent,
  type LayoutRectangle,
  Platform,
  View,
} from "react-native";
import { GlassSurface } from "../GlassSurface";
import { cn } from "../lib/utils";
import { NativeTabBar } from "../NativeTabBar";
import { NavigationIcon } from "../NavigationIcon";
import { useReducedMotion } from "../useReducedMotion";
import { Button } from "./ui/button";
import { Text } from "./ui/text";

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
        className="h-[74px] w-full max-w-[480px]"
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
      className={
        desktop
          ? "mt-11 gap-2"
          : "w-full max-w-[480px] flex-row gap-1 rounded-[36px] p-1.5 shadow-[0_8px_32px_rgba(31,52,44,0.12)]"
      }
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
          className="absolute top-0 left-0 border-[0.5px] border-[#ffffffc7] bg-[#ffffff80] shadow-[0_2px_6px_rgba(31,52,44,0.06)] web:bg-white/[0.18] web:bg-[linear-gradient(150deg,rgba(255,255,255,0.7),rgba(255,255,255,0.12))] web:[transition:left_320ms_cubic-bezier(0.2,0.8,0.2,1),top_320ms_cubic-bezier(0.2,0.8,0.2,1)] web:motion-reduce:transition-none! web:[@media(prefers-reduced-transparency:reduce)]:bg-[#e6eeea]! web:[@media(prefers-reduced-transparency:reduce)]:bg-none!"
          style={{
            width: bounds.width,
            height: bounds.height,
            borderRadius: bounds.height / 2,
            ...(Platform.OS === "web"
              ? { left: bounds.x, top: bounds.y }
              : { transform: position.getTranslateTransform() }),
          }}
        />
      )}
      {navigation.map((item) => (
        <Button
          key={item.id}
          role="tab"
          accessibilityRole="tab"
          variant="link"
          aria-selected={tab === item.id}
          accessibilityLabel={item.label}
          onPress={() => navigate(item.id)}
          onLayout={({ nativeEvent: { layout } }) =>
            setLayouts((previous) => ({ ...previous, [item.id]: layout }))
          }
          className={cn(
            "active:scale-[0.96] web:whitespace-normal web:transition-none web:focus-visible:outline-2 web:focus-visible:outline-offset-[3px] web:focus-visible:outline-[#15964a] web:focus-visible:ring-0",
            desktop
              ? "min-h-[50px] justify-start gap-3 rounded-[25px] px-3 py-2.5"
              : "min-h-14 flex-1 shrink flex-col gap-0.5 rounded-[28px] px-1 py-1",
          )}
        >
          <NavigationIcon
            kind={item.symbol}
            strokeClassName={tab === item.id ? "border-primary" : "border-[#53645c]"}
            fillClassName={tab === item.id ? "bg-primary" : "bg-[#53645c]"}
          />
          <Text
            className={cn(
              "shrink font-medium text-[#4b5f52] group-active:no-underline web:hover:no-underline web:group-hover:no-underline",
              desktop ? "text-[14px]" : "text-[12px]",
              tab === item.id && "font-semibold text-primary",
            )}
          >
            {item.label}
          </Text>
        </Button>
      ))}
    </Surface>
  );
}

import { type ReactNode, useEffect, useRef } from "react";
import { Animated, Platform, type StyleProp, View, type ViewStyle } from "react-native";
import { cn } from "./lib/utils";
import { useReducedMotion } from "./useReducedMotion";

export function Skeleton({
  children,
  label,
  style,
  className,
}: {
  children: ReactNode;
  label: string;
  style?: StyleProp<ViewStyle>;
  className?: string;
}) {
  const opacity = useRef(new Animated.Value(1)).current;
  const reducedMotion = useReducedMotion();
  useEffect(() => {
    if (reducedMotion) {
      opacity.setValue(1);
      return;
    }
    const pulse = Animated.loop(
      Animated.sequence([
        Animated.timing(opacity, {
          toValue: 0.45,
          duration: 700,
          useNativeDriver: Platform.OS !== "web",
          isInteraction: false,
        }),
        Animated.timing(opacity, {
          toValue: 1,
          duration: 700,
          useNativeDriver: Platform.OS !== "web",
          isInteraction: false,
        }),
      ]),
    );
    pulse.start();
    return () => pulse.stop();
  }, [opacity, reducedMotion]);
  return (
    <Animated.View
      accessible
      accessibilityRole="progressbar"
      accessibilityLabel={label}
      accessibilityState={{ busy: true }}
      className={className}
      style={[style, { opacity }]}
    >
      {children}
    </Animated.View>
  );
}

export function SkeletonBlock({
  style,
  className,
}: {
  style?: StyleProp<ViewStyle>;
  className?: string;
}) {
  return (
    <View
      testID="skeleton-block"
      accessible={false}
      aria-hidden
      className={cn("h-4 rounded-lg bg-skeleton", className)}
      style={style}
    />
  );
}

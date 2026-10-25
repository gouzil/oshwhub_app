import { View, type ViewProps } from "react-native";
import { cn } from "./lib/utils";

export function GlassSurface({ className, ...props }: ViewProps) {
  return (
    <View
      {...props}
      nativeID="navigation-glass"
      testID="navigation-glass"
      className={cn(
        "bg-surface web:before:absolute web:before:inset-0 web:before:rounded-[inherit] web:before:pointer-events-none web:before:content-[''] web:before:shadow-[inset_0_1px_1px_rgba(255,255,255,0.9),inset_1px_0_1px_rgba(255,255,255,0.45),inset_0_-1px_1px_rgba(32,45,64,0.1)] dark:web:before:shadow-[inset_0_1px_1px_rgba(255,255,255,0.1)] web:supports-[(backdrop-filter:blur(1px))_or_(-webkit-backdrop-filter:blur(1px))]:bg-card/40 web:supports-[(backdrop-filter:blur(1px))_or_(-webkit-backdrop-filter:blur(1px))]:bg-[linear-gradient(135deg,rgba(255,255,255,0.3),rgba(255,255,255,0.04))] dark:web:supports-[(backdrop-filter:blur(1px))_or_(-webkit-backdrop-filter:blur(1px))]:bg-card/80 dark:web:bg-none web:supports-[(backdrop-filter:blur(1px))_or_(-webkit-backdrop-filter:blur(1px))]:backdrop-blur-[18px] web:supports-[(backdrop-filter:blur(1px))_or_(-webkit-backdrop-filter:blur(1px))]:backdrop-saturate-[180%] web:[@media(prefers-reduced-transparency:reduce)]:bg-surface! web:[@media(prefers-reduced-transparency:reduce)]:bg-none! web:[@media(prefers-reduced-transparency:reduce)]:backdrop-filter-none!",
        className,
      )}
    />
  );
}

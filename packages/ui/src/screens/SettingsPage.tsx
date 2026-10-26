import { Pressable, ScrollView, View } from "react-native";
import { useUniwind } from "uniwind";
import { ActionButton } from "../components/action-button";
import { Card } from "../components/ui/card";
import { Text } from "../components/ui/text";
import { cn } from "../lib/utils";
import { type ThemeColor, type ThemeMode, themeColors, themeModes } from "../theme";

export function SettingsPage({
  bottomInset,
  themeMode,
  onThemeMode,
  modeError,
  themeColor,
  onThemeColor,
  error,
}: {
  bottomInset: number;
  themeMode: ThemeMode;
  onThemeMode: (mode: ThemeMode) => void;
  modeError: string | null;
  themeColor: ThemeColor;
  onThemeColor: (theme: ThemeColor) => void;
  error: string | null;
}) {
  const { theme: colorScheme } = useUniwind();
  return (
    <ScrollView
      contentContainerClassName="w-full max-w-[800px] grow self-center gap-4 p-7"
      contentContainerStyle={{ paddingBottom: 28 + bottomInset }}
    >
      <Text
        accessibilityRole="header"
        className="mt-5 text-[28px] leading-[37px] font-bold tracking-[-0.7px]"
      >
        设置
      </Text>
      <Text className="text-[14px] leading-[25px] text-muted-foreground">
        调整应用外观，让浏览更合心意。
      </Text>
      <Card className="mt-4 gap-4 rounded-[20px] p-5">
        <Text accessibilityRole="header" className="text-[17px] font-semibold">
          外观
        </Text>
        <Text className="text-[13px] leading-[23px] text-muted-foreground">
          默认跟随系统，也可以固定使用亮色或暗色模式。
        </Text>
        <View accessibilityRole="radiogroup" accessibilityLabel="外观" className="gap-2.5">
          {themeModes.map((mode) => {
            const selected = themeMode === mode.id;
            return (
              <Pressable
                key={mode.id}
                accessibilityRole="radio"
                accessibilityLabel={`外观 ${mode.label}`}
                accessibilityState={{ checked: selected }}
                aria-checked={selected}
                onPress={() => onThemeMode(mode.id)}
                className={cn(
                  "min-h-14 flex-row items-center gap-3 rounded-xl border border-border px-4 py-3 active:opacity-80 web:focus-visible:outline-ring",
                  selected && "border-primary bg-secondary",
                )}
              >
                <Text className="min-w-0 flex-1 text-[14px] font-medium">{mode.label}</Text>
                <Text className="text-[12px] text-primary">{selected ? "✓ 已选" : ""}</Text>
              </Pressable>
            );
          })}
        </View>
        {modeError && (
          <Text accessibilityRole="alert" className="text-[12px] leading-5 text-destructive">
            {modeError}
          </Text>
        )}
      </Card>
      <Card className="gap-4 rounded-[20px] p-5">
        <Text accessibilityRole="header" className="text-[17px] font-semibold">
          主题颜色
        </Text>
        <Text className="text-[13px] leading-[23px] text-muted-foreground">
          默认蓝色参考 OSHWHub 官网。选择后立即应用，并自动保存。
        </Text>
        <View accessibilityRole="radiogroup" accessibilityLabel="主题颜色" className="gap-2.5">
          {themeColors.map((theme) => {
            const selected = themeColor.id === theme.id;
            return (
              <Pressable
                key={theme.id}
                accessibilityRole="radio"
                accessibilityLabel={`主题颜色 ${theme.label}`}
                accessibilityState={{ checked: selected }}
                aria-checked={selected}
                onPress={() => onThemeColor(theme)}
                className={cn(
                  "min-h-16 flex-row items-center gap-3 rounded-xl border border-border px-4 py-3 active:opacity-80 web:focus-visible:outline-ring",
                  selected && "border-primary bg-secondary",
                )}
              >
                <View
                  aria-hidden
                  accessibilityElementsHidden
                  className="size-7 rounded-full"
                  style={{
                    backgroundColor: colorScheme === "dark" ? theme.darkPrimary : theme.primary,
                  }}
                />
                <Text className="min-w-0 flex-1 text-[14px] font-medium">{theme.label}</Text>
                <Text className="text-[12px] text-primary">
                  {selected ? "✓ 已选" : theme.id === "oshwhub" ? "默认" : ""}
                </Text>
              </Pressable>
            );
          })}
        </View>
        {error && (
          <Text accessibilityRole="alert" className="text-[12px] leading-5 text-destructive">
            {error}
          </Text>
        )}
        <ActionButton
          label="恢复默认颜色"
          secondary
          disabled={themeColor.id === "oshwhub" && !error}
          onPress={() => onThemeColor(themeColors[0])}
        />
      </Card>
    </ScrollView>
  );
}

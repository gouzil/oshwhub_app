import { openExternal } from "@oshwhub/bridge";
import { useState } from "react";
import { View } from "react-native";
import { safeUrl } from "../model";
import { Button } from "./ui/button";
import { Text } from "./ui/text";

export function ExternalLink({ url, label }: { url: string; label: string }) {
  const [error, setError] = useState(false);
  const target = safeUrl(url);
  if (!target)
    return (
      <Text className="text-center text-sm leading-[23px] text-muted-foreground">
        {label}（链接不可用）
      </Text>
    );
  return (
    <View>
      <Button
        variant="link"
        role="link"
        accessibilityRole="link"
        accessibilityLabel={`${label}，在浏览器中打开`}
        onPress={() => {
          setError(false);
          openExternal(target).catch(() => setError(true));
        }}
        className="min-h-11 flex-col items-stretch rounded-none px-0 py-2.5 active:opacity-[0.72] web:whitespace-normal"
      >
        <Text className="text-sm font-normal leading-[22px] text-primary">
          {label}{" "}
          <Text accessibilityElementsHidden className="text-sm font-normal text-primary">
            ↗
          </Text>
        </Text>
      </Button>
      {error && (
        <Text
          accessibilityRole="alert"
          className="text-center text-[11px] leading-[18px] text-[#946a58]"
        >
          无法打开链接，请稍后重试。
        </Text>
      )}
    </View>
  );
}

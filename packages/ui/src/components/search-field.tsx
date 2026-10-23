import type { ComponentProps } from "react";
import { View } from "react-native";
import { cn } from "../lib/utils";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Text } from "./ui/text";

type SearchFieldProps = ComponentProps<typeof Input> & {
  onClear: () => void;
  showClear: boolean;
  clearLabel: string;
  containerClassName?: string;
};

export function SearchField({
  onClear,
  showClear,
  clearLabel,
  containerClassName,
  className,
  ...props
}: SearchFieldProps) {
  return (
    <View
      className={cn(
        "min-h-12 min-w-0 flex-row items-center rounded-xl border border-input bg-muted",
        containerClassName,
      )}
    >
      <Text className="pl-3 text-2xl text-muted-foreground" accessibilityElementsHidden aria-hidden>
        ⌕
      </Text>
      <Input
        className={cn("h-12 flex-1 border-0 bg-transparent text-sm shadow-none", className)}
        returnKeyType="search"
        autoCorrect={false}
        clearButtonMode="never"
        {...props}
      />
      {showClear && (
        <Button variant="ghost" size="icon" accessibilityLabel={clearLabel} onPress={onClear}>
          <Text className="text-xl text-muted-foreground">×</Text>
        </Button>
      )}
    </View>
  );
}

import { cva } from "class-variance-authority";
import { View } from "react-native";
import { cn } from "../lib/utils";
import { Button } from "./ui/button";
import { Text } from "./ui/text";

const trigger = cva("min-h-11 rounded-full px-4 shadow-none", {
  variants: {
    variant: {
      pills: "",
      underline: "min-h-[52px] rounded-none border-b-2 border-transparent px-1.5",
      dark: "min-h-11 rounded-lg px-3",
    },
    selected: { true: "bg-secondary", false: "bg-transparent" },
  },
  compoundVariants: [
    { variant: "underline", selected: true, className: "border-primary bg-transparent" },
    { variant: "dark", selected: true, className: "bg-[#234438]" },
  ],
});

export function TabBar<T extends string>({
  items,
  value,
  onValueChange,
  accessibilityLabel,
  variant = "pills",
  disabled = false,
  className,
}: {
  items: readonly { value: T; label: string; accessibilityLabel?: string }[];
  value: T;
  onValueChange: (value: T) => void;
  accessibilityLabel: string;
  variant?: "pills" | "underline" | "dark";
  disabled?: boolean;
  className?: string;
}) {
  return (
    <View
      role="tablist"
      accessibilityLabel={accessibilityLabel}
      className={cn("flex-row gap-2", className)}
    >
      {items.map((item) => (
        <Button
          key={item.value}
          role="tab"
          variant="ghost"
          aria-selected={value === item.value}
          accessibilityLabel={item.accessibilityLabel ?? item.label}
          accessibilityState={{ selected: value === item.value, disabled }}
          disabled={disabled}
          onPress={() => onValueChange(item.value)}
          className={trigger({ variant, selected: value === item.value })}
        >
          <Text
            className={cn(
              "text-sm text-muted-foreground",
              value === item.value && "font-semibold text-primary",
              variant === "dark" && "text-[#d7e6df]",
            )}
          >
            {item.label}
          </Text>
        </Button>
      ))}
    </View>
  );
}

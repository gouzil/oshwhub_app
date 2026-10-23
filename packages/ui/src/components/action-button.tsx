import type { ComponentProps } from "react";
import { Button } from "./ui/button";
import { Text } from "./ui/text";

type ActionButtonProps = Omit<ComponentProps<typeof Button>, "children"> & {
  label: string;
  secondary?: boolean;
};

export function ActionButton({ label, secondary = false, ...props }: ActionButtonProps) {
  return (
    <Button accessibilityLabel={label} variant={secondary ? "secondary" : "default"} {...props}>
      <Text className="text-center font-semibold">{label}</Text>
    </Button>
  );
}

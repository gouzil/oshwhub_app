import { View } from "react-native";
import { cn } from "./lib/utils";

export function NavigationIcon({
  kind,
  strokeClassName,
  fillClassName,
}: {
  kind: string;
  strokeClassName: string;
  fillClassName: string;
}) {
  return (
    <View className="size-6" aria-hidden accessibilityElementsHidden>
      <View
        className={cn(
          "absolute border-[1.7px]",
          kind === "cpu" ? "inset-1 rounded-[4px]" : "inset-[1px] rounded-[12px]",
          strokeClassName,
        )}
      />
      {kind === "safari" ? (
        <View
          className={cn(
            "absolute top-[5px] left-[9px] h-[14px] w-[6px] rotate-[40deg] rounded-[1px] border-[1.7px]",
            strokeClassName,
          )}
        />
      ) : kind === "cpu" ? (
        <>
          <View
            className={cn("absolute inset-2 rounded-[1.5px] border-[1.5px]", strokeClassName)}
          />
          {[
            ["left-[7px]", "top-[7px]"],
            ["left-[15px]", "top-[15px]"],
          ].map(([vertical, horizontal]) => (
            <View key={vertical} className="absolute inset-0">
              <View
                className={cn(
                  "absolute top-0 h-1 w-[1.7px] rounded-[1px]",
                  vertical,
                  fillClassName,
                )}
              />
              <View
                className={cn(
                  "absolute bottom-0 h-1 w-[1.7px] rounded-[1px]",
                  vertical,
                  fillClassName,
                )}
              />
              <View
                className={cn(
                  "absolute left-0 h-[1.7px] w-1 rounded-[1px]",
                  horizontal,
                  fillClassName,
                )}
              />
              <View
                className={cn(
                  "absolute right-0 h-[1.7px] w-1 rounded-[1px]",
                  horizontal,
                  fillClassName,
                )}
              />
            </View>
          ))}
        </>
      ) : (
        <>
          <View
            className={cn("absolute top-[6px] left-[11px] size-[2px] rounded-[1px]", fillClassName)}
          />
          <View
            className={cn(
              "absolute top-[11px] left-[11px] h-[7px] w-[2px] rounded-[1px]",
              fillClassName,
            )}
          />
        </>
      )}
    </View>
  );
}

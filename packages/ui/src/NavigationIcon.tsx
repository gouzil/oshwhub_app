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
          kind === "cpu"
            ? "inset-1 rounded-[4px]"
            : kind === "gearshape"
              ? "inset-[4px] rounded-full"
              : "inset-[1px] rounded-[12px]",
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
      ) : kind === "gearshape" ? (
        <>
          <View
            className={cn("absolute inset-[9px] rounded-full border-[1.7px]", strokeClassName)}
          />
          {[
            "top-0 left-[10px] h-[5px] w-1",
            "bottom-0 left-[10px] h-[5px] w-1",
            "left-0 top-[10px] h-1 w-[5px]",
            "right-0 top-[10px] h-1 w-[5px]",
            "top-[3px] left-[3px] size-1 rotate-45",
            "top-[3px] right-[3px] size-1 rotate-45",
            "bottom-[3px] left-[3px] size-1 rotate-45",
            "bottom-[3px] right-[3px] size-1 rotate-45",
          ].map((position) => (
            <View
              key={position}
              className={cn("absolute rounded-[1px]", position, fillClassName)}
            />
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

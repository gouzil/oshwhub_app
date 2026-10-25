import type { BomItem } from "@oshwhub/bridge";
import { useEffect, useMemo, useState } from "react";
import { ActivityIndicator, Platform, ScrollView, View } from "react-native";
import { SearchField } from "./components/search-field";
import { Empty, ErrorPanel } from "./components/states";
import { Text } from "./components/ui/text";
import { useCommand } from "./hooks/use-command";
import { cn } from "./lib/utils";

const columns = [
  { key: "name", label: "名称 / 参数", className: "w-32" },
  { key: "designator", label: "位号", className: "w-[168px]" },
  { key: "footprint", label: "封装", className: "w-[136px]" },
  { key: "quantity", label: "数量", className: "w-[60px]" },
  { key: "partNumber", label: "制造商型号", className: "w-[156px]" },
  { key: "manufacturer", label: "制造商", className: "w-32" },
  { key: "supplier", label: "供应商", className: "w-[88px]" },
  { key: "supplierPart", label: "供应商料号", className: "w-32" },
] satisfies { key: keyof BomItem; label: string; className: string }[];

export function BomList({ items, wide }: { items: BomItem[]; wide: boolean }) {
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState("");
  useEffect(() => {
    const timer = setTimeout(() => setSearch(query.trim()), 180);
    return () => clearTimeout(timer);
  }, [query]);
  const command = useMemo(
    () => ({ command: "searchBom" as const, items, query: search }),
    [items, search],
  );
  const { value: matches, loading, error, retry } = useCommand<number[]>(command);
  const pending = loading || query.trim() !== search;
  const rows = (matches ?? []).map((index, row) => {
    const item = items[index];
    return (
      <View
        key={`${item.designator}-${index}`}
        testID={`bom-item-${index}`}
        role={Platform.OS === "web" && wide ? "row" : undefined}
        className={cn(
          wide
            ? "flex-row border-b border-border"
            : "gap-2.5 rounded-[10px] border border-border bg-white p-3.5",
          wide && row % 2 === 1 && "bg-canvas",
        )}
      >
        <Text
          role={Platform.OS === "web" && wide ? "cell" : undefined}
          className={cn(
            wide
              ? "w-12 shrink-0 px-3 py-3.5 text-[12px] leading-[22px] text-muted-foreground"
              : "mb-0.5 text-[12px] font-semibold text-primary",
          )}
        >
          {wide ? index + 1 : `物料 ${String(index + 1).padStart(2, "0")}`}
        </Text>
        {columns.map((column) => {
          const value = String(item[column.key] ?? "").trim() || "未提供";
          return (
            <View
              key={column.key}
              testID={`bom-field-${column.key}`}
              role={Platform.OS === "web" ? (wide ? "cell" : "group") : undefined}
              accessible={Platform.OS !== "web"}
              accessibilityLabel={`${column.label}：${value}`}
              className={cn(
                wide ? "shrink-0 px-3 py-3.5" : "flex-row items-start gap-3",
                wide && column.className,
              )}
            >
              {!wide && (
                <Text className="w-[84px] shrink-0 text-[12px] leading-[22px] text-muted-foreground">
                  {column.label}
                </Text>
              )}
              <Text
                selectable
                className={cn(
                  "min-w-0 flex-1 text-[13px] leading-[22px] text-foreground",
                  column.key === "name" && "font-semibold",
                  column.key === "quantity" && "font-semibold text-primary",
                  value === "未提供" && "text-[12px] leading-[22px] text-muted-foreground",
                )}
              >
                {value}
              </Text>
            </View>
          );
        })}
      </View>
    );
  });
  return (
    <View testID="bom-list" className="gap-3">
      <SearchField
        accessibilityLabel="搜索物料清单"
        placeholder="搜索名称、位号、型号或料号…"
        placeholderTextColorClassName="accent-muted-foreground"
        value={query}
        onChangeText={setQuery}
        onSubmitEditing={({ nativeEvent }) => {
          setQuery(nativeEvent.text);
          setSearch(nativeEvent.text.trim());
        }}
        autoCapitalize="none"
        maxLength={100}
        showClear={query.length > 0}
        clearLabel="清除物料搜索"
        onClear={() => {
          setQuery("");
          setSearch("");
        }}
      />
      <View className="min-h-6 flex-row items-center gap-2">
        {pending && <ActivityIndicator size="small" colorClassName="accent-primary" />}
        <Text
          testID="bom-search-count"
          accessibilityLiveRegion="polite"
          className="text-[12px] leading-5 text-muted-foreground"
        >
          {pending
            ? "正在筛选物料…"
            : error
              ? "物料搜索失败"
              : `显示 ${matches?.length ?? 0} / ${items.length} 种物料`}
        </Text>
      </View>
      {error ? (
        <ErrorPanel error={error} retry={retry} />
      ) : pending ? null : !rows.length ? (
        <Empty
          title="没有匹配的物料"
          detail="试试名称、位号、封装、厂家或料号，或清除搜索查看全部。"
        />
      ) : !wide ? (
        rows
      ) : (
        <View className="gap-2">
          <Text className="text-[12px] leading-5 text-muted-foreground">
            位号对应电路图上的元件编号；窄窗口可左右滑动表格。
          </Text>
          <ScrollView
            testID="bom-table-scroll"
            horizontal
            showsHorizontalScrollIndicator
            tabIndex={Platform.OS === "web" ? 0 : undefined}
            accessibilityLabel="物料表格，可左右滚动查看完整字段"
            className="rounded-[10px] border border-border"
          >
            <View
              role={Platform.OS === "web" ? "table" : undefined}
              accessibilityLabel="物料清单"
              className="w-[1040px]"
            >
              <View
                role={Platform.OS === "web" ? "row" : undefined}
                className="flex-row border-b border-border bg-surface"
              >
                <Text
                  role={Platform.OS === "web" ? "columnheader" : undefined}
                  className="w-12 shrink-0 px-3 py-3.5 text-[12px] leading-[22px] font-semibold text-foreground"
                >
                  序号
                </Text>
                {columns.map((column) => (
                  <Text
                    key={column.key}
                    role={Platform.OS === "web" ? "columnheader" : undefined}
                    className={cn(
                      "shrink-0 px-3 py-3.5 text-[12px] leading-[22px] font-semibold text-foreground",
                      column.className,
                    )}
                  >
                    {column.label}
                  </Text>
                ))}
              </View>
              {rows}
            </View>
          </ScrollView>
        </View>
      )}
    </View>
  );
}

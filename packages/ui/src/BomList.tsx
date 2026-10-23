import type { BomItem } from "@oshwhub/bridge";
import { useEffect, useMemo, useState } from "react";
import {
  ActivityIndicator,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { color, Empty, ErrorPanel, useCommand } from "./common";

const columns = [
  { key: "name", label: "名称 / 参数", width: 128 },
  { key: "designator", label: "位号", width: 168 },
  { key: "footprint", label: "封装", width: 136 },
  { key: "quantity", label: "数量", width: 60 },
  { key: "partNumber", label: "制造商型号", width: 156 },
  { key: "manufacturer", label: "制造商", width: 128 },
  { key: "supplier", label: "供应商", width: 88 },
  { key: "supplierPart", label: "供应商料号", width: 128 },
] satisfies { key: keyof BomItem; label: string; width: number }[];

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
        style={wide ? [styles.tableRow, row % 2 === 1 && styles.alternate] : styles.card}
      >
        <Text
          role={Platform.OS === "web" && wide ? "cell" : undefined}
          style={wide ? [styles.cell, styles.sequence, styles.label] : styles.cardHeading}
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
              style={wide ? [styles.cell, { width: column.width }] : styles.field}
            >
              {!wide && <Text style={[styles.label, styles.fieldLabel]}>{column.label}</Text>}
              <Text
                selectable
                style={[
                  styles.value,
                  column.key === "name" && styles.name,
                  column.key === "quantity" && styles.quantity,
                  value === "未提供" && styles.label,
                ]}
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
    <View testID="bom-list" style={styles.cards}>
      <View style={styles.searchBox}>
        <TextInput
          accessibilityLabel="搜索物料清单"
          placeholder="搜索名称、位号、型号或料号…"
          placeholderTextColor={color.muted}
          value={query}
          onChangeText={setQuery}
          onSubmitEditing={({ nativeEvent }) => {
            setQuery(nativeEvent.text);
            setSearch(nativeEvent.text.trim());
          }}
          returnKeyType="search"
          autoCorrect={false}
          autoCapitalize="none"
          maxLength={100}
          clearButtonMode="never"
          style={styles.searchInput}
        />
        {query.length > 0 && (
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="清除物料搜索"
            onPress={() => {
              setQuery("");
              setSearch("");
            }}
            style={styles.clearSearch}
          >
            <Text style={styles.clearSearchText}>×</Text>
          </Pressable>
        )}
      </View>
      <View style={styles.searchStatus}>
        {pending && <ActivityIndicator size="small" color={color.green} />}
        <Text testID="bom-search-count" accessibilityLiveRegion="polite" style={styles.hint}>
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
        <View style={styles.tableContainer}>
          <Text style={styles.hint}>位号对应电路图上的元件编号；窄窗口可左右滑动表格。</Text>
          <ScrollView
            testID="bom-table-scroll"
            horizontal
            showsHorizontalScrollIndicator
            tabIndex={Platform.OS === "web" ? 0 : undefined}
            accessibilityLabel="物料表格，可左右滚动查看完整字段"
            style={styles.tableScroll}
          >
            <View
              role={Platform.OS === "web" ? "table" : undefined}
              accessibilityLabel="物料清单"
              style={{ width: 48 + columns.reduce((sum, column) => sum + column.width, 0) }}
            >
              <View
                role={Platform.OS === "web" ? "row" : undefined}
                style={[styles.tableRow, styles.tableHeader]}
              >
                <Text
                  role={Platform.OS === "web" ? "columnheader" : undefined}
                  style={[styles.cell, styles.sequence, styles.header]}
                >
                  序号
                </Text>
                {columns.map((column) => (
                  <Text
                    key={column.key}
                    role={Platform.OS === "web" ? "columnheader" : undefined}
                    style={[styles.cell, styles.header, { width: column.width }]}
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

const styles = StyleSheet.create({
  cards: { gap: 12 },
  searchBox: {
    flexDirection: "row",
    alignItems: "center",
    minHeight: 48,
    borderWidth: 1,
    borderColor: color.line,
    borderRadius: 12,
    backgroundColor: "#f3f6f3",
  },
  searchInput: {
    flex: 1,
    minWidth: 0,
    minHeight: 46,
    paddingHorizontal: 12,
    paddingVertical: 10,
    fontSize: 13,
    color: color.ink,
  },
  clearSearch: { minWidth: 44, minHeight: 44, alignItems: "center", justifyContent: "center" },
  clearSearchText: { fontSize: 22, color: color.muted },
  searchStatus: { flexDirection: "row", alignItems: "center", gap: 8, minHeight: 24 },
  card: {
    borderWidth: 1,
    borderColor: color.line,
    borderRadius: 10,
    padding: 14,
    gap: 10,
    backgroundColor: "#fff",
  },
  cardHeading: { color: color.green, fontSize: 12, fontWeight: "600", marginBottom: 2 },
  field: { flexDirection: "row", alignItems: "flex-start", gap: 12 },
  fieldLabel: { width: 84, flexShrink: 0 },
  label: { color: color.muted, fontSize: 12, lineHeight: 22 },
  value: { flex: 1, minWidth: 0, color: color.ink, fontSize: 13, lineHeight: 22 },
  name: { fontWeight: "600" },
  quantity: { color: color.green, fontWeight: "600" },
  tableContainer: { gap: 8 },
  hint: { color: color.muted, fontSize: 12, lineHeight: 20 },
  tableScroll: { borderWidth: 1, borderColor: color.line, borderRadius: 10 },
  tableRow: { flexDirection: "row", borderBottomWidth: 1, borderBottomColor: color.line },
  alternate: { backgroundColor: "#f7faf8" },
  cell: { paddingHorizontal: 12, paddingVertical: 14, flexShrink: 0 },
  sequence: { width: 48 },
  tableHeader: { backgroundColor: "#edf4ef" },
  header: { color: "#42574b", fontSize: 12, lineHeight: 22, fontWeight: "600" },
});

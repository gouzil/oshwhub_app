import { StyleSheet, View } from "react-native";

export function NavigationIcon({ kind, color }: { kind: string; color: string }) {
  const stroke = { borderColor: color };
  const fill = { backgroundColor: color };
  return (
    <View style={styles.icon} aria-hidden accessibilityElementsHidden>
      <View style={[kind === "cpu" ? styles.chip : styles.ring, stroke]} />
      {kind === "safari" ? (
        <View style={[styles.needle, stroke]} />
      ) : kind === "cpu" ? (
        <>
          <View style={[styles.core, stroke]} />
          {[7, 15].map((offset) => (
            <View key={offset} style={StyleSheet.absoluteFill}>
              <View style={[styles.pinVertical, { left: offset, top: 0 }, fill]} />
              <View style={[styles.pinVertical, { left: offset, bottom: 0 }, fill]} />
              <View style={[styles.pinHorizontal, { top: offset, left: 0 }, fill]} />
              <View style={[styles.pinHorizontal, { top: offset, right: 0 }, fill]} />
            </View>
          ))}
        </>
      ) : (
        <>
          <View style={[styles.dot, fill]} />
          <View style={[styles.stem, fill]} />
        </>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  icon: { width: 24, height: 24 },
  ring: { position: "absolute", inset: 1, borderRadius: 12, borderWidth: 1.7 },
  needle: {
    position: "absolute",
    left: 9,
    top: 5,
    width: 6,
    height: 14,
    borderWidth: 1.7,
    borderRadius: 1,
    transform: [{ rotate: "40deg" }],
  },
  chip: { position: "absolute", inset: 4, borderRadius: 4, borderWidth: 1.7 },
  core: { position: "absolute", inset: 8, borderRadius: 1.5, borderWidth: 1.5 },
  pinVertical: { position: "absolute", width: 1.7, height: 4, borderRadius: 1 },
  pinHorizontal: { position: "absolute", width: 4, height: 1.7, borderRadius: 1 },
  dot: { position: "absolute", left: 11, top: 6, width: 2, height: 2, borderRadius: 1 },
  stem: { position: "absolute", left: 11, top: 11, width: 2, height: 7, borderRadius: 1 },
});

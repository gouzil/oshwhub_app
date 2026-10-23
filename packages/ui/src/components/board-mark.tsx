import { StyleSheet, View } from "react-native";

export function BoardMark({ small = false }: { small?: boolean }) {
  return (
    <View accessible={false} style={[common.boardMark, small && common.boardMarkSmall]}>
      <View style={common.boardChip} />
      <View style={common.boardTraceA} />
      <View style={common.boardTraceB} />
      <View style={common.boardPinA} />
      <View style={common.boardPinB} />
    </View>
  );
}

const common = StyleSheet.create({
  boardMark: {
    width: 64,
    height: 48,
    borderWidth: 1.5,
    borderColor: "#a6c8b2",
    borderRadius: 7,
    transform: [{ rotate: "-7deg" }],
  },
  boardMarkSmall: { width: 58, height: 44 },
  boardChip: {
    position: "absolute",
    width: 18,
    height: 18,
    left: 20,
    top: 13,
    borderRadius: 3,
    borderWidth: 1.5,
    borderColor: "#89b39b",
    backgroundColor: "#d6e7db",
  },
  boardTraceA: {
    position: "absolute",
    left: 7,
    top: 17,
    width: 12,
    height: 1.5,
    backgroundColor: "#a6c8b2",
  },
  boardTraceB: {
    position: "absolute",
    right: 7,
    top: 26,
    width: 16,
    height: 1.5,
    backgroundColor: "#a6c8b2",
  },
  boardPinA: {
    position: "absolute",
    right: 8,
    top: 7,
    width: 4,
    height: 4,
    borderRadius: 3,
    backgroundColor: "#a6c8b2",
  },
  boardPinB: {
    position: "absolute",
    left: 8,
    bottom: 7,
    width: 4,
    height: 4,
    borderRadius: 3,
    backgroundColor: "#a6c8b2",
  },
});

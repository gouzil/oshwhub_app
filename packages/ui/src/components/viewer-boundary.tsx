import { Component, type ReactNode } from "react";
import { ScrollView, StyleSheet } from "react-native";
import { color } from "../theme";
import { ErrorPanel } from "./states";

export class ViewerBoundary extends Component<
  { children: ReactNode; retry: () => void },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    return this.state.error ? (
      <ScrollView contentContainerStyle={styles.error}>
        <ErrorPanel
          error={this.state.error}
          retry={() => {
            this.setState({ error: null });
            this.props.retry();
          }}
        />
      </ScrollView>
    ) : (
      this.props.children
    );
  }
}

const styles = StyleSheet.create({
  error: { padding: 20, flexGrow: 1, justifyContent: "center", backgroundColor: color.canvas },
});

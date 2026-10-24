import { Component, type ReactNode } from "react";
import { ScrollView } from "react-native";
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
      <ScrollView contentContainerClassName="grow justify-center bg-canvas p-5">
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

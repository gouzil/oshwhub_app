import { type Command, call } from "@oshwhub/bridge";
import { useEffect, useState } from "react";

export function useCommand<T>(command: Command) {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<{ value: T | null; error: Error | null; loading: boolean }>({
    value: null,
    error: null,
    loading: true,
  });
  // biome-ignore lint/correctness/useExhaustiveDependencies: The attempt counter retries the same command.
  useEffect(() => {
    let active = true;
    setState({ value: null, error: null, loading: true });
    call<T>(command).then(
      (value) => {
        if (active) setState({ value, error: null, loading: false });
      },
      (error: unknown) => {
        if (active)
          setState({
            value: null,
            error: error instanceof Error ? error : new Error("暂时无法加载，请重试。"),
            loading: false,
          });
      },
    );
    return () => {
      active = false;
    };
  }, [command, attempt]);
  return { ...state, retry: () => setAttempt((n) => n + 1) };
}

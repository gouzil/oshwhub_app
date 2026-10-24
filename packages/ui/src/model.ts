import type { FeedPage, ProjectSummary, Sort } from "@oshwhub/bridge";

type FeedRequest = { query: string; sort: Sort; page: number; pageSize: number };
type FeedSnapshot = {
  items: ProjectSummary[];
  loading: boolean;
  refreshing: boolean;
  error: Error | null;
  refreshError: Error | null;
  page: number;
  hasMore: boolean;
  total: number;
};

export class FeedController {
  private snapshot: FeedSnapshot = {
    items: [],
    loading: false,
    refreshing: false,
    error: null,
    refreshError: null,
    page: 0,
    hasMore: true,
    total: 0,
  };
  private version = 0;
  private query = "";
  private sort: Sort = "recommend";
  private listeners = new Set<() => void>();

  constructor(private load: (request: FeedRequest) => Promise<FeedPage>) {}
  getSnapshot = () => this.snapshot;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private update(snapshot: FeedSnapshot) {
    this.snapshot = snapshot;
    for (const listener of this.listeners) listener();
  }
  search = async (query: string, sort: Sort) => {
    this.version += 1;
    this.query = query;
    this.sort = sort;
    this.update({
      items: [],
      loading: false,
      refreshing: false,
      error: null,
      refreshError: null,
      page: 0,
      hasMore: true,
      total: 0,
    });
    await this.loadMore();
  };
  loadMore = async () => {
    if (this.snapshot.loading || this.snapshot.refreshing || !this.snapshot.hasMore) return;
    await this.loadPage(this.snapshot.page + 1, false);
  };
  refresh = async () => {
    if (this.snapshot.refreshing) return;
    this.version += 1;
    await this.loadPage(1, true);
  };
  private async loadPage(page: number, refreshing: boolean) {
    const version = this.version;
    this.update({
      ...this.snapshot,
      loading: !refreshing,
      refreshing,
      error: null,
      refreshError: null,
    });
    try {
      const result = await this.load({ page, query: this.query, sort: this.sort, pageSize: 20 });
      if (version !== this.version) return;
      const items = page === 1 ? [] : [...this.snapshot.items];
      const seen = new Set(items.map((item) => item.id));
      for (const item of result.items) {
        if (!seen.has(item.id)) {
          seen.add(item.id);
          items.push(item);
        }
      }
      this.update({
        items,
        page,
        loading: false,
        refreshing: false,
        error: null,
        refreshError: null,
        hasMore: result.hasMore,
        total: result.total,
      });
    } catch (error) {
      if (version !== this.version) return;
      const failure = error instanceof Error ? error : new Error("暂时无法加载，请重试。");
      this.update({
        ...this.snapshot,
        loading: false,
        refreshing: false,
        error: refreshing ? null : failure,
        refreshError: refreshing ? failure : null,
      });
    }
  }
}
export function layoutFor(width: number) {
  const sidebar = width >= 1024 ? 216 : 0;
  const content = Math.min(width - sidebar, 1440);
  const padding = width < 400 ? 16 : 24;
  const gap = width < 620 ? 12 : 16;
  const columns = content >= 1100 ? 5 : content >= 900 ? 4 : content >= 620 ? 3 : 2;
  return {
    columns,
    cardWidth: Math.floor((content - padding * 2 - gap * (columns - 1)) / columns),
    gap,
    padding,
    sidebar,
  };
}
export function safeUrl(value: string | null | undefined): string | null {
  if (!value) return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:" ? url.href : null;
  } catch {
    return null;
  }
}

export function imagePreviewUrl(
  value: string | null | undefined,
  pixelWidth: number,
): string | null {
  const uri = safeUrl(value);
  if (!uri) return null;
  const url = new URL(uri);
  // Existing queries may be signed or already sized by the author.
  if (
    url.hostname !== "image.lceda.cn" ||
    url.search ||
    !Number.isFinite(pixelWidth) ||
    pixelWidth <= 0
  )
    return uri;
  const width = [320, 640, 960, 1280, 1920].find((size) => size >= pixelWidth) ?? 1920;
  url.searchParams.set("x-oss-process", `image/resize,w_${width}/quality,q_80/format,webp`);
  return url.href;
}

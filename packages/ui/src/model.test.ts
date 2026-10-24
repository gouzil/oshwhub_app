import type { FeedPage, ProjectSummary } from "@oshwhub/bridge";
import { describe, expect, it } from "vitest";
import { FeedController, imagePreviewUrl, layoutFor, safeUrl } from "./model";

const project = (id: string): ProjectSummary => ({
  id,
  name: id,
  path: id,
  description: "",
  cover: null,
  author: "作者",
  authorAvatar: null,
  likes: 0,
  views: 0,
  tags: [],
  updatedAt: null,
  url: `https://oshwhub.com/author/${id}`,
});
const page = (ids: string[], n = 1, hasMore = true): FeedPage => ({
  items: ids.map(project),
  page: n,
  total: 50,
  hasMore,
});
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
};

describe("feed requests", () => {
  it("keeps the new search when an older request completes last", async () => {
    const old = deferred<FeedPage>();
    const current = deferred<FeedPage>();
    const feed = new FeedController(({ query }) =>
      query === "new" ? current.promise : old.promise,
    );
    const first = feed.search("old", "recommend");
    const second = feed.search("new", "new");
    current.resolve(page(["new"]));
    await second;
    old.resolve(page(["old"]));
    await first;
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["new"]);
  });

  it("retries the failed page and appends each project only once", async () => {
    const requested: number[] = [];
    let fail = true;
    const feed = new FeedController(async ({ page: n }) => {
      requested.push(n);
      if (n === 2 && fail) {
        fail = false;
        throw new Error("离线");
      }
      return n === 1 ? page(["a", "b"]) : page(["b", "c"], 2, false);
    });
    await feed.search("", "recommend");
    await feed.loadMore();
    expect(feed.getSnapshot().page).toBe(1);
    expect(feed.getSnapshot().error?.message).toBe("离线");
    await feed.loadMore();
    await feed.loadMore();
    expect(requested).toEqual([1, 2, 2]);
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["a", "b", "c"]);
  });

  it("ignores duplicate load requests and stale errors", async () => {
    const old = deferred<FeedPage>();
    const feed = new FeedController(({ query }) =>
      query === "old" ? old.promise : Promise.resolve(page(["b"])),
    );
    const first = feed.search("old", "like");
    await feed.loadMore();
    await feed.search("new", "like");
    old.reject(new Error("过期错误"));
    await first;
    expect(feed.getSnapshot().error).toBeNull();
    expect(feed.getSnapshot().items[0]?.id).toBe("b");
  });

  it("keeps cards while refreshing, replaces page one, and resumes the current search", async () => {
    const fresh = deferred<FeedPage>();
    const requests: { query: string; sort: string; page: number }[] = [];
    const feed = new FeedController((request) => {
      requests.push(request);
      if (requests.length === 3) return fresh.promise;
      return Promise.resolve(page([request.page === 1 ? "old" : "next"], request.page));
    });
    await feed.search("电源", "like");
    await feed.loadMore();
    const refreshing = feed.refresh();
    await feed.refresh();
    await feed.loadMore();
    expect(requests).toHaveLength(3);
    expect(requests[2]).toMatchObject({ query: "电源", sort: "like", page: 1 });
    expect(feed.getSnapshot()).toMatchObject({ refreshing: true, loading: false, page: 2 });
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["old", "next"]);
    fresh.resolve(page(["fresh", "fresh"]));
    await refreshing;
    expect(feed.getSnapshot()).toMatchObject({ refreshing: false, page: 1, hasMore: true });
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["fresh"]);
    await feed.loadMore();
    expect(requests[3]).toMatchObject({ query: "电源", sort: "like", page: 2 });
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["fresh", "next"]);
  });

  it("can refresh an exhausted feed and keeps it intact after a refresh failure", async () => {
    let attempt = 0;
    const feed = new FeedController(async () => {
      attempt += 1;
      if (attempt === 2) throw new Error("刷新离线");
      return page([attempt === 1 ? "old" : "fresh"], 1, false);
    });
    await feed.search("", "recommend");
    await feed.refresh();
    expect(feed.getSnapshot()).toMatchObject({ refreshing: false, page: 1, hasMore: false });
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["old"]);
    expect(feed.getSnapshot().refreshError?.message).toBe("刷新离线");
    expect(feed.getSnapshot().error).toBeNull();
    await feed.refresh();
    expect(feed.getSnapshot().refreshError).toBeNull();
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["fresh"]);
  });

  it.each([
    false,
    true,
  ])("ignores superseded pagination after refresh (failure: %s)", async (fail) => {
    const old = deferred<FeedPage>();
    let first = true;
    const feed = new FeedController(({ page: n }) => {
      if (n === 2) return old.promise;
      const result = page([first ? "old" : "fresh"]);
      first = false;
      return Promise.resolve(result);
    });
    await feed.search("", "new");
    const paging = feed.loadMore();
    await feed.refresh();
    if (fail) old.reject(new Error("过期分页错误"));
    else old.resolve(page(["stale"], 2, false));
    await paging;
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["fresh"]);
    expect(feed.getSnapshot()).toMatchObject({ page: 1, error: null, refreshError: null });
  });

  it.each([
    false,
    true,
  ])("keeps a newer search after an old refresh (failure: %s)", async (fail) => {
    const old = deferred<FeedPage>();
    let attempt = 0;
    const feed = new FeedController(({ query }) => {
      attempt += 1;
      return attempt === 2 ? old.promise : Promise.resolve(page([query]));
    });
    await feed.search("old", "recommend");
    const refreshing = feed.refresh();
    await feed.search("new", "like");
    if (fail) old.reject(new Error("过期刷新错误"));
    else old.resolve(page(["stale"]));
    await refreshing;
    expect(feed.getSnapshot().items.map((item) => item.id)).toEqual(["new"]);
    expect(feed.getSnapshot()).toMatchObject({
      refreshing: false,
      refreshError: null,
      error: null,
    });
  });
});

describe("responsive layout", () => {
  it.each([
    [320, 2],
    [390, 2],
    [768, 3],
    [1024, 3],
    [1440, 5],
  ])("%ipx keeps its cards within the available content width", (width, columns) => {
    const layout = layoutFor(width);
    expect(layout.columns).toBe(columns);
    expect(layout.cardWidth).toBeGreaterThanOrEqual(130);
    expect(
      layout.cardWidth * columns + layout.gap * (columns - 1) + layout.padding * 2,
    ).toBeLessThanOrEqual(width - layout.sidebar);
  });
});

describe("external content URLs", () => {
  it("accepts http(s) and rejects script, local, and malformed targets", () => {
    expect(safeUrl("https://oshwhub.com/a")).toBe("https://oshwhub.com/a");
    expect(safeUrl("http://example.com/a")).toBe("http://example.com/a");
    for (const url of [
      "javascript:alert(1)",
      "file:///etc/passwd",
      "data:text/html,x",
      "not a URL",
      null,
    ]) {
      expect(safeUrl(url)).toBeNull();
    }
  });
});

describe("display-sized project images", () => {
  const source = "https://image.lceda.cn/oshwhub/pullImage/self-made.png";
  it("requests a bounded CDN rendition for the rendered pixel width", () => {
    const preview = new URL(imagePreviewUrl(source, 900) as string);
    expect(preview.searchParams.get("x-oss-process")).toBe(
      "image/resize,w_960/quality,q_80/format,webp",
    );
    expect(new URL(imagePreviewUrl(source, 8000) as string).searchParams.get("x-oss-process")).toBe(
      "image/resize,w_1920/quality,q_80/format,webp",
    );
  });
  it("preserves signed URLs, existing renditions, and other hosts", () => {
    for (const url of [
      `${source}?token=secret&expires=123`,
      `${source}?x-oss-process=image/resize,w_320`,
      "https://example.com/original.png",
      "https://image.lceda.cn.example.com/original.png",
    ]) {
      expect(imagePreviewUrl(url, 640)).toBe(url);
    }
    expect(imagePreviewUrl("javascript:alert(1)", 640)).toBeNull();
    expect(imagePreviewUrl(source, Number.NaN)).toBe(source);
  });
});

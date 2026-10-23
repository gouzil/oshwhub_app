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

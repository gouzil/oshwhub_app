import { expect, test } from "./fixtures";

test("project collection and comments refresh from server state", async ({ page, bridge }) => {
  bridge.authLoggedIn = true;
  bridge.comments = [
    {
      uuid: "comment-1",
      content: "已有一条评论",
      creator: bridge.authUser,
      createdAt: "2026-10-24 12:00:00",
      like: false,
      replies: [],
    },
  ];
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await expect(page.getByRole("button", { name: "收藏项目", exact: true })).toBeVisible();
  await expect(page.getByText("已有一条评论", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "收藏项目", exact: true }).click();
  await expect(page.getByRole("button", { name: "取消收藏", exact: true })).toBeVisible();
  expect(bridge.requests).toContainEqual({
    command: "toggleProjectCollection",
    id: "self-made-1",
    collect: true,
  });

  await page.getByRole("textbox", { name: "发表评论", exact: true }).fill("新评论");
  await page.getByRole("button", { name: "发表评论", exact: true }).click();
  await expect(page.getByText("新评论", { exact: true })).toBeVisible();
  expect(bridge.requests).toContainEqual({
    command: "createComment",
    id: "self-made-1",
    content: "新评论",
  });
});

test("guest collection action routes to the account tab", async ({ page, bridge }) => {
  bridge.comments = [
    {
      uuid: "guest-comment",
      content: "访客可见评论",
      creator: bridge.authUser,
      createdAt: "2026-10-24 12:00:00",
      like: false,
      replies: [],
    },
  ];
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await expect(page.getByText("访客可见评论", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "收藏项目", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "收藏项目", exact: true }).click();
  await expect(page.getByText("登录 OSHWHub", { exact: true })).toBeVisible();
});

test("failed collection preserves server state and request id", async ({ page, bridge }) => {
  bridge.authLoggedIn = true;
  bridge.interactionError = {
    code: "BLOCKED",
    message: "收藏失败",
    requestId: "collect-e2e-request",
  };
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await page.getByRole("button", { name: "收藏项目", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("collect-e2e-request");
  await expect(page.getByRole("button", { name: "收藏项目", exact: true })).toBeVisible();
  expect(bridge.requests).toContainEqual({
    command: "toggleProjectCollection",
    id: "self-made-1",
    collect: true,
  });
});

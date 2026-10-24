import {
  type Comment,
  type CommentsPage,
  type ContentBlock,
  call,
  type ProjectDetail,
  type ProjectStatus,
} from "@oshwhub/bridge";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FlatList, Platform, ScrollView, View } from "react-native";
import { BomList } from "../BomList";
import { ActionButton as Button } from "../components/action-button";
import { BoardOption } from "../components/board-option";
import { Cover } from "../components/cover";
import { ExternalLink } from "../components/external-link";
import { PageHeader } from "../components/page-header";
import { ProjectContent } from "../components/project-content";
import { Empty, ErrorPanel } from "../components/states";
import { TabBar } from "../components/tab-bar";
import { Input } from "../components/ui/input";
import { Text } from "../components/ui/text";
import { useCommand } from "../hooks/use-command";
import { cn } from "../lib/utils";
import { Skeleton, SkeletonBlock } from "../loading";

export function ProjectPage({
  id,
  width,
  bottomInset,
  onBack,
  onPcb,
  authenticated,
  authVersion,
  onAccount,
}: {
  id: string;
  width: number;
  bottomInset: number;
  onBack: () => void;
  onPcb: (documentId: string, name: string) => void;
  authenticated: boolean;
  authVersion: number;
  onAccount: () => void;
}) {
  const command = useMemo(() => ({ command: "project" as const, id }), [id]);
  const { value: project, loading, error, retry } = useCommand<ProjectDetail>(command);
  const [tab, setTab] = useState<"intro" | "hardware">("intro");
  const [boardId, setBoardId] = useState<string | null>(null);
  const [status, setStatus] = useState<ProjectStatus | null>(null);
  const [statusError, setStatusError] = useState<Error | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [collectionBusy, setCollectionBusy] = useState(false);
  const [interactionError, setInteractionError] = useState<Error | null>(null);
  const [comments, setComments] = useState<CommentsPage | null>(null);
  const [commentsError, setCommentsError] = useState<Error | null>(null);
  const [commentDraft, setCommentDraft] = useState("");
  const [commentBusy, setCommentBusy] = useState(false);
  const list = useRef<FlatList<ContentBlock>>(null);
  const [readyThrough, setReadyThrough] = useState(-1);
  const viewabilityConfig = useRef({ itemVisiblePercentThreshold: 1 }).current;
  const onViewableItemsChanged = useRef(
    ({ viewableItems }: { viewableItems: { index?: number | null }[] }) => {
      const last = Math.max(-1, ...viewableItems.map((item) => item.index ?? -1));
      if (last >= 0) setReadyThrough((previous) => Math.max(previous, last + 1));
    },
  ).current;
  const selected = project?.boards.find((board) => board.id === boardId) ?? project?.boards[0];
  const wide = width >= 900;
  const loadInteractions = useCallback(() => {
    if (!project) return;
    let active = true;
    setStatusLoading(true);
    setStatusError(null);
    setCommentsError(null);
    void Promise.allSettled([
      call<ProjectStatus>({ command: "projectStatus", id }),
      call<CommentsPage>({ command: "comments", id, sort: "hot", page: 1, pageSize: 20 }),
    ])
      .then(([statusResult, commentsResult]) => {
        if (!active) return;
        if (statusResult.status === "fulfilled") setStatus(statusResult.value);
        else {
          const details =
            statusResult.reason instanceof Error
              ? statusResult.reason
              : new Error("暂时无法加载收藏状态，请重试。");
          if ((statusResult.reason as { code?: string })?.code === "AUTH_REQUIRED") {
            setStatusError(details);
          } else {
            setInteractionError(details);
          }
        }
        if (commentsResult.status === "fulfilled") setComments(commentsResult.value);
        else {
          setCommentsError(
            commentsResult.reason instanceof Error
              ? commentsResult.reason
              : new Error("暂时无法加载评论，请重试。"),
          );
        }
      })
      .finally(() => {
        if (active) setStatusLoading(false);
      });
    return () => {
      active = false;
    };
  }, [id, project]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: Authentication changes must refresh session-bound project state.
  useEffect(() => loadInteractions(), [authVersion, loadInteractions]);
  const collectionLabel =
    statusError && (statusError as { code?: string }).code === "AUTH_REQUIRED"
      ? "登录后收藏"
      : status?.collect
        ? "取消收藏"
        : "收藏项目";
  const validComment =
    commentDraft.trim().length > 0 &&
    commentDraft.trim().length <= 2000 &&
    ![...commentDraft].some((character) => /\p{Cc}/u.test(character));
  const toggleCollection = () => {
    if (statusError && (statusError as { code?: string }).code === "AUTH_REQUIRED") {
      onAccount();
      return;
    }
    if (!status || collectionBusy) return;
    const collect = !status.collect;
    setCollectionBusy(true);
    setInteractionError(null);
    void call<ProjectStatus>({ command: "toggleProjectCollection", id, collect })
      .then(setStatus, (error: unknown) => {
        const details = error instanceof Error ? error : new Error("收藏操作失败，请重试。");
        if ((error as { code?: string })?.code === "AUTH_REQUIRED") onAccount();
        else setInteractionError(details);
      })
      .finally(() => setCollectionBusy(false));
  };
  const createComment = () => {
    if (!authenticated) {
      onAccount();
      return;
    }
    const content = commentDraft.trim();
    if (!validComment || commentBusy) return;
    setCommentBusy(true);
    setInteractionError(null);
    void call<Comment>({ command: "createComment", id, content })
      .then(
        () => {
          setCommentDraft("");
          return call<CommentsPage>({
            command: "comments",
            id,
            sort: "hot",
            page: 1,
            pageSize: 20,
          });
        },
        (error: unknown) => {
          const details = error instanceof Error ? error : new Error("发表评论失败，请重试。");
          if ((error as { code?: string })?.code === "AUTH_REQUIRED") onAccount();
          else setInteractionError(details);
          return null;
        },
      )
      .then((next) => {
        if (next) setComments(next);
      })
      .finally(() => setCommentBusy(false));
  };
  return (
    <View className="min-h-0 flex-1">
      <PageHeader title={project?.summary.name ?? "项目详情"} onBack={onBack} />
      {loading ? (
        <ScrollView
          contentContainerClassName={cn(
            "w-full max-w-[1120px] self-center",
            width < 400 ? "p-4" : "p-7",
          )}
          contentContainerStyle={{ paddingBottom: 40 + bottomInset }}
        >
          <Skeleton label="正在打开项目" className="gap-6">
            <View className={cn("gap-6", wide && "flex-row items-start gap-8")}>
              <SkeletonBlock
                className={cn(
                  "h-[190px] w-full overflow-hidden rounded-[18px]",
                  wide && "h-[260px] w-[44%]",
                )}
              />
              <View className="min-w-0 flex-1 gap-3">
                <SkeletonBlock className="h-3 w-[30%]" />
                <SkeletonBlock className="h-[30px] w-[85%]" />
                <SkeletonBlock className="w-1/2" />
                <SkeletonBlock />
                <SkeletonBlock className="w-[70%]" />
              </View>
            </View>
            <SkeletonBlock className="h-11 w-[60%]" />
            <SkeletonBlock />
            <SkeletonBlock />
            <SkeletonBlock className="w-[70%]" />
          </Skeleton>
        </ScrollView>
      ) : error ? (
        <ScrollView
          contentContainerClassName="grow justify-center p-5"
          contentContainerStyle={{ paddingBottom: 40 + bottomInset }}
        >
          <ErrorPanel error={error} retry={retry} />
        </ScrollView>
      ) : (
        project && (
          <FlatList<ContentBlock>
            ref={list}
            testID="project-content-list"
            keyboardShouldPersistTaps="handled"
            className="flex-1"
            contentContainerClassName={cn(
              "w-full max-w-[1120px] self-center",
              width < 400 ? "p-4" : "p-7",
            )}
            contentContainerStyle={{ paddingBottom: 40 + bottomInset }}
            data={tab === "intro" ? project.content : []}
            keyExtractor={(item, index) => `${index}-${item.kind}`}
            initialNumToRender={4}
            maxToRenderPerBatch={4}
            windowSize={3}
            removeClippedSubviews={Platform.OS === "android"}
            onViewableItemsChanged={onViewableItemsChanged}
            viewabilityConfig={viewabilityConfig}
            extraData={readyThrough}
            renderItem={({ item, index }) => (
              <View className="pt-[18px]">
                <ProjectContent block={item} visible={index <= readyThrough} />
              </View>
            )}
            ListHeaderComponent={
              <>
                <View className={cn("gap-6", wide && "flex-row items-start gap-8")}>
                  <View className={cn("w-full overflow-hidden rounded-[18px]", wide && "w-[44%]")}>
                    <Cover
                      url={project.summary.cover}
                      title={project.summary.name}
                      aspectRatio={wide ? 1.35 : 1.7}
                    />
                  </View>
                  <View className="min-w-0 flex-1 gap-3">
                    <Text className="text-[11px] font-semibold tracking-[1px] text-primary">
                      开源项目
                    </Text>
                    <Text
                      selectable
                      className="text-[25px] leading-[35px] font-bold text-foreground"
                    >
                      {project.summary.name}
                    </Text>
                    <Text className="text-[13px] text-muted-foreground">
                      由 {project.summary.author || "开源创作者"} 分享
                    </Text>
                    {project.summary.description.length > 0 && (
                      <Text selectable className="text-[14px] leading-6 text-[#56695e]">
                        {project.summary.description}
                      </Text>
                    )}
                    <View className="flex-row flex-wrap gap-1.5">
                      {[...new Set(project.summary.tags)].map((tag) => (
                        <View key={tag} className="rounded-[6px] bg-secondary px-[9px] py-[5px]">
                          <Text className="text-[11px] text-[#568065]">{tag}</Text>
                        </View>
                      ))}
                    </View>
                    <Text className="text-[12px] text-[#8a978e]">
                      ♡ {project.summary.likes.toLocaleString()} 喜欢{" "}
                      {project.summary.views.toLocaleString()} 次浏览
                    </Text>
                    <ExternalLink url={project.summary.url} label="在原网站中查看" />
                    <View className="gap-2 pt-1">
                      <Button
                        label={collectionLabel}
                        secondary={status?.collect ?? false}
                        disabled={statusLoading || collectionBusy}
                        onPress={toggleCollection}
                      />
                      {interactionError && (
                        <ErrorPanel
                          error={interactionError}
                          retry={() => setInteractionError(null)}
                        />
                      )}
                    </View>
                  </View>
                </View>
                <TabBar
                  items={[
                    { value: "intro", label: "项目介绍" },
                    { value: "hardware", label: `板子与 BOM (${project.boards.length})` },
                  ]}
                  value={tab}
                  onValueChange={(next) => {
                    setTab(next);
                    list.current?.scrollToOffset({ offset: 0, animated: false });
                  }}
                  accessibilityLabel="项目内容"
                  variant="underline"
                  className="mt-7 flex-wrap gap-3.5 border-b border-border"
                />
                {tab === "hardware" && (
                  <View className="gap-4 pt-6">
                    {project.boards.length ? (
                      <>
                        <Text className="text-[17px] leading-[26px] font-semibold text-foreground">
                          选择板子
                        </Text>
                        <ScrollView
                          horizontal
                          showsHorizontalScrollIndicator={false}
                          contentContainerClassName="gap-3 py-1"
                        >
                          {project.boards.map((board) => (
                            <BoardOption
                              key={board.id}
                              board={board}
                              selected={selected?.id === board.id}
                              onPress={() => setBoardId(board.id)}
                            />
                          ))}
                        </ScrollView>
                        {selected && (
                          <View className="border-t border-border pt-4">
                            <View className="mb-[26px] flex-row flex-wrap items-center justify-between gap-4">
                              <View className="shrink gap-1.5">
                                <Text className="text-[17px] leading-[26px] font-semibold text-foreground">
                                  {selected.name || "未命名板子"}
                                </Text>
                                <Text className="text-[12px] text-muted-foreground">
                                  {selected.bom.length} 种物料 ·{" "}
                                  {selected.bom.reduce((sum, item) => sum + item.quantity, 0)}{" "}
                                  个元件
                                </Text>
                              </View>
                              <Button
                                label={selected.pcbId ? "查看 PCB" : "未公开 PCB 文件"}
                                disabled={!selected.pcbId}
                                onPress={() => {
                                  if (selected.pcbId) onPcb(selected.pcbId, selected.name);
                                }}
                              />
                            </View>
                            <Text className="mb-2 text-[12px] tracking-[0.5px] text-muted-foreground">
                              物料清单 / BOM
                            </Text>
                            {selected.bom.length ? (
                              <BomList key={selected.id} items={selected.bom} wide={width >= 720} />
                            ) : (
                              <Empty title="暂无物料清单" detail="作者尚未公开此板的 BOM。" />
                            )}
                          </View>
                        )}
                      </>
                    ) : (
                      <Empty title="暂无公开板子" detail="作者尚未公开板子与物料清单。" />
                    )}
                  </View>
                )}
              </>
            }
            ListEmptyComponent={
              tab === "intro" ? (
                <Empty title="作者尚未添加项目正文" detail="可以前往原网站查看项目的最新信息。" />
              ) : undefined
            }
            ListFooterComponent={
              tab === "intro" ? (
                <View className="gap-[18px] pt-6">
                  {project.attachments.length > 0 && (
                    <View className="border-t border-border pt-5">
                      <Text className="text-[17px] leading-[26px] font-semibold text-foreground">
                        项目附件
                      </Text>
                      {project.attachments.map((attachment, i) => (
                        <ExternalLink
                          key={`${attachment.url}-${i}`}
                          url={attachment.url}
                          label={attachment.name || "打开附件"}
                        />
                      ))}
                    </View>
                  )}
                  {project.boards.length > 0 && (
                    <Button
                      label="查看板子与物料清单"
                      secondary
                      onPress={() => {
                        setTab("hardware");
                        list.current?.scrollToOffset({ offset: 0, animated: false });
                      }}
                    />
                  )}
                  <CommentsSection
                    comments={comments}
                    error={commentsError}
                    draft={commentDraft}
                    onDraft={setCommentDraft}
                    onSubmit={createComment}
                    authenticated={authenticated}
                    busy={commentBusy}
                    onAccount={onAccount}
                    onRetry={loadInteractions}
                  />
                </View>
              ) : undefined
            }
          />
        )
      )}
    </View>
  );
}

function CommentsSection({
  comments,
  error,
  draft,
  onDraft,
  onSubmit,
  authenticated,
  busy,
  onAccount,
  onRetry,
}: {
  comments: CommentsPage | null;
  error: Error | null;
  draft: string;
  onDraft: (value: string) => void;
  onSubmit: () => void;
  authenticated: boolean;
  busy: boolean;
  onAccount: () => void;
  onRetry: () => void;
}) {
  return (
    <View className="gap-4 border-t border-border pt-6">
      <Text accessibilityRole="header" className="text-[17px] font-semibold">
        评论 {comments?.total ?? 0}
      </Text>
      {error ? (
        <ErrorPanel error={error} retry={onRetry} />
      ) : comments ? (
        comments.items.length ? (
          <View className="gap-4">
            {comments.items.map((comment) => (
              <CommentItem key={comment.uuid} comment={comment} />
            ))}
          </View>
        ) : (
          <Text className="text-sm text-muted-foreground">还没有评论，欢迎分享你的想法。</Text>
        )
      ) : (
        <Text className="text-sm text-muted-foreground">正在加载评论…</Text>
      )}
      <View className="gap-2 rounded-2xl border border-border bg-card p-4">
        <Input
          accessibilityLabel="发表评论"
          placeholder={authenticated ? "写下你的评论" : "登录后发表评论"}
          value={draft}
          onChangeText={onDraft}
          multiline
          maxLength={2000}
          editable={authenticated && !busy}
          className="min-h-20 items-start py-3"
        />
        <Button
          label={authenticated ? "发表评论" : "登录后发表评论"}
          disabled={authenticated && (!draft.trim() || busy)}
          onPress={authenticated ? onSubmit : onAccount}
        />
      </View>
    </View>
  );
}

function CommentItem({ comment, depth = 0 }: { comment: Comment; depth?: number }) {
  return (
    <View className={cn("gap-1.5", depth > 0 && "ml-5 border-l border-border pl-3")}>
      <View className="flex-row items-center gap-2">
        <Text className="text-sm font-semibold">{comment.creator.nickname}</Text>
        <Text className="text-xs text-muted-foreground">{comment.createdAt}</Text>
      </View>
      <Text selectable className="text-sm leading-6 text-foreground">
        {comment.content}
      </Text>
      {comment.replies.map((reply) => (
        <CommentItem key={reply.uuid} comment={reply} depth={depth + 1} />
      ))}
    </View>
  );
}

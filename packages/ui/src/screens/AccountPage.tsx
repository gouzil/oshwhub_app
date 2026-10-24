import type { User } from "@oshwhub/bridge";
import { ScrollView, View } from "react-native";
import { ActionButton } from "../components/action-button";
import { ErrorPanel, Loading } from "../components/states";
import { Text } from "../components/ui/text";

export function AccountPage({
  user,
  loading,
  error,
  onLogin,
  onLogout,
  onRetry,
}: {
  user: User | null;
  loading: boolean;
  error: Error | null;
  onLogin: () => void;
  onLogout: () => void;
  onRetry: () => void;
}) {
  return (
    <ScrollView
      contentContainerClassName="min-h-full w-full max-w-[760px] self-center justify-center gap-6 p-6"
      accessibilityLabel="账户页面"
    >
      {loading ? (
        <Loading label="正在检查登录状态" />
      ) : error ? (
        <ErrorPanel error={error} retry={onRetry} />
      ) : user ? (
        <View className="items-center gap-4 rounded-3xl border border-border bg-card p-8 shadow-sm shadow-black/5">
          <Text accessibilityRole="header" className="text-2xl font-bold">
            {user.nickname}
          </Text>
          <Text className="text-sm text-muted-foreground">@{user.username}</Text>
          <Text className="text-center text-sm leading-6 text-muted-foreground">
            已连接 OSHWHub 账号，可以收藏项目并发表评论。
          </Text>
          <ActionButton label="退出登录" secondary onPress={onLogout} />
        </View>
      ) : (
        <View className="items-center gap-4 rounded-3xl border border-border bg-card p-8 shadow-sm shadow-black/5">
          <Text accessibilityRole="header" className="text-2xl font-bold">
            登录 OSHWHub
          </Text>
          <Text className="text-center text-sm leading-6 text-muted-foreground">
            登录会打开嘉立创官方 Passport 认证页。密码、验证码和扫码内容由官方页面处理。
          </Text>
          <ActionButton label="登录" onPress={onLogin} />
          <Text className="text-center text-xs leading-5 text-muted-foreground">
            当前版本只在应用进程内保存登录态，重启后需要重新登录。
          </Text>
        </View>
      )}
    </ScrollView>
  );
}

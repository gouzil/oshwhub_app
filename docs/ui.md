# 共享 UI 与 Tailwind

手机和平板使用 Expo / React Native，桌面使用 Electron / React Native Web，界面都来自 `@oshwhub/ui`。基础控件采用 [React Native Reusables](https://reactnativereusables.com/) 的 UniWind 版本，支持 Tailwind CSS 4；shadcn/ui 的 DOM 组件不能直接用于原生端。

## 分层

| 位置 | 职责 |
| --- | --- |
| `packages/ui/src/components/ui/` | Button、Text、Input、Card、Badge，保留上游变体与组合方式 |
| `packages/ui/src/components/` | SearchField、TabBar、ProjectCard、BoardOption、ProjectContent、导航、页头、封面、外链、加载/空/错误状态和查看器错误边界 |
| `packages/ui/src/screens/` | 发现、实验室、关于、项目详情、PCB 查看页面；只编排数据和组件 |
| `packages/ui/src/hooks/` | 共享命令加载与重试 |
| `packages/ui/src/App.tsx` | 当前页面、导航、安全区域和系统返回键 |
| `packages/ui/src/theme.css` | 所有端共同编译的 Tailwind 主题和源码扫描入口 |
| `packages/ui/src/theme.ts` | 保留既有 `color` 导出的兼容入口；新样式使用主题类 |

组件通过 `packages/ui/src/index.ts` 统一导出。移动 Metro 和桌面 Vite 分别接入 UniWind；两个入口引用同一份主题，不在页面重复安装或配置 Tailwind。生成的 `uniwind-types.d.ts` 保留类型支持，由构建工具更新。

## 使用

```tsx
import { ActionButton, Button, Card, CardContent, Text } from "@oshwhub/ui";

<Card className="gap-4 rounded-xl bg-card">
  <CardContent className="gap-3">
    <Text variant="h3">项目标题</Text>
    <Button variant="outline" onPress={onDetails}>
      <Text>查看详情</Text>
    </Button>
    <ActionButton label="打开 PCB" disabled={!pcbId} onPress={onOpen} />
  </CardContent>
</Card>
```

`Button` 支持 default、secondary、outline、ghost、destructive、link 和尺寸变体；`ActionButton` 适用于只有标签的操作，并统一可访问名称。触控目标至少 44px。使用库中的 `Text` 作为按钮、卡片和徽章子项，可继承对应的文本颜色；直接使用 React Native `Text` 时需自行提供颜色。

`SearchField` 复用于项目搜索和 BOM，支持受控 value 或非受控 defaultValue，并透传原生 input ref。首页保留非受控输入，防止 FlatList 重渲染覆盖正在输入的字符；搜索提交、清除和 BOM 防抖由页面负责。`TabBar` 复用于排序、详情分区及 2D/3D 模式，统一选中和禁用语义。

颜色优先使用 `bg-primary`、`text-muted-foreground`、`border-border` 等语义类。`cn()` 会合并条件类并解决 Tailwind 冲突；条件分支必须写出完整类名，避免拼接无法扫描的工具类。当前应用统一使用浅色主题。

应用壳、导航、页面、BOM、封面、图片、加载和错误状态均使用 UniWind 工具类，非渲染器 UI 已移除 `StyleSheet.create`。`RemoteImage`、`Skeleton` 和 `SkeletonBlock` 透传 `className`；列表使用 `contentContainerClassName` / `columnWrapperClassName`。静态尺寸、排版、状态色和 Web 玻璃效果都在组件中声明；桌面全局 CSS 只保留 Tailwind 导入与浏览器根节点、字体、焦点的基础规则。

`style` 仅用于运行时计算的卡片/侧栏宽度、安全区域间距、图片宽高比和导航/骨架动画。第三方 SafeArea、Expo 玻璃和原生 tab 宿主通过 `withUniwind` 接入；系统玻璃 API、降低动态效果/透明度设置仍按平台处理。复合项目卡和板子单选项保留有语义的 `Pressable` 并使用 UniWind，普通操作、文本、输入、卡片和徽章复用 Reusables。`packages/pcb-viewer` 内的 Skia、WebGL、手势及查看器控件样式不在本次迁移范围。

## 扩展与检查

新增基础控件先检查 Reusables，按需引入源码，并保留 MIT 许可与具体版本记录；当前来源和本地调整见 `packages/ui/vendor/react-native-reusables/`。不要将依赖 DOM 的 shadcn 组件复制到共享原生包。

```sh
pnpm check
pnpm build:web
pnpm test:e2e
pnpm test:desktop
pnpm --filter @oshwhub/mobile exec expo export --platform android --platform ios --output-dir ../../.work/mobile-export
```

E2E 自动在独立的 5273 端口启动当前工作树，避免误用其他工作区的 5173 服务。桌面实包还应检查 `oshwhub://app/`、preload → Rust、CanvasKit 和中文字体，不能仅凭浏览器预览通过判定打包成功。

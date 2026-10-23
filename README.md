# OSHWHub App

开源硬件社区的跨端只读客户端。React Native 共享手机、平板和桌面界面，Rust 负责公开 API、内容归一化、工程历史、几何与拾取。二维 PCB 引擎独立实现，使用 Skia 绘制，不依赖 EasyEDA 的预览器或闭源渲染引擎。

提供项目发现、搜索、排序、分页、详情、BOM、专业版 PCB 二维 / 三维查看，以及本地自制交互示例。打开 PCB 后点击右上角「3D」，可旋转、平移、缩放、查看顶底面及切换器件显隐；器件模型按工程绑定渐进加载，失败可重试。账号、评论、云收藏和编辑后续迭代。非官方客户端，项目内容及素材版权属于原作者。

## 开发

需要 Node.js 24、pnpm 10.32.1、Rust 1.98.1 和 prek。

```sh
pnpm install --frozen-lockfile
prek install
pnpm dev
```

打开 `http://127.0.0.1:5173`。浏览器开发预览通过本机 Rust 服务调用核心；移动安装包调用进程内 Rust；桌面通过私有标准输入输出连接随包 Rust 子进程。

```sh
pnpm check
pnpm test:e2e
prek run --all-files
```

## 平台

```sh
pnpm android    # Android SDK/NDK、JDK 21，默认 emulator-5554
pnpm ios        # macOS、Xcode、CocoaPods
pnpm desktop          # Electron + React Native Web 开发窗口
pnpm build:desktop    # 当前系统与架构的安装包
pnpm build:desktop --dir # 当前系统的可运行目录包
```

Android 提供 arm64-v8a / x86_64；iOS 支持 iPhone 和 iPad；桌面采用 Electron 自带 Chromium，支持 macOS、Windows、Linux。共享组件采用 React Native Reusables + UniWind / Tailwind CSS 4。原生目录和 Rust 库由构建命令生成。详细依赖、签名和构建产物路径见开发文档。

## 文档

- [架构与设计](docs/design.md)
- [实施阶段](docs/plan.md)
- [开发、日志与平台构建](docs/development.md)
- [共享组件与 Tailwind 使用](docs/ui.md)
- [公开 API 契约](docs/api.md)
- [独立 PCB 引擎、格式覆盖与性能边界](docs/renderer.md)
- [三维设计、模型来源与限制](docs/3d.md)
- [验收结果](docs/validation.md)

当前 PCB 数据入口针对专业版公开工程。已实现图元和已知格式限制见渲染文档；无法完整解析的内容必须显示错误或警告。请附平台、工程 ID、错误码和 requestId 报告问题。

本项目代码采用 MIT；React Native Reusables 组件保留上游 MIT 许可，随包字体使用 SIL OFL。公开工程的许可各自独立，不能据本仓库许可重新分发作者的 PCB 数据。

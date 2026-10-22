# PCB 查看器

`PcbViewer({ scene, onPick?, fontSource? })` 消费 Rust 输出的 `Scene`。路径不在 JavaScript 中解释为 PCB 对象；每个图层在加载时记录为一个 SkPicture，并复用同画笔和字号的 Skia 对象。平移与缩放更新 Reanimated 共享矩阵，图层切换复用已记录的图片。卸载时释放 SkPicture 和桌面事件监听。

填充使用默认 Winding；孔洞轮廓由 Rust 统一为反向 winding。独立钻孔层覆盖色与画布背景均为 `#0b121a`。拾取仅在单指未移动的释放时跨桥，并传递可见图层 ID。

默认字体是 SIL OFL 1.1 的 **Noto Sans SC**，覆盖 Latin 和简体中文。`src/assets/OFL.txt` 随字体分发；字体为 Google Fonts 上游变量字体的 400 字重静态实例，保留全部字形，大小 10,595,892 字节。上游来源：

- [字体](https://github.com/google/fonts/blob/main/ofl/notosanssc/NotoSansSC%5Bwght%5D.ttf)
- [许可证](https://github.com/google/fonts/blob/main/ofl/notosanssc/OFL.txt)

生成命令：

```sh
fonttools varLib.instancer NotoSansSC.ttf wght=400 --output NotoSansSC-Regular.ttf
```

打包文件 SHA-256：`ff2a28427e399dd838c02c142fcfeff6aae202609d60a970fb258994a533ab6a`。字体通过 Skia `useTypeface` 直接加载；原生入口静态 `require` asset，Web 入口使用 Vite `?url`，因此 Web 解析扩展必须优先 `.web.ts`。`fontSource` 接受 Skia 的 `DataSource`，可提供工程字体。缺字会在画布下方明确提示；应用不隐瞒字形缺失，也不声称复制原工程字体和排版。

交互：单指/鼠标拖动平移，双指围绕中心缩放，滚轮围绕鼠标缩放，`+`/`-` 缩放，方向键平移，`0` 适应窗口。工具栏触控目标至少 44px，图层开关使用可访问 checkbox。

离线测试包括真实 CanvasKit 软件表面像素验证（填充、孔洞、字体、损坏路径），以及图层分组和触控数学。[Playwright 回归](../../tests/e2e/README.md)使用真实 CanvasKit WASM、中文字体和浏览器触摸事件，覆盖六组尺寸的缩放、拖动、双指缩放、图层、拾取与返回。此自动化未覆盖原生 GPU、密集真实工程和真机手势。

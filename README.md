# Snapshoot

Snapshoot 是一款跨平台截图工具，支持 Windows 和 macOS。它可以在截图后直接标注、钉图、长截图、OCR 识别和翻译，一次操作走完，不用来回切换多个软件。

## 功能

- **截图**：框选任意区域截图，支持多显示器。
- **标注**：截图后直接在图上画框、箭头、线条、马赛克、打字，还能裁剪和撤销。
- **钉图**：把截图钉在屏幕最上层，边看图边写东西，支持缩放和透明度调节。
- **长截图**：自动滚动拼接，适合截网页和长文档。
- **OCR 识别**：识别图片里的文字，支持本地和在线识别服务。
- **翻译**：划词翻译、输入翻译、截图翻译、OCR 翻译，支持多家翻译服务，可在设置里自行添加和切换。
- **历史记录**：翻译和识别记录可回看、搜索、导出。
- **备份**：配置可备份到本地、WebDAV 或阿里云盘。
- **外部调用**：本机通过 `http://127.0.0.1:60828` 调用翻译、OCR 等功能，端口可在设置里改。

## 快捷键

快捷键都可以在设置 → 快捷键里自己改，下面是默认值：

| 功能 | macOS | Windows |
| --- | --- | --- |
| 截图标注 | Command + Shift + A | F1 |
| 剪贴板贴图 | Command + Shift + P | F3 |
| 划词翻译 | 未设置 | 未设置 |
| 输入翻译 | 未设置 | 未设置 |
| OCR 识别 | 未设置 | 未设置 |
| OCR 翻译 | 未设置 | 未设置 |

划词、输入、OCR 这几项默认是空的，第一次用时去设置里按一个自己顺手的组合键即可。

## 下载安装

安装包见 GitHub Releases 页面：

https://github.com/shenziggy0/snapshoot/releases

v1.0.0 提供三个安装包：

| 系统 | 文件 |
| --- | --- |
| Windows 64 位 | `Snapshoot_1.0.0_x64-setup.exe` |
| macOS Apple silicon（M 系列芯片） | `Snapshoot_1.0.0_aarch64.dmg` |
| macOS Intel | `Snapshoot_1.0.0_x64.dmg` |

下载后按下面对应系统的方法安装。实际文件名以 Releases 页面的附件为准。

### Windows

直接运行 `Snapshoot_1.0.0_x64-setup.exe`，按安装向导完成即可。

### macOS

先确认自己的芯片：左上角苹果菜单 → 关于本机，看“芯片”一栏是 Apple M 系列还是 Intel，下载对应的 dmg。

1. 打开 dmg，把 Snapshoot 拖到“应用程序”文件夹。
2. 首次打开时，macOS 可能会提示“无法验证开发者”。这是因为安装包没有经过 Apple 签名和公证，不是文件损坏。
3. 按下面的方法放行一次，之后就可以正常双击打开：
   - 在“访达 → 应用程序”里按住 Control 键点 Snapshoot → 打开 → 在弹窗里点“打开”；
   - 如果已经直接双击被拦截，去“系统设置 → 隐私与安全性”，在页面下方点“仍要打开”；
   - 实在打不开时，才用终端执行 `xattr -cr /Applications/Snapshoot.app` 清除隔离标记，再重新打开。
4. 首次截图和用快捷键时，系统会要求授权：去“系统设置 → 隐私与安全性”，给 Snapshoot 打开“屏幕录制”和“辅助功能”权限，否则截不到图、快捷键也不生效。

## 隐私提示

翻译和 OCR 的服务密钥由你自己在设置里填写，存在本地，不会上传到 Snapshoot 的服务器。默认翻译服务是不需要密钥的火山免费服务。

下面这三项例外，启用前请先看清楚：

- Lingva 翻译：待翻译文本会发到 `lingva.pot-app.com`，那是上游的服务器，不是 Snapshoot 的。
- ECDICT 词典：待查文本会发到 `pot-app.com/api/dict`，同样是上游的服务器。
- 阿里云盘备份：登录换 token 时会经过 `pot-app.com/api/ali_access_token`，同样是上游的服务器。

这三项都不是默认开启的，只有你自己在设置里选了它们才会被用到。

## 源码编译

不想用安装包，也可以自己从源码编译。需要 Node.js 21、pnpm 9、Rust stable。

```bash
git clone https://github.com/shenziggy0/snapshoot.git
cd snapshoot
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` 是开发运行。打包用：

```bash
pnpm tauri build
```

打包产物在 `src-tauri/target/release/bundle/` 下面。只打 macOS dmg 或 Windows 安装包时，可以加参数，例如：

```bash
pnpm tauri build --target aarch64-apple-darwin --bundles dmg
pnpm tauri build --target x86_64-apple-darwin --bundles dmg
pnpm tauri build -b nsis --target x86_64-pc-windows-msvc
```

## 反馈

遇到问题或想提建议，请到 GitHub Issues：

https://github.com/shenziggy0/snapshoot/issues

致谢：Snapshoot 基于 Pot（pot-app/pot-desktop）开发，遵循 GPL-3.0 协议。

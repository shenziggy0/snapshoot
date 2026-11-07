<p align="center">
  <img src="public/icon.png" width="128" alt="Snapshoot">
</p>

<h1 align="center">Snapshoot</h1>

Snapshoot 是一款跨平台截图工具，支持 Windows 和 macOS。它可以在截图后直接标注、钉图、长截图、录屏、OCR 识别和翻译，一次操作走完，不用来回切换多个软件。

## 功能

-   **截图**：框选任意区域截图，支持多显示器。
-   **快速捕获**：可开启选区稳定后自动复制；完成复制、保存或贴图后显示 Quick Access 浮层，最近 60 张截图保存在本地历史中，可重新复制、保存、分享、OCR 或进入标注。
-   **标注**：截图后直接在图上画框、箭头、线条、马赛克、打字，还能裁剪和撤销。
-   **钉图**：把截图或剪贴板文字钉在屏幕最上层，边看图边写东西；图片贴图支持缩放和透明度调节。
-   **长截图**：自动滚动拼接，适合截网页和长文档。
-   **录屏**：框选区域录成 MP4，可选分辨率、帧率和清晰度，支持系统声音或麦克风、暂停和继续；录完自动保存并放进剪贴板，可以直接粘贴发送。macOS 需要 12.3 及以上，系统声音需要 13 及以上。
-   **OCR 识别**：识别图片里的文字，支持本地和在线识别服务；macOS 使用 Apple Vision 本地识别并返回文字位置与二维码。
-   **翻译**：划词翻译、输入翻译、截图翻译、OCR 翻译，以及直接在冻结截图上显示译文；支持多家翻译服务，可在设置里自行添加和切换。
-   **隐私打码**：截图内识别邮箱、手机号、密钥、令牌等内容并一键添加马赛克。
-   **分享**：配置 WebDAV 后，可从 Quick Access 上传截图并复制分享链接。
-   **历史记录**：翻译和识别记录可回看、搜索、导出。
-   **备份**：配置可备份到本地、WebDAV 或阿里云盘。
-   **外部调用**：本机通过 `http://127.0.0.1:60828` 调用翻译、OCR 等功能，端口可在设置里改。

## 快捷键

快捷键都可以在设置 → 快捷键里自己改，下面是默认值：

| 功能       | macOS               | Windows |
| ---------- | ------------------- | ------- |
| 截图标注   | Command + Shift + A | F1      |
| 剪贴板贴图 | Command + Shift + P | F3      |
| 区域录屏   | 未设置              | F2      |
| 划词翻译   | 未设置              | 未设置  |
| 输入翻译   | 未设置              | 未设置  |
| OCR 识别   | 未设置              | 未设置  |
| OCR 翻译   | 未设置              | 未设置  |

划词、输入、OCR 这几项默认是空的，macOS 上的录屏也是，第一次用时去设置里按一个自己顺手的组合键即可。

## 下载安装

安装包见 GitHub Releases 页面：

https://github.com/shenziggy0/snapshoot/releases

v1.0.0 提供三个安装包：

| 系统                              | 文件                            |
| --------------------------------- | ------------------------------- |
| Windows 64 位                     | `Snapshoot_1.0.0_x64-setup.exe` |
| macOS Apple silicon（M 系列芯片） | `Snapshoot_1.0.0_aarch64.dmg`   |
| macOS Intel                       | `Snapshoot_1.0.0_x64.dmg`       |

下载后按下面对应系统的方法安装。实际文件名以 Releases 页面的附件为准。

### Windows

直接运行 `Snapshoot_1.0.0_x64-setup.exe`，按安装向导完成即可。

### macOS

先确认自己的芯片：左上角苹果菜单 → 关于本机，看“芯片”一栏是 Apple M 系列还是 Intel，下载对应的 dmg。

1. 打开 dmg，把 Snapshoot 拖到“应用程序”文件夹。
2. 先阅读 DMG 中的“安装前请阅读 - Read Me.txt”，再从“应用程序”打开 Snapshoot。
3. 当前版本没有 Developer ID 签名和 Apple 公证，可能被 macOS 拦截。确认安装包来自可信来源后，可在“系统设置 → 隐私与安全性”中选择“仍要打开”（如有）。若提示“已损坏”且仍打不开，可执行：
    ```bash
    xattr -dr com.apple.quarantine /Applications/Snapshoot.app
    ```
    该命令只移除本应用的下载隔离标记，不会修复损坏文件；如果仍无法打开，请重新下载并反馈完整错误。
4. 首次使用需在“系统设置 → 隐私与安全性”中授权“屏幕录制”和“辅助功能”。更新到支持权限迁移的版本后，应用会在首次启动时尝试清除自己的这两项旧授权，并引导重新授权；同一构建不重复清除。授权完成后请完全退出并重开应用。若自动清除失败，请移除旧条目，再添加“应用程序”里的 Snapshoot。

完整说明见 [macOS 安装与更新说明](docs/install/macOS安装说明.txt)。

开发者打包：先运行 `pnpm tauri build --target aarch64-apple-darwin --bundles app`，再运行：

```bash
python3 scripts/package_macos.py --app src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Snapshoot.app --output-dir src-tauri/target/aarch64-apple-darwin/release/bundle/dmg --arch aarch64
```

Intel 构建将 target 改为 `x86_64-apple-darwin`、arch 改为 `x64`。此流程对应用做完整临时签名校验，并将说明放入 DMG；仍不属于 Apple 公证版本。权限迁移按版本与签名指纹识别构建，发布时仍应提升版本号；开发模式与 smoke 测试不会清除权限。

## 隐私提示

翻译和 OCR 的服务密钥由你自己在设置里填写，存在本地，不会上传到 Snapshoot 的服务器。默认翻译服务是不需要密钥的火山免费服务。

下面这三项例外，启用前请先看清楚：

-   Lingva 翻译：待翻译文本会发到 `lingva.pot-app.com`，那是上游的服务器，不是 Snapshoot 的。
-   ECDICT 词典：待查文本会发到 `pot-app.com/api/dict`，同样是上游的服务器。
-   阿里云盘备份：登录换 token 时会经过 `pot-app.com/api/ali_access_token`，同样是上游的服务器。

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

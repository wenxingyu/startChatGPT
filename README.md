# startChatGPT（Rust 版）

动态寻找最新版 `OpenAI.Codex`，使用以下代理参数启动其中的 `app\\chatgpt.exe`：

```text
--proxy-server=http://127.0.0.1:10808
```

启动时会显示原生 Windows Loading 动画：蓝灰到深靛色渐变背景、青绿到紫色顶部光带、放大的
ChatGPT 官方图标和青绿色旋转圆点。启动器持续枚举可见的顶层窗口，并核对窗口所属进程的完整
路径；检测到真正的 ChatGPT 主窗口后自动关闭 Loading。等待超过 60 秒或进程异常退出时，Loading
会关闭并显示错误对话框。

## 编译

```powershell
cd C:\Code\ontology\startChatGPT-rust
.\build.ps1
```

Release 配置针对体积优化：`opt-level = "z"`、完整 LTO、单 codegen unit、`panic = "abort"`
并移除符号。官方 ChatGPT 图标通过项目内的 `chatgpt_icon_windows_amd64.syso` 资源对象嵌入。

# 代码结构

项目使用单个 Cargo package 和一个二进制 crate。当前只有一个启动器，Windows
和 macOS 共用配置、参数解析及额度服务；用 Rust 模块划分职责即可，无需为每个平台
建立单独的 crate。后续如果需要供其他程序复用的库或独立发布的组件，再考虑
`lib.rs` 或 Cargo workspace。

使用 `模块名.rs` 声明模块，在同名目录下放置子模块。例如 `usage.rs` 声明
`mod bridge;`，Rust 自动读取 `usage/bridge.rs`，不需要自定义 `#[path]`。

```text
src/
├── main.rs                       # 二进制入口，调用平台启动入口
├── config.rs                     # 代理设置、校验与本地配置读写
├── launch_options.rs             # 共享命令行解析及参数转发
├── usage.rs                      # 额度模块声明及对平台层的接口导出
├── usage/
│   ├── model.rs                  # 额度窗口、缓存状态、响应解析
│   ├── discovery.rs              # Codex CLI 候选路径与 macOS CLI PATH
│   ├── bridge.rs                 # app-server 子进程与 JSON 请求通信
│   ├── service.rs                # 后台刷新、手动刷新、停止与故障重连
│   └── service/
│       └── recovery_tests.rs      # 使用真实子进程与管道的故障恢复测试
├── platform.rs                   # 条件编译选择平台与共享系统接口
└── platform/
    ├── windows.rs                # Windows 入口与额度更新消息适配
    ├── windows/
    │   ├── launcher.rs           # 启动流程、应用查找及错误提示
    │   ├── packaged.rs           # 已注册应用包查找、COM 激活与参数编码
    │   ├── process.rs            # 主窗口检测及进程退出等待
    │   ├── memory.rs             # Windows 空闲工作集释放
    │   ├── settings.rs           # 原生代理设置窗口
    │   ├── splash.rs             # 启动动画及绘制
    │   ├── usage_tray.rs         # 系统托盘图标、菜单及详情窗口
    │   └── usage_widget.rs       # 额度小窗及详情文本
    ├── macos.rs                  # macOS 入口
    └── macos/
        ├── launcher.rs           # 设置选择、桌面启动及监控模式
        ├── discovery.rs          # ChatGPT.app 查找与 bundle 校验
        ├── ui.rs                 # Swift 组件定位、单次请求及设置响应解析
        └── monitor.rs            # 额度状态与 AppKit 监控事件转发
native/macos/main.swift           # 原生 AppKit 界面组件
```

## 模块边界

- `main.rs` 只声明顶层模块并调用 `platform::run()`；平台选择集中在 `platform.rs`。
- `config` 和 `launch_options` 提供两个平台共享的设置与参数逻辑。
- 平台启动与界面通过 `usage` 导出的 `State`、`Action` 和 `start_with_notify`
  使用额度服务，通信、候选路径和解析实现保持在私有子模块内。
- `usage::service` 负责更新状态与重连；`usage::bridge` 负责进程生命周期与请求。
  `usage::model` 不依赖平台界面或子进程。
- Windows 窗口消息回调在 `platform/windows.rs` 转换为服务的通用通知回调，额度
  服务不引用窗口句柄。子进程工作集释放通过 `platform::trim_child_process` 调用；
  macOS 上该接口为空操作。
- Windows 的主窗口检测、进程退出等待放在 `process`，启动动画和托盘都可以使用，
  托盘无需依赖动画模块。
- `pub(crate)` 表示整个 crate 需要的共享接口，`pub(super)` 表示同平台或同功能
  子模块之间的接口，其余实现保持私有。跨功能引用使用 `crate::`，同组引用使用
  `super::`。

平台启动和 UI 模块按目标系统条件编译。Windows 测试构建也编译 macOS Rust
模块以覆盖纯逻辑测试，保留重组前的测试范围；不会在测试中启动 AppKit 界面。
CLI 查找和进程启动仍保留必要的系统条件分支，因为它们是同一项共享服务的
平台适配细节。

## 测试与构建

在支持的 Windows 或 macOS 主机上运行：

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked
cargo build --release --locked
```

纯逻辑单元测试与实现放在同一文件的 `#[cfg(test)] mod tests` 中。
额度恢复测试属于 `usage::service::recovery_tests`，无需访问账号或网络；
其中 `mock_server` 是供测试启动的子进程入口，普通测试运行时保持 ignored。
请求超时和自动重连测试分别使用真实的 15 秒超时与 45 秒刷新间隔。
Windows 的桌面激活测试仍需安装 ChatGPT，保持 ignored，仅在明确需要时单独运行。

模块重组不改变二进制名称、Cargo 依赖、配置文件格式、命令行接口或 Swift JSON
协议。`build.ps1`、`build-macos.sh` 和现有 Windows/macOS CI 继续使用原有构建入口。

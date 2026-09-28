// AppKit owns the macOS UI; Rust owns settings persistence and the quota service.
// stdin/stdout contain only newline-delimited JSON, never account credentials.
import AppKit
import Foundation
import Darwin

func send(_ value: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: value) else { return }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
}

func finish(_ value: [String: Any], code: Int32 = 0) -> Never {
    send(value)
    exit(code)
}

func readRequest() -> [String: Any]? {
    guard let line = readLine(), let data = line.data(using: .utf8),
          let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
    return value
}

func validProxy(_ value: String) -> Bool {
    !value.isEmpty && value.rangeOfCharacter(from: .whitespacesAndNewlines) == nil &&
        ["http://", "https://", "socks4://", "socks5://"].contains { value.hasPrefix($0) }
}

final class SettingsControls: NSObject {
    let direct = NSButton(checkboxWithTitle: "不使用代理（直接连接）", target: nil, action: nil)
    let field = NSTextField(frame: NSRect(x: 0, y: 12, width: 360, height: 24))

    override init() {
        super.init()
        direct.target = self
        direct.action = #selector(toggle)
        direct.frame = NSRect(x: 0, y: 48, width: 360, height: 24)
    }

    @objc func toggle() { field.isEnabled = direct.state != .on }
}

func settings(_ request: [String: Any]) -> Never {
    let controls = SettingsControls()
    controls.field.stringValue = request["proxy"] as? String ?? "http://127.0.0.1:10808"
    controls.direct.state = request["proxy"] is String ? .off : .on
    controls.toggle()
    let accessory = NSView(frame: NSRect(x: 0, y: 0, width: 360, height: 84))
    accessory.addSubview(controls.direct)
    accessory.addSubview(controls.field)
    let alert = NSAlert()
    alert.messageText = "startChatGPT 代理设置"
    alert.informativeText = "支持 HTTP、HTTPS、SOCKS4 和 SOCKS5。保存后启动应用。"
    alert.addButton(withTitle: "保存并启动")
    alert.addButton(withTitle: "取消")
    alert.accessoryView = accessory
    NSApp.activate(ignoringOtherApps: true)
    while true {
        guard alert.runModal() == .alertFirstButtonReturn else { finish(["cancelled": true]) }
        if controls.direct.state == .on { finish(["proxy": NSNull()]) }
        let value = controls.field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        if validProxy(value) { finish(["proxy": value]) }
        let invalid = NSAlert()
        invalid.messageText = "代理地址无效"
        invalid.informativeText = "请输入以 http://、https://、socks4:// 或 socks5:// 开头且不含空格的地址。"
        invalid.runModal()
    }
}

func applications(at bundle: URL) -> [NSRunningApplication] {
    NSWorkspace.shared.runningApplications.filter {
        $0.bundleURL?.resolvingSymlinksInPath() == bundle.resolvingSymlinksInPath() && !$0.isTerminated
    }
}

func proxyEnvironment(_ inherited: [String: String], proxy: String?) -> [String: String] {
    var environment = inherited
    // Empty values override ambient launchd variables as well as shell ones.
    for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] {
        environment[key] = proxy ?? ""
    }
    environment["NO_PROXY"] = ""
    environment["no_proxy"] = ""
    return environment
}

func launch(_ request: [String: Any]) {
    guard let path = request["bundle"] as? String,
          let arguments = request["arguments"] as? [String] else {
        finish(["error": "应用启动请求无效"], code: 1)
    }
    let bundle = URL(fileURLWithPath: path, isDirectory: true)
    // LaunchServices cannot change the arguments/environment of an existing app.
    if !applications(at: bundle).isEmpty {
        finish(["error": "ChatGPT 已在运行，无法应用新的代理设置。请先完全退出 ChatGPT，再通过 startChatGPT 启动；只看额度可使用 --usage-only。"], code: 1)
    }
    let configuration = NSWorkspace.OpenConfiguration()
    configuration.arguments = arguments
    configuration.environment = proxyEnvironment(ProcessInfo.processInfo.environment, proxy: request["proxy"] as? String)
    configuration.activates = true
    NSWorkspace.shared.openApplication(at: bundle, configuration: configuration) { app, error in
        DispatchQueue.main.async {
            if error != nil || app == nil {
                finish(["error": "macOS 无法启动指定应用，请检查安装是否完整及系统是否允许打开该应用。"], code: 1)
            }
            finish(["launched": true])
        }
    }
    DispatchQueue.main.asyncAfter(deadline: .now() + 60) {
        finish(["error": "等待 macOS 应用启动超时（60 秒）"], code: 1)
    }
    NSApp.run()
}

struct QuotaRow {
    let label: String
    let description: String
    let remaining: Double?
    let reset: Double?

    var percent: String { remaining.map { String(format: "%.0f%%", $0) } ?? "--" }
    var title: String { "\(description)：\(percent)" }
}

func quotaRows(_ request: [String: Any]) -> [QuotaRow] {
    let windows = request["windows"] as? [Any] ?? []
    return (0..<2).map { index in
        let value = index < windows.count ? windows[index] as? [String: Any] : nil
        return QuotaRow(label: value?["label"] as? String ?? (index == 0 ? "5H" : "1W"),
                        description: value?["description"] as? String ?? (index == 0 ? "短期额度" : "每周额度"),
                        remaining: value?["remaining"] as? Double,
                        reset: value?["resetsAt"] as? Double)
    }
}

func quotaColor(_ remaining: Double?, stale: Bool) -> NSColor {
    guard !stale, let remaining = remaining else { return .secondaryLabelColor }
    if remaining <= 10 { return .systemRed }
    if remaining <= 25 { return .systemOrange }
    return .systemGreen
}

func acquireMonitorLock() -> Int32 {
    let directory = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Caches/startChatGPT", isDirectory: true)
    do {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    } catch {
        finish(["error": "无法创建额度显示目录"], code: 1)
    }
    let descriptor = open(directory.appendingPathComponent("monitor.lock").path, O_CREAT | O_RDWR, mode_t(S_IRUSR | S_IWUSR))
    guard descriptor >= 0 else { finish(["error": "无法打开额度显示锁"], code: 1) }
    if flock(descriptor, LOCK_EX | LOCK_NB) != 0 {
        let duplicate = errno == EWOULDBLOCK
        close(descriptor)
        if duplicate { finish(["alreadyRunning": true]) }
        finish(["error": "无法锁定额度显示实例"], code: 1)
    }
    return descriptor // Keep this descriptor open until the helper exits.
}

final class Monitor: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var statusItem: NSStatusItem?
    var panel: NSPanel?
    var rows = quotaRows([:])
    var stale = true
    var error: String?
    var watchedBundle: URL?
    var observer: NSObjectProtocol?
    let formatter = DateFormatter()

    init(_ request: [String: Any]) {
        super.init()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        if request["watchApp"] as? Bool == true, let path = request["bundle"] as? String {
            watchedBundle = URL(fileURLWithPath: path, isDirectory: true)
        }
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        render()
        if request["widget"] as? Bool == true { showDetails() }
        observer = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didTerminateApplicationNotification, object: nil, queue: .main
        ) { [weak self] _ in
            guard let self = self, let bundle = self.watchedBundle else { return }
            if applications(at: bundle).isEmpty { NSApp.terminate(nil) }
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        if let bundle = watchedBundle, applications(at: bundle).isEmpty { NSApp.terminate(nil) }
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let observer = observer { NSWorkspace.shared.notificationCenter.removeObserver(observer) }
        if let item = statusItem { NSStatusBar.system.removeStatusItem(item) }
    }

    func update(_ request: [String: Any]) {
        rows = quotaRows(request)
        stale = request["stale"] as? Bool ?? true
        error = request["error"] as? String
        render()
    }

    func detailsText() -> String {
        var lines: [String] = []
        for row in rows {
            lines.append(row.title)
            if let reset = row.reset {
                lines.append("重置：\(formatter.string(from: Date(timeIntervalSince1970: reset)))")
            }
        }
        if stale { lines.append("数据已过期，保留上次读取的额度") }
        if let error = error { lines.append(error) }
        lines.append("额度来自当前登录的 Codex CLI 账户")
        return lines.joined(separator: "\n")
    }

    func render() {
        let title = rows[0].percent
        statusItem?.button?.attributedTitle = NSAttributedString(string: title, attributes: [
            .foregroundColor: quotaColor(rows[0].remaining, stale: stale),
            .font: NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium)
        ])
        statusItem?.button?.toolTip = detailsText()
        let menu = NSMenu()
        for row in rows {
            let item = NSMenuItem(title: row.title, action: nil, keyEquivalent: "")
            menu.addItem(item)
            if let reset = row.reset {
                menu.addItem(NSMenuItem(title: "重置：\(formatter.string(from: Date(timeIntervalSince1970: reset)))", action: nil, keyEquivalent: ""))
            }
        }
        if stale { menu.addItem(NSMenuItem(title: "数据已过期", action: nil, keyEquivalent: "")) }
        if let error = error { menu.addItem(NSMenuItem(title: error, action: nil, keyEquivalent: "")) }
        menu.addItem(.separator())
        addAction("额度详情", #selector(showDetails), to: menu)
        addAction("立即刷新", #selector(refresh), to: menu)
        menu.addItem(.separator())
        addAction("退出额度显示", #selector(quit), to: menu)
        statusItem?.menu = menu
        updatePanel()
    }

    func addAction(_ title: String, _ action: Selector, to menu: NSMenu) {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        menu.addItem(item)
    }

    func updatePanel() {
        guard let content = panel?.contentView else { return }
        (content.viewWithTag(1) as? NSTextField)?.stringValue = rows.map { "\($0.label)  \($0.percent)" }.joined(separator: "     ")
        (content.viewWithTag(1) as? NSTextField)?.textColor = quotaColor(rows[0].remaining, stale: stale)
        (content.viewWithTag(2) as? NSTextField)?.stringValue = detailsText()
    }

    @objc func showDetails() {
        if panel == nil {
            let window = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 440, height: 340),
                                 styleMask: [.titled, .closable, .utilityWindow], backing: .buffered, defer: false)
            window.title = "Codex 额度 · 剩余"
            window.isReleasedWhenClosed = false
            window.hidesOnDeactivate = false
            window.level = .floating
            window.delegate = self
            let numbers = NSTextField(labelWithString: "")
            numbers.frame = NSRect(x: 20, y: 284, width: 400, height: 36)
            numbers.font = NSFont.monospacedDigitSystemFont(ofSize: 24, weight: .medium)
            numbers.tag = 1
            let details = NSTextField(wrappingLabelWithString: "")
            details.frame = NSRect(x: 20, y: 52, width: 400, height: 220)
            details.font = NSFont.systemFont(ofSize: 12)
            details.tag = 2
            let refreshButton = NSButton(title: "立即刷新", target: self, action: #selector(refresh))
            refreshButton.frame = NSRect(x: 20, y: 12, width: 100, height: 28)
            window.contentView?.addSubview(numbers)
            window.contentView?.addSubview(details)
            window.contentView?.addSubview(refreshButton)
            window.center()
            panel = window
        }
        updatePanel()
        panel?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc func refresh() { send(["action": "refresh"]) }
    @objc func quit() { NSApp.terminate(nil) }
}

let mode = CommandLine.arguments.dropFirst().first ?? ""
if mode == "self-test" {
    precondition(validProxy("socks5://127.0.0.1:10808"))
    precondition(!validProxy("http://bad address"))
    precondition(!validProxy("127.0.0.1:7890"))
    let direct = proxyEnvironment(["HTTP_PROXY": "http://old", "no_proxy": "*", "OTHER": "preserved"], proxy: nil)
    precondition(direct["HTTP_PROXY"] == "" && direct["no_proxy"] == "" && direct["OTHER"] == "preserved")
    let proxied = proxyEnvironment(["NO_PROXY": "*"], proxy: "http://127.0.0.1:7890")
    precondition(proxied["HTTPS_PROXY"] == "http://127.0.0.1:7890" && proxied["https_proxy"] == proxied["HTTPS_PROXY"])
    precondition(proxied["NO_PROXY"] == "")
    let rows = quotaRows(["windows": [["label": "5H", "description": "5 小时额度", "remaining": 63.0, "resetsAt": 123.0], NSNull()]])
    precondition(rows[0].percent == "63%" && rows[0].reset == 123)
    precondition(rows[1].percent == "--")
    precondition(quotaColor(10, stale: false) == .systemRed)
    precondition(quotaColor(25, stale: false) == .systemOrange)
    precondition(quotaColor(63, stale: true) == .secondaryLabelColor)
    finish(["ok": true])
}

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
guard let request = readRequest() else { finish(["error": "界面请求无效"], code: 1) }
switch mode {
case "settings": settings(request)
case "launch": launch(request)
case "error":
    let alert = NSAlert()
    alert.alertStyle = .critical
    alert.messageText = "startChatGPT 启动失败"
    alert.informativeText = request["message"] as? String ?? "未知错误"
    application.activate(ignoringOtherApps: true)
    alert.runModal()
    finish(["ok": true])
case "monitor":
    let lockDescriptor = acquireMonitorLock()
    let monitor = Monitor(request)
    application.delegate = monitor
    send(["ready": true])
    DispatchQueue.global(qos: .utility).async {
        while let update = readRequest() {
            DispatchQueue.main.async { monitor.update(update) }
        }
        // The Rust parent has exited or closed its pipe.
        DispatchQueue.main.async { NSApp.terminate(nil) }
    }
    withExtendedLifetime(monitor) { application.run() }
    close(lockDescriptor)
default: finish(["error": "未知界面模式"], code: 1)
}

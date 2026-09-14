import Cocoa
import FlutterMacOS

/// Security-scoped bookmarks + directory picker for agent `repo` workspaces (S-KD 14).
final class WorkspaceBookmarkPlugin: NSObject, FlutterPlugin {
  private var accessed: [String: URL] = [:]

  static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "kim.workspace",
      binaryMessenger: registrar.messenger)
    let instance = WorkspaceBookmarkPlugin()
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "pickDirectory":
      pickDirectory(result: result)
    case "startAccessing":
      guard let args = call.arguments as? [String: Any],
            let b64 = args["bookmarkBase64"] as? String
      else {
        result(
          FlutterError(code: "bad_args", message: "bookmarkBase64 required", details: nil))
        return
      }
      startAccessing(bookmarkBase64: b64, result: result)
    case "stopAccessing":
      guard let args = call.arguments as? [String: Any],
            let b64 = args["bookmarkBase64"] as? String
      else {
        result(nil)
        return
      }
      stopAccessing(bookmarkBase64: b64)
      result(nil)
    case "realHomeAgentsSkills":
      result(realHomeAgentsSkills())
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  private func pickDirectory(result: @escaping FlutterResult) {
    let panel = NSOpenPanel()
    panel.canChooseFiles = false
    panel.canChooseDirectories = true
    panel.allowsMultipleSelection = false
    panel.canCreateDirectories = false
    panel.prompt = "Choose"
    panel.message = "Select a local repository for this agent"
    let response = panel.runModal()
    guard response == .OK, let url = panel.url else {
      result(nil)
      return
    }
    do {
      let data = try url.bookmarkData(
        options: [.withSecurityScope],
        includingResourceValuesForKeys: nil,
        relativeTo: nil)
      let b64 = data.base64EncodedString()
      result(["path": url.path, "bookmarkBase64": b64])
    } catch {
      result(
        FlutterError(
          code: "bookmark_failed", message: error.localizedDescription, details: nil))
    }
  }

  private func startAccessing(bookmarkBase64: String, result: @escaping FlutterResult) {
    guard let data = Data(base64Encoded: bookmarkBase64) else {
      result(
        FlutterError(code: "bad_bookmark", message: "invalid base64", details: nil))
      return
    }
    var isStale = false
    do {
      let url = try URL(
        resolvingBookmarkData: data,
        options: [.withSecurityScope, .withoutUI],
        relativeTo: nil,
        bookmarkDataIsStale: &isStale)
      if isStale {
        result(
          FlutterError(code: "stale", message: "bookmark is stale; reselect", details: nil))
        return
      }
      if !url.startAccessingSecurityScopedResource() {
        result(
          FlutterError(
            code: "access_denied", message: "startAccessing failed", details: nil))
        return
      }
      accessed[bookmarkBase64] = url
      result(["path": url.path, "ok": true])
    } catch {
      result(
        FlutterError(
          code: "resolve_failed", message: error.localizedDescription, details: nil))
    }
  }

  private func stopAccessing(bookmarkBase64: String) {
    if let url = accessed.removeValue(forKey: bookmarkBase64) {
      url.stopAccessingSecurityScopedResource()
    }
  }

  /// Real user home `~/.agents/skills`, not the App Sandbox container home (S-KD 23).
  private func realHomeAgentsSkills() -> String? {
    let pw = getpwuid(getuid())
    guard let pw, let dir = pw.pointee.pw_dir else {
      return nil
    }
    let home = String(cString: dir)
    return (home as NSString).appendingPathComponent(".agents/skills")
  }
}

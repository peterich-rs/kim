import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)
    WorkspaceBookmarkPlugin.register(with: flutterViewController.registrar(forPlugin: "WorkspaceBookmarkPlugin"))

    super.awakeFromNib()
    minSize = NSSize(width: 420, height: 560)
    title = (Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String)
      ?? (Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String)
      ?? "KIM"
  }
}

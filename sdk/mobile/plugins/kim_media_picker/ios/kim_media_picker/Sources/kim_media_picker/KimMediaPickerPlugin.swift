import AVFoundation
import Flutter
import Photos
import UIKit

public class KimMediaPickerPlugin: NSObject, FlutterPlugin {
  private var pending: FlutterResult?

  public static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "kim.media_picker",
      binaryMessenger: registrar.messenger()
    )
    let instance = KimMediaPickerPlugin()
    registrar.addMethodCallDelegate(instance, channel: channel)
  }

  public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "pickSingle":
      presentAlbum(maxCount: 1, result: result)
    case "pickMultiple", "pickAlbum":
      let args = call.arguments as? [String: Any]
      let maxCount = max((args?["maxCount"] as? Int) ?? 9, 1)
      presentAlbum(maxCount: maxCount, result: result)
    case "capture", "takePhoto":
      let args = call.arguments as? [String: Any]
      let mode = (args?["mode"] as? String) ?? (call.method == "takePhoto" ? "photo" : "mixed")
      presentCamera(mode: mode, result: result)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  private func presentAlbum(maxCount: Int, result: @escaping FlutterResult) {
    guard pending == nil else {
      result(FlutterError(code: "already_active", message: "picker already open", details: nil))
      return
    }
    guard let host = Self.topController() else {
      result(FlutterError(code: "unavailable", message: "no view controller", details: nil))
      return
    }
    pending = result
    PickerStore.shared.reset(maxCount: maxCount)
    let album = AlbumViewController { [weak self] assets in
      self?.finish(assets)
    }
    let nav = UINavigationController(rootViewController: album)
    nav.modalPresentationStyle = .fullScreen
    nav.navigationBar.prefersLargeTitles = false
    presentWhenIdle(nav, from: host)
  }

  private func presentCamera(mode: String, result: @escaping FlutterResult) {
    guard pending == nil else {
      result(FlutterError(code: "already_active", message: "picker already open", details: nil))
      return
    }
    guard let host = Self.topController() else {
      result(FlutterError(code: "unavailable", message: "no view controller", details: nil))
      return
    }
    pending = result
    PickerStore.shared.reset(maxCount: 1)
    let camera = CameraViewController(mode: mode) { [weak self] assets in
      self?.finish(assets)
    }
    camera.modalPresentationStyle = .fullScreen
    presentWhenIdle(camera, from: host)
  }

  private func finish(_ maps: [[String: Any]]) {
    let result = pending
    pending = nil
    result?(maps)
  }

  static func topController() -> UIViewController? {
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
    let window =
      scenes.flatMap { $0.windows }.first { $0.isKeyWindow }
      ?? scenes.flatMap { $0.windows }.first { $0.windowLevel == .normal && !$0.isHidden }
      ?? scenes.first?.windows.first
    var top = window?.rootViewController
    while let presented = top?.presentedViewController {
      top = presented
    }
    return top
  }

  private func presentWhenIdle(_ vc: UIViewController, from host: UIViewController, tries: Int = 8) {
    if host.presentedViewController != nil || host.isBeingDismissed {
      if tries <= 0 {
        finish([])
        return
      }
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self] in
        guard let self, let next = Self.topController() else {
          self?.finish([])
          return
        }
        self.presentWhenIdle(vc, from: next, tries: tries - 1)
      }
      return
    }
    host.present(vc, animated: true)
  }
}

enum KimPickerStyle {
  static let accent = UIColor(red: 7 / 255, green: 193 / 255, blue: 96 / 255, alpha: 1)
  static let text = UIColor.label
  static let muted = UIColor.secondaryLabel
  static let bar = UIColor.secondarySystemBackground
  static let bg = UIColor.systemBackground
}

final class PickerStore {
  static let shared = PickerStore()
  var maxCount = 9
  var assets: [PHAsset] = []
  var selected: [String] = []

  func reset(maxCount: Int) {
    self.maxCount = max(maxCount, 1)
    assets = []
    selected = []
  }

  func index(of id: String) -> Int {
    guard let i = selected.firstIndex(of: id) else { return 0 }
    return i + 1
  }

  func toggle(_ asset: PHAsset, from view: UIView) -> Bool {
    if let i = selected.firstIndex(of: asset.localIdentifier) {
      selected.remove(at: i)
      return true
    }
    if selected.count >= maxCount {
      if maxCount == 1 {
        selected = [asset.localIdentifier]
        return true
      }
      let toast = UILabel()
      toast.text = "最多只能选择\(maxCount)张照片"
      toast.textColor = .white
      toast.backgroundColor = UIColor.black.withAlphaComponent(0.78)
      toast.font = .systemFont(ofSize: 14)
      toast.textAlignment = .center
      toast.layer.cornerRadius = 8
      toast.clipsToBounds = true
      toast.frame = CGRect(x: 40, y: view.bounds.midY - 20, width: view.bounds.width - 80, height: 40)
      view.addSubview(toast)
      UIView.animate(withDuration: 0.3, delay: 1.2, options: .curveEaseIn) {
        toast.alpha = 0
      } completion: { _ in
        toast.removeFromSuperview()
      }
      return false
    }
    selected.append(asset.localIdentifier)
    return true
  }

  func selectedAssets() -> [PHAsset] {
    selected.compactMap { id in assets.first { $0.localIdentifier == id } }
  }
}

import Flutter
import Security
import UIKit

/// `kim.keystore` MethodChannel: Keychain generic-password CRUD.
/// Rust owns key names and lifecycle; this side only executes.
/// `kSecAttrAccessibleAfterFirstUnlockThisDevice` matches the previous
/// flutter_secure_storage options (first_unlock_this_device, no biometry).
enum KimKeystore {
  static let channel = "kim.keystore"
  private static let service = Bundle.main.bundleIdentifier ?? "com.kim.kim_mobile"

  static func register(with registrar: FlutterPluginRegistrar) {
    let instance = KimKeystorePlugin()
    registrar.addMethodCallDelegate(
      instance,
      channel: FlutterMethodChannel(name: channel, binaryMessenger: registrar.messenger))
  }

  static func read(_ key: String) -> String? {
    var query = baseQuery(key: key)
    query[kSecReturnData as String] = kCFBooleanTrue
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var item: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &item)
    guard status == errSecSuccess, let data = item as? Data else {
      return nil
    }
    return String(data: data, encoding: .utf8)
  }

  static func write(_ key: String, _ value: String) throws {
    let data = Data(value.utf8)
    var update = baseQuery(key: key)
    update[kSecReturnData as String] = kCFBooleanTrue
    update[kSecMatchLimit as String] = kSecMatchLimitOne
    var item: CFTypeRef?
    let copyStatus = SecItemCopyMatching(update as CFDictionary, &item)
    if copyStatus == errSecSuccess {
      let attributes: [String: Any] = [kSecValueData as String: data]
      let status = SecItemUpdate(update as CFDictionary, attributes as CFDictionary)
      guard status == errSecSuccess else {
        throw KeystoreError.osStatus(status)
      }
      return
    }
    if copyStatus != errSecItemNotFound {
      throw KeystoreError.osStatus(copyStatus)
    }
    var add = baseQuery(key: key)
    add[kSecValueData as String] = data
    add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDevice
    let status = SecItemAdd(add as CFDictionary, nil)
    guard status == errSecSuccess else {
      throw KeystoreError.osStatus(status)
    }
  }

  static func delete(_ key: String) {
    let query = baseQuery(key: key)
    SecItemDelete(query as CFDictionary)
  }

  private static func baseQuery(key: String) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: service,
      kSecAttrAccount as String: key,
    ]
  }

  enum KeystoreError: Error {
    case osStatus(OSStatus)
  }
}

/// Method-channel shell shared by iOS and macOS entry points.
final class KimKeystorePlugin: NSObject, FlutterPlugin {
  func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let args = call.arguments as? [String: Any],
          let key = args["key"] as? String
    else {
      result(FlutterError(code: "bad_args", message: "key required", details: nil))
      return
    }
    switch call.method {
    case "read":
      result(KimKeystore.read(key))
    case "write":
      guard let value = args["value"] as? String else {
        result(FlutterError(code: "bad_args", message: "value required", details: nil))
        return
      }
      do {
        try KimKeystore.write(key, value)
        result(nil)
      } catch {
        result(FlutterError(code: "keychain", message: "\(error)", details: nil))
      }
    case "delete":
      KimKeystore.delete(key)
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }
}

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    let registrar = engineBridge.pluginRegistry.registrar(forPlugin: "KimKeystorePlugin")
    KimKeystore.register(with: registrar)
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
  }
}

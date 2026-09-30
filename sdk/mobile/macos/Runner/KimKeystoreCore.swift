import Cocoa
import FlutterMacOS
import Security

/// `kim.keystore` MethodChannel for macOS. Data-Protection keychain
/// (iOS-style): no login-keychain password prompt, no biometry, no
/// passcode; available after first unlock. `kSecUseAuthenticationUIFail`
/// semantics — never present an auth UI (ad-hoc "Sign to Run Locally"
/// signatures fail with -34018 instead of prompting).
enum KimKeystoreCore {
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
    var query = baseQuery(key: key)
    query[kSecReturnData as String] = kCFBooleanTrue
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var item: CFTypeRef?
    let copyStatus = SecItemCopyMatching(query as CFDictionary, &item)
    if copyStatus == errSecSuccess {
      let attributes: [String: Any] = [kSecValueData as String: data]
      let status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
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
    add[kSecUseDataProtectionKeychain as String] = kCFBooleanTrue
    let status = SecItemAdd(add as CFDictionary, nil)
    guard status == errSecSuccess else {
      throw KeystoreError.osStatus(status)
    }
  }

  static func delete(_ key: String) {
    var query = baseQuery(key: key)
    query[kSecUseDataProtectionKeychain as String] = kCFBooleanTrue
    SecItemDelete(query as CFDictionary)
  }

  private static func baseQuery(key: String) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: service,
      kSecAttrAccount as String: key,
      kSecUseDataProtectionKeychain as String: kCFBooleanTrue,
    ]
  }

  enum KeystoreError: Error {
    case osStatus(OSStatus)
  }
}

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
      result(KimKeystoreCore.read(key))
    case "write":
      guard let value = args["value"] as? String else {
        result(FlutterError(code: "bad_args", message: "value required", details: nil))
        return
      }
      do {
        try KimKeystoreCore.write(key, value)
        result(nil)
      } catch {
        result(FlutterError(code: "keychain", message: "\(error)", details: nil))
      }
    case "delete":
      KimKeystoreCore.delete(key)
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }
}

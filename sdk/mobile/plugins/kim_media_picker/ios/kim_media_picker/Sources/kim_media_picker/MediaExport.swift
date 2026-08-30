import AVFoundation
import CoreMedia
import Photos
import UIKit

enum MediaExport {
  static let maxEdge: CGFloat = 2560

  static func export(_ assets: [PHAsset], completion: @escaping ([[String: Any]]) -> Void) {
    guard !assets.isEmpty else {
      completion([])
      return
    }
    let group = DispatchGroup()
    var slots: [[String: Any]?] = Array(repeating: nil, count: assets.count)
    for (i, asset) in assets.enumerated() {
      group.enter()
      export(asset) { map in
        slots[i] = map
        group.leave()
      }
    }
    group.notify(queue: .main) {
      completion(slots.compactMap { $0 })
    }
  }

  static func export(_ asset: PHAsset, completion: @escaping ([String: Any]?) -> Void) {
    let opts = PHImageRequestOptions()
    opts.isNetworkAccessAllowed = true
    opts.deliveryMode = .highQualityFormat
    opts.resizeMode = .none
    opts.isSynchronous = false
    PHImageManager.default().requestImageDataAndOrientation(for: asset, options: opts) { data, uti, _, info in
      if (info?[PHImageCancelledKey] as? Bool) == true {
        DispatchQueue.main.async { completion(nil) }
        return
      }
      if (info?[PHImageResultIsDegradedKey] as? Bool) == true {
        return
      }
      if let data {
        DispatchQueue.global(qos: .userInitiated).async {
          let map = writeImage(data, id: asset.localIdentifier, uti: uti)
          DispatchQueue.main.async { completion(map) }
        }
        return
      }
      requestRendered(asset, completion: completion)
    }
  }

  static func export(image: UIImage, id: String) -> [String: Any]? {
    guard let data = image.jpegData(compressionQuality: 0.92) else { return nil }
    return writeFile(data, id: id, mimeType: "image/jpeg", ext: "jpg", image: image)
  }

  static func writeImage(_ data: Data, id: String, uti: String?) -> [String: Any]? {
    let mime = mimeType(uti: uti, data: data)
    if mime == "image/png" || mime == "image/jpeg" || mime == "image/gif" || mime == "image/webp" {
      if let image = UIImage(data: data), needsDownsample(image) {
        let scaled = downsample(image)
        if mime == "image/png", let png = scaled.pngData() {
          return writeFile(png, id: id, mimeType: "image/png", ext: "png", image: scaled)
        }
        if let jpeg = scaled.jpegData(compressionQuality: 0.92) {
          return writeFile(jpeg, id: id, mimeType: "image/jpeg", ext: "jpg", image: scaled)
        }
      } else {
        let image = UIImage(data: data)
        let ext = mime == "image/png" ? "png" : mime == "image/gif" ? "gif" : mime == "image/webp" ? "webp" : "jpg"
        return writeFile(data, id: id, mimeType: mime, ext: ext, image: image)
      }
    }
    guard let image = UIImage(data: data) else { return nil }
    let scaled = downsample(image)
    guard let jpeg = scaled.jpegData(compressionQuality: 0.92) else { return nil }
    return writeFile(jpeg, id: id, mimeType: "image/jpeg", ext: "jpg", image: scaled)
  }

  private static func writeFile(
    _ data: Data,
    id: String,
    mimeType: String,
    ext: String,
    image: UIImage?
  ) -> [String: Any]? {
    let dir = exportDir()
    let url = dir.appendingPathComponent(UUID().uuidString).appendingPathExtension(ext)
    do {
      try data.write(to: url, options: .atomic)
    } catch {
      return nil
    }
    let width: Int
    let height: Int
    if let image {
      width = Int(image.size.width * image.scale)
      height = Int(image.size.height * image.scale)
    } else {
      width = 0
      height = 0
    }
    return [
      "id": id,
      "path": url.path,
      "width": width,
      "height": height,
      "size": data.count,
      "mimeType": mimeType,
      "durationMs": 0,
    ]
  }

  private static func mimeType(uti: String?, data: Data) -> String {
    switch uti {
    case "public.png": return "image/png"
    case "public.jpeg": return "image/jpeg"
    case "com.compuserve.gif": return "image/gif"
    case "org.webmproject.webp", "public.webp": return "image/webp"
    default: break
    }
    if data.count >= 8, data.starts(with: [0x89, 0x50, 0x4E, 0x47]) { return "image/png" }
    if data.count >= 3, data[0] == 0xFF, data[1] == 0xD8, data[2] == 0xFF { return "image/jpeg" }
    if data.count >= 6, data.starts(with: [0x47, 0x49, 0x46, 0x38]) { return "image/gif" }
    if data.count >= 12, data.starts(with: [0x52, 0x49, 0x46, 0x46]),
       data[8] == 0x57, data[9] == 0x45, data[10] == 0x42, data[11] == 0x50 {
      return "image/webp"
    }
    return "image/heic"
  }

  private static func needsDownsample(_ image: UIImage) -> Bool {
    max(image.size.width, image.size.height) > maxEdge
  }

  static func exportVideo(url: URL, id: String) -> [String: Any]? {
    let asset = AVURLAsset(url: url)
    let durationMs = Int(CMTimeGetSeconds(asset.duration) * 1000)
    var width = 0
    var height = 0
    if let track = asset.tracks(withMediaType: .video).first {
      let size = track.naturalSize.applying(track.preferredTransform)
      width = Int(abs(size.width))
      height = Int(abs(size.height))
    }
    let dir = exportDir()
    let dest = dir.appendingPathComponent(UUID().uuidString).appendingPathExtension("mp4")
    do {
      if FileManager.default.fileExists(atPath: dest.path) {
        try FileManager.default.removeItem(at: dest)
      }
      try FileManager.default.copyItem(at: url, to: dest)
    } catch {
      return nil
    }
    let size = (try? FileManager.default.attributesOfItem(atPath: dest.path)[.size] as? NSNumber)?.intValue ?? 0
    return [
      "id": id,
      "path": dest.path,
      "width": width,
      "height": height,
      "size": size,
      "mimeType": "video/mp4",
      "durationMs": durationMs,
    ]
  }

  private static func requestRendered(_ asset: PHAsset, completion: @escaping ([String: Any]?) -> Void) {
    let opts = PHImageRequestOptions()
    opts.isNetworkAccessAllowed = true
    opts.deliveryMode = .highQualityFormat
    opts.isSynchronous = false
    PHImageManager.default().requestImage(
      for: asset,
      targetSize: PHImageManagerMaximumSize,
      contentMode: .aspectFit,
      options: opts
    ) { image, info in
      if (info?[PHImageCancelledKey] as? Bool) == true {
        DispatchQueue.main.async { completion(nil) }
        return
      }
      if (info?[PHImageResultIsDegradedKey] as? Bool) == true {
        return
      }
      guard let image else {
        DispatchQueue.main.async { completion(nil) }
        return
      }
      DispatchQueue.global(qos: .userInitiated).async {
        let map = export(image: image, id: asset.localIdentifier)
        DispatchQueue.main.async { completion(map) }
      }
    }
  }

  private static func exportDir() -> URL {
    let base =
      FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first
      ?? FileManager.default.temporaryDirectory
    let dir = base.appendingPathComponent("kim_media_picker", isDirectory: true)
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return dir
  }

  private static func downsample(_ image: UIImage) -> UIImage {
    let size = image.size
    let longest = max(size.width, size.height)
    guard longest > maxEdge else { return image }
    let scale = maxEdge / longest
    let next = CGSize(width: size.width * scale, height: size.height * scale)
    let renderer = UIGraphicsImageRenderer(size: next)
    return renderer.image { _ in
      image.draw(in: CGRect(origin: .zero, size: next))
    }
  }
}

# kim_media_picker

In-repo Flutter plugin for KIM. Native WeChat-style **相册** and **拍摄**.

## Album

- `pickSingle()` — one photo
- `pickMultiple({maxCount: 9})` — numbered multi-select

## Camera

- `takePhoto()` / `capture(mode: photo)` — stills only
- `takeVideo()` / `capture(mode: video)` — tap to start / stop, like a stock camera
- `capture()` default `mixed` — 拍照 / 录像 switch. Photo: tap still, long-press video (WeChat). Video: tap records.

Android is CameraX + MediaStore. iOS is AVFoundation + PhotoKit.

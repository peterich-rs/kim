import AVFoundation
import Photos
import UIKit

final class CameraViewController: UIViewController, AVCapturePhotoCaptureDelegate, AVCaptureFileOutputRecordingDelegate {
  var onFinish: ([[String: Any]]) -> Void
  private let mode: String
  private var lane: String
  private let session = AVCaptureSession()
  private let photoOutput = AVCapturePhotoOutput()
  private let movieOutput = AVCaptureMovieFileOutput()
  private var preview: AVCaptureVideoPreviewLayer?
  private var current: AVCaptureDevice.Position = .back
  private var flash: AVCaptureDevice.FlashMode = .off
  private let previewHost = UIView()
  private let captured = UIImageView()
  private let closeBtn = UIButton(type: .system)
  private let flashBtn = UIButton(type: .system)
  private let shutter = UIButton(type: .custom)
  private let switchBtn = UIButton(type: .system)
  private let albumThumb = UIImageView()
  private let photoTab = UIButton(type: .system)
  private let videoTab = UIButton(type: .system)
  private let modeRow = UIStackView()
  private let timerView = UILabel()
  private let reviewBar = UIView()
  private var shot: UIImage?
  private var videoURL: URL?
  private var longPressRecording = false
  private var recordStartedAt = Date()
  private var timer: Timer?
  private let sessionQueue = DispatchQueue(label: "kim.media_picker.camera")

  private var allowsPhoto: Bool { mode != "video" }
  private var allowsVideo: Bool { mode != "photo" }
  private var videoLane: Bool { !allowsPhoto || lane == "video" }

  init(mode: String = "mixed", onFinish: @escaping ([[String: Any]]) -> Void) {
    self.mode = mode
    self.lane = mode == "video" ? "video" : "photo"
    self.onFinish = onFinish
    super.init(nibName: nil, bundle: nil)
  }

  required init?(coder: NSCoder) { nil }

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .black
    buildUi()
    applyLaneChrome()
    requestCamera()
    loadAlbumThumb()
  }

  override func viewDidLayoutSubviews() {
    super.viewDidLayoutSubviews()
    preview?.frame = previewHost.bounds
  }

  override var prefersStatusBarHidden: Bool { true }

  private func requestCamera() {
    switch AVCaptureDevice.authorizationStatus(for: .video) {
    case .authorized:
      requestMicThenConfigure()
    case .notDetermined:
      AVCaptureDevice.requestAccess(for: .video) { [weak self] ok in
        DispatchQueue.main.async {
          if ok { self?.requestMicThenConfigure() } else { self?.denied("请在设置中开启相机权限") }
        }
      }
    default:
      denied("请在设置中开启相机权限")
    }
  }

  private func requestMicThenConfigure() {
    guard allowsVideo else {
      configureSession()
      return
    }
    switch AVCaptureDevice.authorizationStatus(for: .audio) {
    case .authorized, .denied, .restricted:
      configureSession()
    case .notDetermined:
      AVCaptureDevice.requestAccess(for: .audio) { [weak self] _ in
        DispatchQueue.main.async { self?.configureSession() }
      }
    @unknown default:
      configureSession()
    }
  }

  private func denied(_ message: String) {
    let alert = UIAlertController(title: nil, message: message, preferredStyle: .alert)
    alert.addAction(UIAlertAction(title: "取消", style: .cancel) { [weak self] _ in self?.cancel() })
    alert.addAction(UIAlertAction(title: "去设置", style: .default) { [weak self] _ in
      if let url = URL(string: UIApplication.openSettingsURLString) {
        UIApplication.shared.open(url)
      }
      self?.cancel()
    })
    present(alert, animated: true)
  }

  private func configureSession() {
    sessionQueue.async { [weak self] in
      guard let self else { return }
      self.session.beginConfiguration()
      self.session.sessionPreset = .high
      self.session.inputs.forEach { self.session.removeInput($0) }
      self.session.outputs.forEach { self.session.removeOutput($0) }
      let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: self.current)
      guard let device, let input = try? AVCaptureDeviceInput(device: device) else {
        self.session.commitConfiguration()
        DispatchQueue.main.async { self.noCamera() }
        return
      }
      if self.session.canAddInput(input) { self.session.addInput(input) }
      if self.allowsVideo,
         let audio = AVCaptureDevice.default(for: .audio),
         let audioInput = try? AVCaptureDeviceInput(device: audio),
         self.session.canAddInput(audioInput) {
        self.session.addInput(audioInput)
      }
      if self.allowsPhoto, self.session.canAddOutput(self.photoOutput) {
        self.session.addOutput(self.photoOutput)
      }
      if self.allowsVideo, self.session.canAddOutput(self.movieOutput) {
        self.session.addOutput(self.movieOutput)
      }
      self.session.commitConfiguration()
      self.session.startRunning()
      DispatchQueue.main.async {
        let layer = AVCaptureVideoPreviewLayer(session: self.session)
        layer.videoGravity = .resizeAspectFill
        self.previewHost.layer.sublayers?.forEach { $0.removeFromSuperlayer() }
        self.previewHost.layer.addSublayer(layer)
        self.preview = layer
        layer.frame = self.previewHost.bounds
      }
    }
  }

  private func noCamera() {
    let alert = UIAlertController(title: nil, message: "当前设备没有可用相机", preferredStyle: .alert)
    alert.addAction(UIAlertAction(title: "确定", style: .default) { [weak self] _ in self?.cancel() })
    present(alert, animated: true)
  }

  private func applyLaneChrome() {
    let photoOn = !videoLane
    photoTab.setTitleColor(photoOn ? .white : UIColor.white.withAlphaComponent(0.55), for: .normal)
    videoTab.setTitleColor(photoOn ? UIColor.white.withAlphaComponent(0.55) : .white, for: .normal)
    photoTab.titleLabel?.font = .systemFont(ofSize: photoOn ? 16 : 14, weight: .semibold)
    videoTab.titleLabel?.font = .systemFont(ofSize: photoOn ? 14 : 16, weight: .semibold)
    modeRow.isHidden = mode != "mixed"
    shutter.backgroundColor = videoLane ? UIColor(red: 0.9, green: 0.22, blue: 0.21, alpha: 1) : .white
    shutter.layer.cornerRadius = 28
    if !movieOutput.isRecording {
      timerView.isHidden = true
    }
  }

  private func buildUi() {
    previewHost.backgroundColor = .black
    captured.contentMode = .scaleAspectFill
    captured.clipsToBounds = true
    captured.isHidden = true
    closeBtn.setTitle("✕", for: .normal)
    closeBtn.setTitleColor(.white, for: .normal)
    closeBtn.titleLabel?.font = .systemFont(ofSize: 22)
    closeBtn.addTarget(self, action: #selector(cancel), for: .touchUpInside)
    flashBtn.setTitle("闪光灯关", for: .normal)
    flashBtn.setTitleColor(.white, for: .normal)
    flashBtn.addTarget(self, action: #selector(cycleFlash), for: .touchUpInside)
    timerView.textColor = .white
    timerView.font = .monospacedDigitSystemFont(ofSize: 16, weight: .medium)
    timerView.isHidden = true
    shutter.backgroundColor = .white
    shutter.layer.borderWidth = 4
    shutter.layer.borderColor = UIColor.white.cgColor
    let tap = UITapGestureRecognizer(target: self, action: #selector(onShutterTap))
    let hold = UILongPressGestureRecognizer(target: self, action: #selector(onShutterHold(_:)))
    hold.minimumPressDuration = 0.35
    tap.require(toFail: hold)
    shutter.addGestureRecognizer(tap)
    shutter.addGestureRecognizer(hold)
    switchBtn.setTitle("翻转", for: .normal)
    switchBtn.setTitleColor(.white, for: .normal)
    switchBtn.addTarget(self, action: #selector(flip), for: .touchUpInside)
    albumThumb.contentMode = .scaleAspectFill
    albumThumb.clipsToBounds = true
    albumThumb.layer.cornerRadius = 6
    albumThumb.backgroundColor = UIColor.white.withAlphaComponent(0.15)
    albumThumb.isUserInteractionEnabled = true
    albumThumb.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(openAlbum)))
    photoTab.setTitle("拍照", for: .normal)
    videoTab.setTitle("录像", for: .normal)
    photoTab.addTarget(self, action: #selector(selectPhotoLane), for: .touchUpInside)
    videoTab.addTarget(self, action: #selector(selectVideoLane), for: .touchUpInside)
    modeRow.axis = .horizontal
    modeRow.alignment = .center
    modeRow.distribution = .equalSpacing
    modeRow.spacing = 28
    modeRow.addArrangedSubview(photoTab)
    modeRow.addArrangedSubview(videoTab)

    let retakeBtn = UIButton(type: .system)
    retakeBtn.setTitle("重拍", for: .normal)
    retakeBtn.setTitleColor(.white, for: .normal)
    retakeBtn.titleLabel?.font = .systemFont(ofSize: 16)
    retakeBtn.addTarget(self, action: #selector(retake), for: .touchUpInside)
    let useBtn = UIButton(type: .system)
    useBtn.setTitle("✓", for: .normal)
    useBtn.setTitleColor(.white, for: .normal)
    useBtn.titleLabel?.font = .systemFont(ofSize: 22, weight: .bold)
    useBtn.backgroundColor = KimPickerStyle.accent
    useBtn.layer.cornerRadius = 28
    useBtn.addTarget(self, action: #selector(useCapture), for: .touchUpInside)
    reviewBar.isHidden = true
    [previewHost, captured, closeBtn, flashBtn, timerView, shutter, switchBtn, albumThumb, modeRow, reviewBar, retakeBtn, useBtn].forEach {
      $0.translatesAutoresizingMaskIntoConstraints = false
    }
    view.addSubview(previewHost)
    view.addSubview(captured)
    view.addSubview(closeBtn)
    view.addSubview(flashBtn)
    view.addSubview(timerView)
    view.addSubview(shutter)
    view.addSubview(switchBtn)
    view.addSubview(albumThumb)
    view.addSubview(modeRow)
    view.addSubview(reviewBar)
    reviewBar.addSubview(retakeBtn)
    reviewBar.addSubview(useBtn)
    NSLayoutConstraint.activate([
      previewHost.topAnchor.constraint(equalTo: view.topAnchor),
      previewHost.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      previewHost.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      previewHost.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      captured.topAnchor.constraint(equalTo: view.topAnchor),
      captured.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      captured.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      captured.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      closeBtn.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 8),
      closeBtn.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 12),
      flashBtn.centerYAnchor.constraint(equalTo: closeBtn.centerYAnchor),
      flashBtn.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -12),
      timerView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 10),
      timerView.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      shutter.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      shutter.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -28),
      shutter.widthAnchor.constraint(equalToConstant: 72),
      shutter.heightAnchor.constraint(equalToConstant: 72),
      albumThumb.centerYAnchor.constraint(equalTo: shutter.centerYAnchor),
      albumThumb.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 28),
      albumThumb.widthAnchor.constraint(equalToConstant: 48),
      albumThumb.heightAnchor.constraint(equalToConstant: 48),
      switchBtn.centerYAnchor.constraint(equalTo: shutter.centerYAnchor),
      switchBtn.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
      modeRow.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      modeRow.bottomAnchor.constraint(equalTo: shutter.topAnchor, constant: -18),
      reviewBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      reviewBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      reviewBar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      reviewBar.heightAnchor.constraint(equalToConstant: 120),
      retakeBtn.leadingAnchor.constraint(equalTo: reviewBar.leadingAnchor, constant: 24),
      retakeBtn.centerYAnchor.constraint(equalTo: reviewBar.centerYAnchor, constant: -8),
      useBtn.trailingAnchor.constraint(equalTo: reviewBar.trailingAnchor, constant: -24),
      useBtn.centerYAnchor.constraint(equalTo: retakeBtn.centerYAnchor),
      useBtn.widthAnchor.constraint(equalToConstant: 56),
      useBtn.heightAnchor.constraint(equalToConstant: 56),
    ])
    shutter.layer.cornerRadius = 36
  }

  private func loadAlbumThumb() {
    let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)
    guard status == .authorized || status == .limited else { return }
    let opts = PHFetchOptions()
    opts.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
    opts.fetchLimit = 1
    let fetch = PHAsset.fetchAssets(with: .image, options: opts)
    guard let asset = fetch.firstObject else { return }
    PHImageManager.default().requestImage(
      for: asset,
      targetSize: CGSize(width: 96, height: 96),
      contentMode: .aspectFill,
      options: nil
    ) { [weak self] image, _ in
      self?.albumThumb.image = image
    }
  }

  @objc private func cancel() {
    sessionQueue.async { self.session.stopRunning() }
    dismiss(animated: true) { self.onFinish([]) }
  }

  @objc private func cycleFlash() {
    switch flash {
    case .off:
      flash = .on
      flashBtn.setTitle("闪光灯开", for: .normal)
    case .on:
      flash = .auto
      flashBtn.setTitle("自动", for: .normal)
    default:
      flash = .off
      flashBtn.setTitle("闪光灯关", for: .normal)
    }
  }

  @objc private func flip() {
    current = current == .back ? .front : .back
    configureSession()
  }

  @objc private func selectPhotoLane() {
    guard !movieOutput.isRecording else { return }
    lane = "photo"
    applyLaneChrome()
  }

  @objc private func selectVideoLane() {
    guard !movieOutput.isRecording else { return }
    lane = "video"
    applyLaneChrome()
  }

  @objc private func onShutterTap() {
    if movieOutput.isRecording {
      movieOutput.stopRecording()
      return
    }
    if videoLane {
      startRecording(fromLongPress: false)
    } else {
      takeStill()
    }
  }

  @objc private func onShutterHold(_ gesture: UILongPressGestureRecognizer) {
    guard allowsVideo, !videoLane else { return }
    if gesture.state == .began {
      startRecording(fromLongPress: true)
    } else if gesture.state == .ended || gesture.state == .cancelled {
      if longPressRecording, movieOutput.isRecording {
        longPressRecording = false
        movieOutput.stopRecording()
      }
    }
  }

  private func takeStill() {
    let settings = AVCapturePhotoSettings()
    if photoOutput.supportedFlashModes.contains(flash) {
      settings.flashMode = flash
    }
    photoOutput.capturePhoto(with: settings, delegate: self)
  }

  private func startRecording(fromLongPress: Bool) {
    guard allowsVideo, !movieOutput.isRecording else { return }
    if AVCaptureDevice.authorizationStatus(for: .audio) != .authorized {
      let alert = UIAlertController(title: nil, message: "录像需要麦克风权限", preferredStyle: .alert)
      alert.addAction(UIAlertAction(title: "确定", style: .default))
      present(alert, animated: true)
      return
    }
    let dir = FileManager.default.temporaryDirectory
    let url = dir.appendingPathComponent("kim_capture_\(UUID().uuidString).mp4")
    longPressRecording = fromLongPress
    recordStartedAt = Date()
    movieOutput.startRecording(to: url, recordingDelegate: self)
    timerView.isHidden = false
    shutter.layer.cornerRadius = 8
    tickTimer()
  }

  private func tickTimer() {
    timer?.invalidate()
    timer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] t in
      guard let self, self.movieOutput.isRecording else {
        t.invalidate()
        return
      }
      let sec = Int(Date().timeIntervalSince(self.recordStartedAt))
      self.timerView.text = String(format: "%02d:%02d", sec / 60, sec % 60)
      if sec >= 60 {
        self.movieOutput.stopRecording()
      }
    }
  }

  func photoOutput(_ output: AVCapturePhotoOutput, didFinishProcessingPhoto photo: AVCapturePhoto, error: Error?) {
    guard error == nil, let data = photo.fileDataRepresentation(), let image = UIImage(data: data) else { return }
    DispatchQueue.main.async {
      self.shot = image
      self.videoURL = nil
      self.showReview(image: image)
    }
  }

  func fileOutput(
    _ output: AVCaptureFileOutput,
    didFinishRecordingTo outputFileURL: URL,
    from connections: [AVCaptureConnection],
    error: Error?
  ) {
    timer?.invalidate()
    DispatchQueue.main.async {
      self.timerView.isHidden = true
      self.applyLaneChrome()
      let tooShort = Date().timeIntervalSince(self.recordStartedAt) < 0.5
      if error != nil || tooShort {
        try? FileManager.default.removeItem(at: outputFileURL)
        if tooShort, error == nil {
          let toast = UILabel()
          toast.text = "录像时间太短"
          toast.textColor = .white
          toast.backgroundColor = UIColor.black.withAlphaComponent(0.78)
          toast.font = .systemFont(ofSize: 14)
          toast.textAlignment = .center
          toast.layer.cornerRadius = 8
          toast.clipsToBounds = true
          toast.frame = CGRect(x: 40, y: self.view.bounds.midY, width: self.view.bounds.width - 80, height: 40)
          self.view.addSubview(toast)
          UIView.animate(withDuration: 0.3, delay: 1.0) { toast.alpha = 0 } completion: { _ in
            toast.removeFromSuperview()
          }
        }
        return
      }
      self.videoURL = outputFileURL
      self.shot = nil
      let gen = AVAssetImageGenerator(asset: AVURLAsset(url: outputFileURL))
      gen.appliesPreferredTrackTransform = true
      let cg = try? gen.copyCGImage(at: .zero, actualTime: nil)
      self.showReview(image: cg.map { UIImage(cgImage: $0) })
    }
  }

  private func showReview(image: UIImage?) {
    captured.image = image
    captured.isHidden = false
    shutter.isHidden = true
    flashBtn.isHidden = true
    switchBtn.isHidden = true
    albumThumb.isHidden = true
    closeBtn.isHidden = true
    modeRow.isHidden = true
    timerView.isHidden = true
    reviewBar.isHidden = false
  }

  @objc private func retake() {
    if let videoURL {
      try? FileManager.default.removeItem(at: videoURL)
    }
    shot = nil
    videoURL = nil
    captured.image = nil
    captured.isHidden = true
    shutter.isHidden = false
    flashBtn.isHidden = false
    switchBtn.isHidden = false
    albumThumb.isHidden = false
    closeBtn.isHidden = false
    reviewBar.isHidden = false
    reviewBar.isHidden = true
    applyLaneChrome()
  }

  @objc private func useCapture() {
    if let videoURL, let map = MediaExport.exportVideo(url: videoURL, id: UUID().uuidString) {
      try? FileManager.default.removeItem(at: videoURL)
      sessionQueue.async { self.session.stopRunning() }
      dismiss(animated: true) { self.onFinish([map]) }
      return
    }
    guard let shot, let map = MediaExport.export(image: shot, id: UUID().uuidString) else { return }
    sessionQueue.async { self.session.stopRunning() }
    dismiss(animated: true) { self.onFinish([map]) }
  }

  @objc private func openAlbum() {
    PickerStore.shared.reset(maxCount: 1)
    let album = AlbumViewController { [weak self] maps in
      if maps.isEmpty {
        return
      }
      self?.sessionQueue.async { self?.session.stopRunning() }
      self?.dismiss(animated: false) { self?.onFinish(maps) }
    }
    let nav = UINavigationController(rootViewController: album)
    nav.modalPresentationStyle = .fullScreen
    present(nav, animated: true)
  }
}

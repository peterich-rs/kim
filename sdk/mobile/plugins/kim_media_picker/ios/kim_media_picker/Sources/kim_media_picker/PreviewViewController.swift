import Photos
import UIKit

final class PreviewViewController: UIViewController, UIScrollViewDelegate {
  private let assets: [PHAsset]
  private var index: Int
  private let onDone: ([[String: Any]]?) -> Void
  private let pager = UIScrollView()
  private let badge = UIButton(type: .custom)
  private let sendBtn = UIButton(type: .system)
  private let manager = PHCachingImageManager()
  private var sending = false

  init(assets: [PHAsset], index: Int, onDone: @escaping ([[String: Any]]?) -> Void) {
    self.assets = assets
    self.index = min(max(index, 0), max(assets.count - 1, 0))
    self.onDone = onDone
    super.init(nibName: nil, bundle: nil)
  }

  required init?(coder: NSCoder) { nil }

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .black
    navigationItem.leftBarButtonItem = UIBarButtonItem(
      title: "返回",
      style: .plain,
      target: self,
      action: #selector(back)
    )
    navigationItem.leftBarButtonItem?.tintColor = .white
    navigationController?.navigationBar.barStyle = .black
    pager.isPagingEnabled = true
    pager.showsHorizontalScrollIndicator = false
    pager.delegate = self
    pager.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(pager)
    let bar = UIView()
    bar.backgroundColor = UIColor.black.withAlphaComponent(0.8)
    bar.translatesAutoresizingMaskIntoConstraints = false
    badge.addTarget(self, action: #selector(toggle), for: .touchUpInside)
    sendBtn.addTarget(self, action: #selector(send), for: .touchUpInside)
    sendBtn.titleLabel?.font = .boldSystemFont(ofSize: 14)
    sendBtn.layer.cornerRadius = 4
    sendBtn.contentEdgeInsets = UIEdgeInsets(top: 6, left: 14, bottom: 6, right: 14)
    badge.translatesAutoresizingMaskIntoConstraints = false
    sendBtn.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(bar)
    view.addSubview(badge)
    bar.addSubview(sendBtn)
    NSLayoutConstraint.activate([
      pager.topAnchor.constraint(equalTo: view.topAnchor),
      pager.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      pager.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      pager.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      bar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      bar.heightAnchor.constraint(equalToConstant: 88),
      sendBtn.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -16),
      sendBtn.topAnchor.constraint(equalTo: bar.topAnchor, constant: 14),
      badge.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 8),
      badge.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),
      badge.widthAnchor.constraint(equalToConstant: 28),
      badge.heightAnchor.constraint(equalToConstant: 28),
    ])
    layoutPages()
    refreshChrome()
  }

  override func viewDidLayoutSubviews() {
    super.viewDidLayoutSubviews()
    pager.contentSize = CGSize(width: view.bounds.width * CGFloat(assets.count), height: view.bounds.height)
    pager.contentOffset = CGPoint(x: view.bounds.width * CGFloat(index), y: 0)
    for (i, sub) in pager.subviews.enumerated() {
      sub.frame = CGRect(x: view.bounds.width * CGFloat(i), y: 0, width: view.bounds.width, height: view.bounds.height)
    }
  }

  private func layoutPages() {
    pager.subviews.forEach { $0.removeFromSuperview() }
    for (i, asset) in assets.enumerated() {
      let image = UIImageView()
      image.contentMode = .scaleAspectFit
      image.frame = CGRect(x: view.bounds.width * CGFloat(i), y: 0, width: view.bounds.width, height: view.bounds.height)
      pager.addSubview(image)
      manager.requestImage(
        for: asset,
        targetSize: CGSize(width: view.bounds.width * 2, height: view.bounds.height * 2),
        contentMode: .aspectFit,
        options: nil
      ) { img, _ in
        image.image = img
      }
    }
    pager.contentSize = CGSize(width: view.bounds.width * CGFloat(assets.count), height: view.bounds.height)
  }

  func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
    let w = max(scrollView.bounds.width, 1)
    index = Int(round(scrollView.contentOffset.x / w))
    refreshChrome()
  }

  private func current() -> PHAsset { assets[index] }

  private func refreshChrome() {
    let asset = current()
    let n = PickerStore.shared.index(of: asset.localIdentifier)
    badge.layer.cornerRadius = 14
    badge.clipsToBounds = true
    badge.setTitleColor(.white, for: .normal)
    if n > 0 {
      badge.backgroundColor = KimPickerStyle.accent
      badge.setTitle("\(n)", for: .normal)
      badge.layer.borderWidth = 0
    } else {
      badge.backgroundColor = UIColor.black.withAlphaComponent(0.2)
      badge.setTitle("", for: .normal)
      badge.layer.borderWidth = 1
      badge.layer.borderColor = UIColor.white.cgColor
    }
    let count = PickerStore.shared.selected.count
    sendBtn.isEnabled = true
    sendBtn.setTitle(count > 0 ? "发送(\(count))" : "发送", for: .normal)
    sendBtn.backgroundColor = count > 0 ? KimPickerStyle.accent : UIColor(white: 0.85, alpha: 1)
    sendBtn.setTitleColor(count > 0 ? .white : UIColor(white: 0.53, alpha: 1), for: .normal)
  }

  @objc private func back() {
    onDone(nil)
    navigationController?.popViewController(animated: true)
  }

  @objc private func toggle() {
    if PickerStore.shared.toggle(current(), from: view) {
      refreshChrome()
    }
  }

  @objc private func send() {
    guard !sending else { return }
    if PickerStore.shared.selected.isEmpty {
      _ = PickerStore.shared.toggle(current(), from: view)
    }
    let chosen = PickerStore.shared.selectedAssets()
    guard !chosen.isEmpty else { return }
    sending = true
    MediaExport.export(chosen) { [weak self] maps in
      self?.onDone(maps)
    }
  }
}

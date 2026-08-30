import Photos
import UIKit

final class AlbumViewController: UIViewController, UICollectionViewDataSource, UICollectionViewDelegate {
  var onFinish: ([[String: Any]]) -> Void
  private let manager = PHCachingImageManager()
  private var assets: [PHAsset] = []
  private var albums: [(id: String, name: String, count: Int, cover: PHAsset?)] = []
  private var bucket = "all"
  private var grid: UICollectionView!
  private var empty = UILabel()
  private var previewBtn = UIButton(type: .system)
  private var sendBtn = UIButton(type: .system)
  private var titleBtn = UIButton(type: .system)
  private var sheet: UITableView?
  private var scrim: UIView?
  private var sending = false

  init(onFinish: @escaping ([[String: Any]]) -> Void) {
    self.onFinish = onFinish
    super.init(nibName: nil, bundle: nil)
  }

  required init?(coder: NSCoder) { nil }

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = KimPickerStyle.bg
    navigationItem.leftBarButtonItem = UIBarButtonItem(
      title: "取消",
      style: .plain,
      target: self,
      action: #selector(cancel)
    )
    navigationItem.leftBarButtonItem?.tintColor = KimPickerStyle.text
    titleBtn.setTitle("所有照片 ▾", for: .normal)
    titleBtn.setTitleColor(KimPickerStyle.text, for: .normal)
    titleBtn.titleLabel?.font = .boldSystemFont(ofSize: 17)
    titleBtn.addTarget(self, action: #selector(toggleAlbums), for: .touchUpInside)
    navigationItem.titleView = titleBtn
    buildGrid()
    buildBar()
    requestPhotos()
  }

  private func requestPhotos() {
    let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)
    switch status {
    case .authorized, .limited:
      load()
    case .notDetermined:
      PHPhotoLibrary.requestAuthorization(for: .readWrite) { [weak self] next in
        DispatchQueue.main.async {
          if next == .authorized || next == .limited {
            self?.load()
          } else {
            self?.denied()
          }
        }
      }
    default:
      denied()
    }
  }

  private func denied() {
    let alert = UIAlertController(title: nil, message: "请在设置中开启相册权限", preferredStyle: .alert)
    alert.addAction(UIAlertAction(title: "取消", style: .cancel) { [weak self] _ in self?.cancel() })
    alert.addAction(UIAlertAction(title: "去设置", style: .default) { [weak self] _ in
      if let url = URL(string: UIApplication.openSettingsURLString) {
        UIApplication.shared.open(url)
      }
      self?.cancel()
    })
    present(alert, animated: true)
  }

  private func load() {
    let opts = PHFetchOptions()
    opts.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
    let fetch = PHAsset.fetchAssets(with: .image, options: opts)
    var all: [PHAsset] = []
    fetch.enumerateObjects { asset, _, _ in all.append(asset) }
    PickerStore.shared.assets = all
    assets = all
    albums = [("all", "所有照片", all.count, all.first)]
    let smart = PHAssetCollection.fetchAssetCollections(with: .smartAlbum, subtype: .any, options: nil)
    let user = PHAssetCollection.fetchAssetCollections(with: .album, subtype: .any, options: nil)
    [smart, user].forEach { list in
      list.enumerateObjects { collection, _, _ in
        if collection.assetCollectionSubtype == .smartAlbumAllHidden {
          return
        }
        let title = collection.localizedTitle ?? ""
        if title == "最近删除" || title == "Recently Deleted" {
          return
        }
        let inner = PHAsset.fetchAssets(in: collection, options: opts)
        guard inner.count > 0 else { return }
        self.albums.append((collection.localIdentifier, collection.localizedTitle ?? "相册", inner.count, inner.firstObject))
      }
    }
    empty.isHidden = !assets.isEmpty
    grid.reloadData()
    refreshBar()
  }

  private func applyBucket(_ id: String) {
    bucket = id
    let opts = PHFetchOptions()
    opts.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
    if id == "all" {
      assets = PickerStore.shared.assets
    } else {
      let cols = PHAssetCollection.fetchAssetCollections(withLocalIdentifiers: [id], options: nil)
      if let col = cols.firstObject {
        var next: [PHAsset] = []
        PHAsset.fetchAssets(in: col, options: opts).enumerateObjects { asset, _, _ in next.append(asset) }
        assets = next
      }
    }
    let name = albums.first { $0.id == id }?.name ?? "所有照片"
    titleBtn.setTitle("\(name) ▾", for: .normal)
    empty.isHidden = !assets.isEmpty
    grid.reloadData()
  }

  private func buildGrid() {
    let layout = UICollectionViewFlowLayout()
    let gap: CGFloat = 2
    let side = floor((view.bounds.width - gap * 3) / 4)
    layout.itemSize = CGSize(width: side, height: side)
    layout.minimumInteritemSpacing = gap
    layout.minimumLineSpacing = gap
    grid = UICollectionView(frame: .zero, collectionViewLayout: layout)
    grid.backgroundColor = KimPickerStyle.bg
    grid.dataSource = self
    grid.delegate = self
    grid.register(Cell.self, forCellWithReuseIdentifier: "cell")
    grid.translatesAutoresizingMaskIntoConstraints = false
    empty.text = "没有照片"
    empty.textColor = KimPickerStyle.muted
    empty.textAlignment = .center
    empty.isHidden = true
    empty.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(grid)
    view.addSubview(empty)
    NSLayoutConstraint.activate([
      grid.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
      grid.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      grid.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      empty.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      empty.centerYAnchor.constraint(equalTo: view.centerYAnchor),
    ])
  }

  private func buildBar() {
    let bar = UIView()
    bar.backgroundColor = KimPickerStyle.bar
    bar.translatesAutoresizingMaskIntoConstraints = false
    previewBtn.setTitle("预览", for: .normal)
    previewBtn.setTitleColor(KimPickerStyle.text, for: .normal)
    previewBtn.titleLabel?.font = .systemFont(ofSize: 16)
    previewBtn.addTarget(self, action: #selector(preview), for: .touchUpInside)
    sendBtn.setTitle("发送", for: .normal)
    sendBtn.setTitleColor(.white, for: .normal)
    sendBtn.titleLabel?.font = .boldSystemFont(ofSize: 14)
    sendBtn.backgroundColor = KimPickerStyle.accent
    sendBtn.layer.cornerRadius = 4
    sendBtn.contentEdgeInsets = UIEdgeInsets(top: 6, left: 14, bottom: 6, right: 14)
    sendBtn.addTarget(self, action: #selector(send), for: .touchUpInside)
    previewBtn.translatesAutoresizingMaskIntoConstraints = false
    sendBtn.translatesAutoresizingMaskIntoConstraints = false
    bar.addSubview(previewBtn)
    bar.addSubview(sendBtn)
    view.addSubview(bar)
    NSLayoutConstraint.activate([
      bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      bar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      bar.topAnchor.constraint(equalTo: grid.bottomAnchor),
      bar.heightAnchor.constraint(equalToConstant: 52 + view.safeAreaInsets.bottom),
      previewBtn.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 16),
      previewBtn.topAnchor.constraint(equalTo: bar.topAnchor, constant: 12),
      sendBtn.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -12),
      sendBtn.centerYAnchor.constraint(equalTo: previewBtn.centerYAnchor),
    ])
  }

  private func refreshBar() {
    let n = PickerStore.shared.selected.count
    previewBtn.alpha = n > 0 ? 1 : 0.35
    previewBtn.isEnabled = n > 0
    sendBtn.isEnabled = n > 0
    if n == 0 || PickerStore.shared.maxCount == 1 {
      sendBtn.setTitle("发送", for: .normal)
    } else {
      sendBtn.setTitle("发送(\(n))", for: .normal)
    }
    sendBtn.backgroundColor = n > 0 ? KimPickerStyle.accent : UIColor(white: 0.85, alpha: 1)
    sendBtn.setTitleColor(n > 0 ? .white : UIColor(white: 0.53, alpha: 1), for: .normal)
    grid.reloadData()
  }

  @objc private func cancel() {
    dismiss(animated: true) { self.onFinish([]) }
  }

  @objc private func send() {
    guard !sending else { return }
    sending = true
    let chosen = PickerStore.shared.selectedAssets()
    MediaExport.export(chosen) { [weak self] maps in
      guard let self else { return }
      if maps.isEmpty {
        self.sending = false
        let alert = UIAlertController(title: nil, message: "无法导出照片，请换一张再试", preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "确定", style: .default))
        self.present(alert, animated: true)
        return
      }
      self.dismiss(animated: true) { self.onFinish(maps) }
    }
  }

  @objc private func preview() {
    let chosen = PickerStore.shared.selectedAssets()
    guard !chosen.isEmpty else { return }
    openPreview(assets: chosen, index: 0)
  }

  @objc private func toggleAlbums() {
    if let sheet {
      scrim?.removeFromSuperview()
      sheet.removeFromSuperview()
      self.sheet = nil
      scrim = nil
      return
    }
    let dim = UIView(frame: view.bounds)
    dim.backgroundColor = UIColor.black.withAlphaComponent(0.45)
    dim.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(toggleAlbums)))
    let table = UITableView(frame: CGRect(x: 0, y: view.safeAreaInsets.top, width: view.bounds.width, height: min(360, view.bounds.height * 0.5)))
    table.dataSource = self
    table.delegate = self
    table.register(UITableViewCell.self, forCellReuseIdentifier: "album")
    table.rowHeight = 76
    view.addSubview(dim)
    view.addSubview(table)
    scrim = dim
    sheet = table
    table.reloadData()
  }

  private func openPreview(assets: [PHAsset], index: Int) {
    let preview = PreviewViewController(assets: assets, index: index) { [weak self] maps in
      if let maps {
        self?.dismiss(animated: true) { self?.onFinish(maps) }
      } else {
        self?.refreshBar()
      }
    }
    navigationController?.pushViewController(preview, animated: true)
  }

  func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
    assets.count
  }

  func collectionView(_ collectionView: UICollectionView, cellForItemAt indexPath: IndexPath) -> UICollectionViewCell {
    let cell = collectionView.dequeueReusableCell(withReuseIdentifier: "cell", for: indexPath) as! Cell
    let asset = assets[indexPath.item]
    let size = cell.bounds.size.width * UIScreen.main.scale
    manager.requestImage(
      for: asset,
      targetSize: CGSize(width: size, height: size),
      contentMode: .aspectFill,
      options: nil
    ) { image, _ in
      cell.image.image = image
    }
    cell.bind(index: PickerStore.shared.index(of: asset.localIdentifier))
    cell.onBadge = { [weak self] in
      guard let self else { return }
      if PickerStore.shared.toggle(asset, from: self.view) {
        self.refreshBar()
      }
    }
    return cell
  }

  func collectionView(_ collectionView: UICollectionView, didSelectItemAt indexPath: IndexPath) {
    PickerStore.shared.assets = assets
    openPreview(assets: assets, index: indexPath.item)
  }

  final class Cell: UICollectionViewCell {
    let image = UIImageView()
    let dim = UIView()
    let badge = UIButton(type: .custom)
    var onBadge: (() -> Void)?

    override init(frame: CGRect) {
      super.init(frame: frame)
      image.contentMode = .scaleAspectFill
      image.clipsToBounds = true
      dim.backgroundColor = UIColor.black.withAlphaComponent(0.2)
      dim.isHidden = true
      badge.titleLabel?.font = .boldSystemFont(ofSize: 12)
      badge.setTitleColor(.white, for: .normal)
      badge.addTarget(self, action: #selector(tapBadge), for: .touchUpInside)
      [image, dim, badge].forEach {
        $0.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview($0)
      }
      NSLayoutConstraint.activate([
        image.topAnchor.constraint(equalTo: contentView.topAnchor),
        image.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
        image.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
        image.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
        dim.topAnchor.constraint(equalTo: image.topAnchor),
        dim.leadingAnchor.constraint(equalTo: image.leadingAnchor),
        dim.trailingAnchor.constraint(equalTo: image.trailingAnchor),
        dim.bottomAnchor.constraint(equalTo: image.bottomAnchor),
        badge.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 6),
        badge.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -6),
        badge.widthAnchor.constraint(equalToConstant: 24),
        badge.heightAnchor.constraint(equalToConstant: 24),
      ])
    }

    required init?(coder: NSCoder) { nil }

    func bind(index: Int) {
      dim.isHidden = index == 0
      badge.layer.cornerRadius = 12
      badge.clipsToBounds = true
      if index > 0 {
        badge.backgroundColor = KimPickerStyle.accent
        badge.setTitle(PickerStore.shared.maxCount == 1 ? "" : "\(index)", for: .normal)
        badge.layer.borderWidth = 0
      } else {
        badge.backgroundColor = UIColor.black.withAlphaComponent(0.2)
        badge.setTitle("", for: .normal)
        badge.layer.borderWidth = 1
        badge.layer.borderColor = UIColor.white.cgColor
      }
    }

    @objc private func tapBadge() { onBadge?() }
  }
}

extension AlbumViewController: UITableViewDataSource, UITableViewDelegate {
  func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { albums.count }

  func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
    let cell = tableView.dequeueReusableCell(withIdentifier: "album", for: indexPath)
    let album = albums[indexPath.row]
    cell.textLabel?.text = "\(album.name)（\(album.count)）"
    cell.imageView?.contentMode = .scaleAspectFill
    cell.imageView?.clipsToBounds = true
    if let cover = album.cover {
      manager.requestImage(for: cover, targetSize: CGSize(width: 112, height: 112), contentMode: .aspectFill, options: nil) { image, _ in
        cell.imageView?.image = image
        cell.setNeedsLayout()
      }
    }
    return cell
  }

  func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
    applyBucket(albums[indexPath.row].id)
    toggleAlbums()
  }
}

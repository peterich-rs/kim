Pod::Spec.new do |s|
  s.name             = 'kim_media_picker'
  s.version          = '0.1.0'
  s.summary          = 'WeChat-style camera and album picker for KIM.'
  s.description      = <<-DESC
Native camera (AVFoundation) and PhotoKit album with WeChat-like chrome.
                       DESC
  s.homepage         = 'https://github.com/im'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'KIM' => 'kim@local' }
  s.source           = { :path => '.' }
  s.source_files = 'kim_media_picker/Sources/kim_media_picker/**/*.swift'
  s.dependency 'Flutter'
  s.platform = :ios, '15.0'
  s.frameworks = 'AVFoundation', 'Photos', 'UIKit', 'CoreMedia'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES', 'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386' }
  s.swift_version = '5.0'
  s.resource_bundles = {'kim_media_picker_privacy' => ['kim_media_picker/Sources/kim_media_picker/PrivacyInfo.xcprivacy']}
end

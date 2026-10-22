Pod::Spec.new do |s|
  s.name = 'OshwhubCore'
  s.version = '0.1.0'
  s.summary = 'Rust business core for OSHWHub'
  s.description = 'Local Rust core bridge; no application logic is duplicated in Swift.'
  s.author = 'gouzil'
  s.homepage = 'https://github.com/gouzil/oshwhub_app'
  s.license = { :type => 'MIT' }
  s.platforms = { :ios => '15.1' }
  s.source = { :git => 'https://github.com/gouzil/oshwhub_app.git' }
  s.static_framework = true
  s.dependency 'ExpoModulesCore'
  s.source_files = '*.{h,swift}'
  s.public_header_files = 'OshwhubCore.h'
  s.vendored_frameworks = 'OshwhubRust.xcframework'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
end

Pod::Spec.new do |s|
  s.description = <<-DESC
    Usage: add this to your podspec:
      s.dependency 'Usage Example', '~> 2.0'
    s.name = 'NotTheName'
  DESC
  s.name = 'Heredoc'
  s.version = '1.0.0'
  s.summary = 'The #1 heredoc demo'
  s.homepage = "https://github.com/example/#{s.name}"
  s.license = { :type => 'MIT', :file => 'LICENSE' }
  s.author = { 'Example' => 'dev@example.com' }
  s.source = { :git => "https://github.com/example/#{s.name}.git", :tag => s.version.to_s }
  s.dependency 'AFNetworking', '~> 4.0'
  s.ios.dependency 'Core', "~> #{s.version}"
  Pod::UI.warn "Note: s.dependency 'Deprecated Thing' is gone"
  puts "s.dependency 'Fake thing', '1.0'"
=begin
  s.dependency 'BlockCommented'
=end
end

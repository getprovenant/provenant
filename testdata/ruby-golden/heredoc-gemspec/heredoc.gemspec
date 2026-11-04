Gem::Specification.new do |s|
  s.name = "heredoc-demo"
  s.version = "1.0.0"
  s.summary = "The #1 heredoc demo" # trailing comment
  s.description = <<~DESC
    Add to your Gemfile:
      s.add_dependency "usage example", "~> 2.0"
    s.name = "not the name"
  DESC
  s.license = "MIT"
  s.add_dependency "rake", ">= 1.0"
  s.add_development_dependency "rspec", "~> 3.0"
  s.post_install_message = <<-MSG
    s.add_runtime_dependency "post install", "1.0"
  MSG
=begin
  s.add_dependency "block commented", "1.0"
=end
end

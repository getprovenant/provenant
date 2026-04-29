$LOAD_PATH << File.expand_path('lib', __dir__)

require 'example/version'

Gem::Specification.new do |specification|
  specification.name = Example::NAME
  specification.version = Example::VERSION
  specification.homepage = Example::HOMEPAGE
  specification.authors = Example::AUTHORS
  specification.email = Example::EMAIL
  specification.summary = Example::SUMMARY
  specification.add_dependency 'json', '~> 2.0'
end

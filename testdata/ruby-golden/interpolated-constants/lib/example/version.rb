module Example
  MAJOR = '1'.freeze
  MINOR = '2'.freeze
  PATCH = '3'.freeze
  NAME = 'example-interpolated'.freeze
  VERSION = [MAJOR, MINOR, PATCH].join('.').freeze
  AUTHORS = ['Example Author'].freeze
  EMAIL = ['example@example.com'].freeze
  SUMMARY = 'Interpolated constant fixture'.freeze
  HOMEPAGE = "https://example.com/#{NAME}".freeze
end

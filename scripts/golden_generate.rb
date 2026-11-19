# frozen_string_literal: true

# Generates golden/<type>/<case>.svg and golden/manifest.json from the Ruby
# Etassia path. Run from a etassia checkout (feat/mermaid-parg or later):
#
#   cd $ETASSIA && bundle exec ruby /path/to/etassia-rs/scripts/golden_generate.rb
#
# Env: GOLDEN_OUT (default: <this repo>/golden), ETASSIA_CORPORA — a
# semicolon list of "<corpus dir>:<glob>" pairs (default: the mermaid
# corpus). Sources are copied next to each svg, always under the .mmd
# name the golden tests expect. Every case is rendered with today pinned
# to 2026-01-01; cases that raise or time out are listed under "errors".
require "bundler/setup"
require "date"
require "digest"
require "fileutils"
require "json"
require "timeout"

$LOAD_PATH.unshift(File.join(Dir.pwd, "lib"))
require "etassia"
require "etassia/notation/plantuml"

OUT = ENV.fetch("GOLDEN_OUT", File.expand_path("../golden", __dir__))
CORPORA = ENV.fetch("ETASSIA_CORPORA", "#{Dir.pwd}/spec/mermaid:*/*.mmd")
      .split(";")
      .map { |pair| pair.split(":", 2) }
TODAY = Date.new(2026, 1, 1)

FileUtils.rm_rf(OUT)
FileUtils.mkdir_p(OUT)
cases = []
errors = []
corpora = CORPORA.flat_map do |(dir, glob)|
  Dir.glob(File.join(dir, glob)).sort.map { |path| [path, dir] }
end
corpora.each do |path, corpus|
  rel = path.delete_prefix("#{corpus}/")
  stem = rel.sub(/\.[^.]+\z/, "")
  source = File.binread(path)
  begin
    svg = Timeout.timeout(10) { Etassia::Engine.new.render(source, today: TODAY) }
  rescue StandardError, SystemStackError => e
    errors << { "case" => rel, "error" => e.class.name }
    next
  end
  next unless svg.is_a?(String)

  dest = File.join(OUT, "#{stem}.svg")
  FileUtils.mkdir_p(File.dirname(dest))
  File.binwrite(dest, svg)
  File.binwrite(File.join(OUT, "#{stem}.mmd"), source)
  cases << { "case" => "#{stem}.mmd", "type" => stem.split(/[\/.]/).first,
             "source_sha256" => Digest::SHA256.hexdigest(source),
             "svg_sha256" => Digest::SHA256.hexdigest(svg.b) }
end
File.write(File.join(OUT, "manifest.json"),
           "#{JSON.pretty_generate({ "today" => TODAY.to_s, "cases" => cases,
                                     "errors" => errors })}\n")
puts "golden: #{cases.size} rendered, #{errors.size} errors -> #{OUT}"

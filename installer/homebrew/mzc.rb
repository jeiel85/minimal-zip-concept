class Mzc < Formula
  desc "Minimal Zip Concept - A visual educational compressor written in Rust"
  homepage "https://github.com/jeiel85/minimal-zip-concept"
  url "https://github.com/jeiel85/minimal-zip-concept/archive/refs/tags/v0.11.1.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Update with actual SHA-256 after tag is published
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/mzc", "--version"
  end
end

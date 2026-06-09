class Mzc < Formula
  desc "Minimal Zip Concept - A visual educational compressor written in Rust"
  homepage "https://github.com/jeiel85/minimal-zip-concept"
  url "https://github.com/jeiel85/minimal-zip-concept/archive/refs/tags/v0.12.1.tar.gz"
  sha256 "1c663601946d363dacf8d535e1a4b61e8eb5fa8f67ae4b3bc2af3e3e32b1c6f2"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/mzc", "--version"
  end
end

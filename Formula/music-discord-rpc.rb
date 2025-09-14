class MusicDiscordRpc < Formula
  desc "Cross-platform Discord rich presence for music with album cover and progress bar"
  homepage "https://github.com/patryk-ku/music-discord-rpc"
  version "0.6.2"
  license "MIT"

  depends_on "media-control"

  on_intel do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-amd64.tar.gz"
    sha256 "4ae98e85b16e7d9d4814b60429cd2b09c42dee3132b5bec1fa394b30aa77a05d"
  end

  on_arm do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-arm64.tar.gz"
    sha256 "17054746cdde5b68a5c26fcec8add442a628a31e42f62396a3055d8bfd8e5115"
  end

  def install
    bin.install "music-discord-rpc"
  end

  service do
    run [opt_bin/"music-discord-rpc"]
    keep_alive true
    environment_variables PATH: "#{HOMEBREW_PREFIX}/bin:/usr/bin:/bin"
    log_path var/"log/music-discord-rpc.log"
    error_log_path var/"log/music-discord-rpc.error.log"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/music-discord-rpc --version")
  end
end

class MusicDiscordRpc < Formula
  desc "Cross-platform Discord rich presence for music with album cover and progress bar"
  homepage "https://github.com/patryk-ku/music-discord-rpc"
  version "0.6.2"
  license "MIT"

  depends_on "media-control"

  on_intel do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-amd64.tar.gz"
    sha256 "25074bf3eae5fafe5405f38d87c65e4aa0b5a7c92574f7379b429a46ce5bb422"
  end

  on_arm do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-arm64.tar.gz"
    sha256 "9fe07e593b1c66872590a5488c93e1dc2078a990f57aff6f79a9e9bd2fd356f6"
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

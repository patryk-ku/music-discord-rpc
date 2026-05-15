class MusicDiscordRpc < Formula
  desc "Cross-platform Discord rich presence for music with album cover and progress bar"
  homepage "https://github.com/patryk-ku/music-discord-rpc"
  version "0.7.0"
  license "MIT"

  depends_on "media-control"

  on_intel do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-amd64.tar.gz"
    sha256 "88e9dbfa78faeb0727abb966f2c85e50f09b1177d9e2d904c8fa5eeb9fd1b824"
  end

  on_arm do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-arm64.tar.gz"
    sha256 "a057135b98968b36aa2bfb0029f44092f3ff7315c949eb98e3ab7617666a8f71"
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

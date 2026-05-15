class MusicDiscordRpc < Formula
  desc "Cross-platform Discord rich presence for music with album cover and progress bar"
  homepage "https://github.com/patryk-ku/music-discord-rpc"
  version "0.7.0"
  license "MIT"

  depends_on "media-control"

  on_intel do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-amd64.tar.gz"
    sha256 "c4b75ea355991d8d686d3baa5e91042c21d13076f88b07dac36eeb7c82a21644"
  end

  on_arm do
    url "https://github.com/patryk-ku/music-discord-rpc/releases/download/v#{version}/music-discord-rpc-macos-arm64.tar.gz"
    sha256 "d288c751e83fed8e1ae27d40dabbbdc14242cc69079e205fae9d59d2cd522764"
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

# Maintainer: Patryk Kurdziel <patryk.kurdziel@protonmail.com>

_pkgname=music-discord-rpc
pkgname="${_pkgname}-bin"
pkgver=0.6.2
pkgrel=1
pkgdesc='Cross-platform Discord rich presence for music with album cover and progress bar support.'
url="https://github.com/patryk-ku/${_pkgname}"
license=('MIT')
arch=('x86_64')
provides=("${_pkgname}")
conflicts=("${_pkgname}")
source=("${_pkgname}-v${pkgver}::${url}/releases/download/v${pkgver}/${_pkgname}"
	"${_pkgname}-v${pkgver}.service::https://raw.githubusercontent.com/patryk-ku/${_pkgname}/refs/tags/v${pkgver}/${_pkgname}.service"
	"LICENSE-v${pkgver}::https://raw.githubusercontent.com/patryk-ku/${_pkgname}/refs/tags/v${pkgver}/LICENSE")
sha512sums=('SKIP' 'SKIP' 'SKIP')

package() {
	install -Dm755 "${_pkgname}-v${pkgver}" "${pkgdir}/usr/bin/${_pkgname}"
	install -Dm644 "${_pkgname}-v${pkgver}.service" "${pkgdir}/usr/lib/systemd/user/${_pkgname}.service"
	install -Dm644 "LICENSE-v${pkgver}" "${pkgdir}/usr/share/licenses/${_pkgname}/LICENSE"
}

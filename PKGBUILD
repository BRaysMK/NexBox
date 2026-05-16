# 示例：在 Arch Linux 上从源码打包时的起点模板。
# 实际打包 Tauri 应用前请根据上游发布流程补全 source、sha256sums 与 build() 细节。

pkgname=gamepro
pkgver=0.1.0
pkgrel=1
pkgdesc="GamePro performance monitor (Tauri + Next.js)"
arch=("x86_64")
url="https://example.com/gamepro"
license=("MIT")
makedepends=("npm" "rust" "cargo" "webkit2gtk" "openssl" "curl" "wget" "libappindicator-gtk3" "librsvg" "patchelf")

build() {
  echo "请根据本仓库 README 完成 npm 与 cargo 构建步骤。"
}

package() {
  echo "请将构建产物安装到 \"\${pkgdir}/usr/bin\" 等路径。"
}

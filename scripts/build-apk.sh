#!/usr/bin/env bash
# Builds a release APK using the official macroquad Android image (notfl3/cargo-apk),
# which bundles cargo-quad-apk, the Android SDK/NDK and a JDK.
#
#   ./scripts/build-apk.sh            # build only
#   ./scripts/build-apk.sh --install  # build, then install on a USB-connected phone via adb
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v docker >/dev/null; then
  echo "docker not found. Install Docker Desktop, or: brew install colima docker && colima start --arch x86_64" >&2
  exit 1
fi

docker run --rm --platform linux/amd64 \
  -v "$PWD":/root/src -w /root/src \
  -v emberfall-cargo-registry:/root/.cargo/registry \
  notfl3/cargo-apk \
  bash -c 'set -e
    rustup toolchain install stable --profile minimal --target armv7-linux-androideabi,aarch64-linux-android
    cargo +stable quad-apk build --release'

APK=$(ls target/android-artifacts/release/apk/*.apk | head -1)
mkdir -p dist && cp "$APK" dist/emberfall.apk
echo "APK: dist/emberfall.apk"

if [[ "${1:-}" == "--install" ]]; then
  ADB=$(command -v adb || echo "$HOME/Library/Android/sdk/platform-tools/adb")
  "$ADB" install -r dist/emberfall.apk
fi

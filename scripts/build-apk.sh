#!/usr/bin/env bash
# Builds a release APK using the official macroquad Android image (notfl3/cargo-apk),
# which bundles cargo-quad-apk, the Android SDK/NDK and a JDK.
#
#   ./scripts/build-apk.sh            # build only
#   ./scripts/build-apk.sh --install  # build, then install on a USB-connected phone via adb
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v docker >/dev/null; then
  echo "docker not found. Install Docker Desktop, or:" >&2
  echo "  brew install colima docker" >&2
  echo "  colima start --vm-type vz --vz-rosetta --mount \"$PWD:w\" --mount \"\$HOME:w\"" >&2
  exit 1
fi

docker run --rm --platform linux/amd64 \
  -v "$PWD":/root/src -w /root/src \
  -v emberfall-cargo-registry:/root/.cargo/registry \
  $( [ -f "$HOME/.android/debug.keystore" ] && echo "-v $HOME/.android/debug.keystore:/root/.android/debug.keystore:ro" ) \
  notfl3/cargo-apk \
  bash -c 'set -e
    rustup toolchain install stable --profile minimal --target armv7-linux-androideabi,aarch64-linux-android
    cp Cargo.lock /tmp/Cargo.lock.orig
            # cargo-quad-apk bundles an older cargo that reads lockfile v3 only.
            sed -i "s/^version = 4$/version = 3/" Cargo.lock
            status=0
            cargo +stable quad-apk build --release || status=$?
            cp /tmp/Cargo.lock.orig Cargo.lock
            exit $status'

APK=$(ls target/android-artifacts/release/apk/*.apk | head -1)
mkdir -p dist && cp "$APK" dist/emberfall.apk
echo "APK: dist/emberfall.apk"

if [[ "${1:-}" == "--install" ]]; then
  ADB=$(command -v adb || echo "$HOME/Library/Android/sdk/platform-tools/adb")
  "$ADB" install -r dist/emberfall.apk
fi

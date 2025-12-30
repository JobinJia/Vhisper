#!/bin/bash
# 编译并运行 Swift Demo
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CORE_DIR="$(dirname "$SCRIPT_DIR")"
XCFRAMEWORK="$CORE_DIR/out/VhisperCore.xcframework"
OUT="$SCRIPT_DIR/vhisper-demo"

echo "=== Building Swift Demo ==="

# 检查 xcframework
if [ ! -d "$XCFRAMEWORK" ]; then
    echo "Error: xcframework not found at $XCFRAMEWORK"
    echo "Run build-xcframework.sh first"
    exit 1
fi

# 编译
swiftc \
    -F "$CORE_DIR/out/VhisperCore.xcframework/macos-arm64_x86_64" \
    -I "$CORE_DIR/out/VhisperCore.xcframework/macos-arm64_x86_64/Headers" \
    -L "$CORE_DIR/out/VhisperCore.xcframework/macos-arm64_x86_64" \
    -lvhisper_core \
    -framework Security \
    -framework CoreAudio \
    -framework AudioToolbox \
    -framework CoreFoundation \
    -framework SystemConfiguration \
    -Xlinker -dead_strip \
    -o "$OUT" \
    "$SCRIPT_DIR/VhisperBridge.swift" \
    "$SCRIPT_DIR/main.swift"

echo "Build successful: $OUT"
echo ""

# 运行
echo "=== Running Demo ==="
"$OUT"

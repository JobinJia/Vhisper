#!/bin/bash
# 构建 vhisper-core xcframework
# 用法: ./build-xcframework.sh [debug|release]

set -e

MODE="${1:-release}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# 输出目录
OUT_DIR="$SCRIPT_DIR/out"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

echo "=== 构建 vhisper-core ($MODE) ==="

# 1. 添加 Rust 目标架构
echo "[1/4] 检查 Rust 目标..."
rustup target add aarch64-apple-darwin x86_64-apple-darwin 2>/dev/null || true

# 2. 编译双架构
echo "[2/4] 编译 aarch64-apple-darwin..."
if [ "$MODE" = "release" ]; then
    cargo build --release --target aarch64-apple-darwin
else
    cargo build --target aarch64-apple-darwin
fi

echo "[2/4] 编译 x86_64-apple-darwin..."
if [ "$MODE" = "release" ]; then
    cargo build --release --target x86_64-apple-darwin
else
    cargo build --target x86_64-apple-darwin
fi

# 3. 合并为 universal 静态库
echo "[3/4] 合并为 universal 静态库..."
TARGET_DIR="$SCRIPT_DIR/../../target"
if [ "$MODE" = "release" ]; then
    ARM64_LIB="$TARGET_DIR/aarch64-apple-darwin/release/libvhisper_core.a"
    X64_LIB="$TARGET_DIR/x86_64-apple-darwin/release/libvhisper_core.a"
else
    ARM64_LIB="$TARGET_DIR/aarch64-apple-darwin/debug/libvhisper_core.a"
    X64_LIB="$TARGET_DIR/x86_64-apple-darwin/debug/libvhisper_core.a"
fi

lipo -create "$ARM64_LIB" "$X64_LIB" -output "$OUT_DIR/libvhisper_core.a"

echo "  -> $(lipo -info "$OUT_DIR/libvhisper_core.a")"

# 4. 创建 xcframework
echo "[4/4] 创建 xcframework..."

# 准备 headers
mkdir -p "$OUT_DIR/Headers"
cp "$SCRIPT_DIR/include/vhisper_core.h" "$OUT_DIR/Headers/"

# 创建 module.modulemap
cat > "$OUT_DIR/Headers/module.modulemap" << 'EOF'
module VhisperCore {
    header "vhisper_core.h"
    export *
}
EOF

# 创建 xcframework
rm -rf "$OUT_DIR/VhisperCore.xcframework"
xcodebuild -create-xcframework \
    -library "$OUT_DIR/libvhisper_core.a" \
    -headers "$OUT_DIR/Headers" \
    -output "$OUT_DIR/VhisperCore.xcframework"

echo ""
echo "=== 构建完成 ==="
echo "输出: $OUT_DIR/VhisperCore.xcframework"
echo ""
echo "使用方法:"
echo "1. 将 VhisperCore.xcframework 拖入 Xcode 项目"
echo "2. 在 Swift 中: import VhisperCore"

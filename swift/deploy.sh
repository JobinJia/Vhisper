#!/bin/bash
# 部署 vhisper 到 /Applications

# 1. 优雅退出（让 app 自己退出，会触发 applicationWillTerminate）
osascript -e 'quit app "vhisper"' 2>/dev/null && echo "✅ 已退出旧进程" || echo "ℹ️  没有运行中的进程"

# 2. 等一下确保完全退出
sleep 0.5

# 3. 复制新的
app_src=$(ls -td ~/Library/Developer/Xcode/DerivedData/vhisper-*/Build/Products/Debug/vhisper.app 2>/dev/null | head -1)

if [ -n "$app_src" ]; then
    rm -rf /Applications/vhisper.app
    cp -R "$app_src" /Applications/
    echo "✅ 已部署到 /Applications/vhisper.app"

    # 4. 启动
    open /Applications/vhisper.app
    echo "✅ 已启动"
else
    echo "❌ 未找到编译的 app，请先在 Xcode 中编译"
fi

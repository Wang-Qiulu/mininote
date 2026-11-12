#!/usr/bin/env bash
# 本地安装：始终从当前源码构建最新版，清掉旧副本，本机只留一份。
#
#   npm run install:local     （等价于 bash scripts/install-local.sh）
#
# 1. 先 npm run app:build，保证装的一定是最新代码
# 2. 旧副本一律移到废纸篓（可恢复），不做不可逆删除
# 3. 用 mv 而不是 cp 安装产物：构建目录不再残留第二份 .app，
#    启动台 / Spotlight 也就不会出现两个图标
# 4. 重做 ad-hoc 签名并重启应用

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="mininote.app"
BUNDLE="$ROOT/src-tauri/target/release/bundle/macos/$APP"
DEST="/Applications/$APP"
TRASH="$HOME/.Trash"

mkdir -p "$TRASH"
cd "$ROOT"

echo "==> 构建最新版"
npm run app:build

if [[ ! -d "$BUNDLE" ]]; then
  echo "构建产物不存在：$BUNDLE" >&2
  exit 1
fi

echo "==> 停止正在运行的实例"
pkill -x mininote 2>/dev/null || true
sleep 1

trash() {
  local src="$1"
  [[ -e "$src" ]] || return 0
  local name target
  name="$(basename "$src")"
  target="$TRASH/${name%.*}-$(date +%s).app"
  mv "$src" "$target"
  echo "    移到废纸篓：$src"
}

echo "==> 清理旧副本"
trash "/Applications/小记事本.app"                                  # 改名前装过的旧版
trash "$DEST"                                                       # 上次安装的 mininote
trash "$ROOT/src-tauri/target/release/bundle/macos/小记事本.app"  # 旧名构建残留
for old in /tmp/mininote-app-prev-*.app; do
  [[ -e "$old" ]] && trash "$old"
done

echo "==> 安装到 $DEST"
mv "$BUNDLE" "$DEST"

echo "==> 重做 ad-hoc 签名"
codesign --force --deep --sign - "$DEST"
codesign -v "$DEST"

echo "==> 启动"
open "$DEST"
echo "完成：本机只保留 $DEST 一份；旧版都在废纸篓里，可随时恢复。"

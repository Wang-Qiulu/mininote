#!/usr/bin/env python3
"""
只截取 mininote 自己的窗口，不打扰屏幕上的其他内容。

用法：
    python3 scripts/shot-window.py [输出路径] [应用名]

原理：用 CoreGraphics 的 CGWindowListCopyWindowInfo 找到 owner 名为
mininote 的窗口，拿到它的 window number，再交给
`screencapture -l<id>`。只截这一个窗口，不会切换前台应用。
"""
import ctypes
import ctypes.util
import subprocess
import sys
import os

# 开发模式与打包后的进程名现在都是 mininote
APP_NAMES = ("mininote",)

cg = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
cf = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")

cg.CGWindowListCopyWindowInfo.restype = ctypes.c_void_p
cg.CGWindowListCopyWindowInfo.argtypes = [ctypes.c_uint32, ctypes.c_uint32]

cf.CFArrayGetCount.restype = ctypes.c_long
cf.CFArrayGetCount.argtypes = [ctypes.c_void_p]
cf.CFArrayGetValueAtIndex.restype = ctypes.c_void_p
cf.CFArrayGetValueAtIndex.argtypes = [ctypes.c_void_p, ctypes.c_long]

cf.CFDictionaryGetValue.restype = ctypes.c_void_p
cf.CFDictionaryGetValue.argtypes = [ctypes.c_void_p, ctypes.c_void_p]

cf.CFStringGetCString.restype = ctypes.c_bool
cf.CFStringGetCString.argtypes = [
    ctypes.c_void_p,
    ctypes.c_char_p,
    ctypes.c_long,
    ctypes.c_uint32,
]

cf.CFStringCreateWithCString.restype = ctypes.c_void_p
cf.CFStringCreateWithCString.argtypes = [
    ctypes.c_void_p,
    ctypes.c_char_p,
    ctypes.c_uint32,
]

cf.CFNumberGetValue.restype = ctypes.c_bool
cf.CFNumberGetValue.argtypes = [
    ctypes.c_void_p,
    ctypes.c_uint32,
    ctypes.c_void_p,
]

# CoreFoundation 常量
CF_STRING_ENCODING_UTF8 = 0x08000100
CF_NUMBER_SINT32 = 3
CF_NUMBER_SINT64 = 4
kCGWindowListOptionOnScreenOnly = 1
kCGWindowListOptionAll = 0
kCGNullWindowID = 0


def cf_key(name: bytes):
    """构造一个真正的 CFString，用作字典的键（不能用 Python bytes 冒充）"""
    return cf.CFStringCreateWithCString(None, name, CF_STRING_ENCODING_UTF8)


KEY_OWNER = cf_key(b"kCGWindowOwnerName")
KEY_NUMBER = cf_key(b"kCGWindowNumber")
KEY_LAYER = cf_key(b"kCGWindowLayer")


def cf_str(value):
    """CFString → python str"""
    if not value:
        return ""
    buf = ctypes.create_string_buffer(512)
    if cf.CFStringGetCString(value, buf, 512, CF_STRING_ENCODING_UTF8):
        return buf.value.decode("utf-8", "replace")
    return ""


def cf_num(value):
    """CFNumber → python int"""
    if not value:
        return 0
    out = ctypes.c_int64(0)
    for kind in (CF_NUMBER_SINT64, CF_NUMBER_SINT32):
        if cf.CFNumberGetValue(value, kind, ctypes.byref(out)):
            return out.value
    return 0


def find_window(names):
    """在窗口列表里找属于本应用的窗口，返回 (window number, layer)。

    先看「在屏幕上」的列表；找不到就退回全量列表——窗口被别的应用完全
    盖住时，macOS 会把它从「在屏幕上」列表里摘掉，但内容仍然可以截。
    """
    for option in (kCGWindowListOptionOnScreenOnly, kCGWindowListOptionAll):
        windows = cg.CGWindowListCopyWindowInfo(option, kCGNullWindowID)
        if not windows:
            continue
        count = cf.CFArrayGetCount(windows)
        best = None
        for i in range(count):
            info = cf.CFArrayGetValueAtIndex(windows, i)
            if not info:
                continue
            owner = cf_str(cf.CFDictionaryGetValue(info, KEY_OWNER))
            if owner not in names:
                continue
            number = cf_num(cf.CFDictionaryGetValue(info, KEY_NUMBER))
            layer = cf_num(cf.CFDictionaryGetValue(info, KEY_LAYER))
            # layer 0 是普通窗口层，托盘图标在更高的层
            if number and (best is None or layer < best[1]):
                best = (number, layer, None)
        if best:
            return best
    return None


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/mininote-window.png"
    names = (sys.argv[2],) if len(sys.argv) > 2 else APP_NAMES

    best = find_window(names)
    if not best:
        print(f"没找到名为 {'/'.join(names)} 的窗口（应用可能没在运行）")
        return 2

    number, layer, _ = best
    print(f"窗口号 {number}（layer {layer}）")
    result = subprocess.run(
        ["screencapture", "-x", "-o", f"-l{number}", out],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"截图失败: {result.stderr.strip()}")
        return result.returncode
    size = os.path.getsize(out) if os.path.exists(out) else 0
    print(f"已保存 {out}（{size / 1024:.0f} KB）")
    return 0


if __name__ == "__main__":
    sys.exit(main())

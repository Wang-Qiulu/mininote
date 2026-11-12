/**
 * 生成菜单栏模板图标（纯 Node，无第三方依赖）。
 *
 *   node scripts/gen-icons.mjs
 *
 * 产出：src-tauri/icons/tray.png —— 44×44 黑 + alpha，
 *      TrayIconBuilder 以模板模式渲染，系统按菜单栏深浅自动反相。
 *
 * 应用图标不在这里生成：源图是 src-tauri/icons/app-icon-source.png，
 * 换图标时跑 `npx tauri icon src-tauri/icons/app-icon-source.png`
 * 生成 icns / ico / 各尺寸 png。这里刻意不写 icon.png，
 * 免得把设计好的应用图标覆盖回程序化的旧版本。
 */
import zlib from "node:zlib";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ICONS = path.join(ROOT, "src-tauri", "icons");

// ---------- PNG 编码 ----------

let CRC_TABLE = null;
function crc32(buf) {
  if (!CRC_TABLE) {
    CRC_TABLE = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      CRC_TABLE[n] = c;
    }
  }
  let crc = -1;
  for (let i = 0; i < buf.length; i++) crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ buf[i]) & 0xff];
  return (crc ^ -1) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crc]);
}

function encodePNG(width, height, rgba) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0; // filter: None
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---------- 绘制工具 ----------

/** 圆角矩形的有符号距离场，<= 0 表示在内部 */
function sdRoundRect(px, py, x0, y0, x1, y1, r) {
  const cx = Math.max(x0 + r, Math.min(px, x1 - r));
  const cy = Math.max(y0 + r, Math.min(py, y1 - r));
  return Math.hypot(px - cx, py - cy) - r;
}

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

/** 圆角矩形覆盖度：距离 <= 0 为 1，边缘 1px 内线性过渡 */
function cover(dist) {
  return clamp01(0.5 - dist);
}

/** 用 3×3 超采样渲染一张 RGBA 图。shade(x, y) 返回 [r, g, b, a] */
function render(width, height, shade) {
  const out = Buffer.alloc(width * height * 4);
  const S = 3;
  const inv = 1 / (S * S);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      for (let sy = 0; sy < S; sy++) {
        for (let sx = 0; sx < S; sx++) {
          const c = shade(x + (sx + 0.5) / S, y + (sy + 0.5) / S);
          const ca = c[3];
          r += c[0] * ca;
          g += c[1] * ca;
          b += c[2] * ca;
          a += ca;
        }
      }
      const i = (y * width + x) * 4;
      if (a > 0) {
        out[i] = Math.round(r / a);
        out[i + 1] = Math.round(g / a);
        out[i + 2] = Math.round(b / a);
      }
      out[i + 3] = Math.round(clamp01(a * inv) * 255);
    }
  }
  return out;
}

// ---------- 菜单栏模板图标 ----------

function trayIcon(size = 44) {
  const s = size / 44;
  const bars = [
    { x0: 6, x1: 34, y: 10, alpha: 1.0 },
    { x0: 6, x1: 27, y: 18.5, alpha: 0.92 },
    { x0: 6, x1: 20, y: 27, alpha: 0.78 },
  ].map((b) => ({
    x0: b.x0 * s,
    x1: b.x1 * s,
    y0: b.y * s,
    y1: (b.y + 4.2) * s,
    alpha: b.alpha,
  }));

  return render(size, size, (px, py) => {
    for (const bar of bars) {
      const d = sdRoundRect(px, py, bar.x0, bar.y0, bar.x1, bar.y1, 2.1 * s);
      const c = cover(d) * bar.alpha;
      if (c > 0) return [0, 0, 0, c];
    }
    return [0, 0, 0, 0];
  });
}

// ---------- 输出 ----------

fs.mkdirSync(ICONS, { recursive: true });
const tray = encodePNG(44, 44, trayIcon(44));
fs.writeFileSync(path.join(ICONS, "tray.png"), tray);
console.log(`✓ src-tauri/icons/tray.png  (44×44, ${(tray.length / 1024).toFixed(1)} KB)`);

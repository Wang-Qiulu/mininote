/**
 * 生成应用图标与菜单栏模板图标（纯 Node，无第三方依赖）。
 *
 *   node scripts/gen-icons.mjs
 *
 * 产出：
 *   src-tauri/icons/icon.png  1024×1024 应用图标源（后续用 `tauri icon` 生成全尺寸）
 *   src-tauri/icons/tray.png  44×44 菜单栏模板图标（黑 + alpha，系统自动反相）
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
  const dx = px - cx;
  const dy = py - cy;
  return Math.hypot(dx, dy) - r;
}

/** 超椭圆（macOS 风格的连续圆角方形），返回 0..1 的内外度 */
function superellipse(px, py, cx, cy, a, b, n) {
  const x = Math.abs((px - cx) / a);
  const y = Math.abs((py - cy) / b);
  return Math.pow(x, n) + Math.pow(y, n);
}

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const mix = (a, b, t) => a + (b - a) * t;

function hexToRgb(hex) {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

/**
 * 用 3×3 超采样渲染一张 RGBA 图。
 * shade(x, y) 返回 [r, g, b, a]，a 为 0..1。
 */
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

/** 圆角矩形覆盖度：距离 <= 0 为 1，边缘 1px 内线性过渡 */
function cover(dist) {
  return clamp01(0.5 - dist);
}

// ---------- 应用图标 ----------

function appIcon(size = 1024) {
  const s = size / 1024;
  const cx = size / 2;
  const cy = size / 2;
  // macOS 图标内容约占画布 80%
  const a = 412 * s;
  const n = 5;

  const top = hexToRgb("#FFC978");
  const bottom = hexToRgb("#F0715C");
  const bars = [
    { x0: 272, x1: 752, y: 396, alpha: 1.0 },
    { x0: 272, x1: 672, y: 500, alpha: 0.88 },
    { x0: 272, x1: 592, y: 604, alpha: 0.68 },
  ].map((b) => ({
    x0: b.x0 * s,
    x1: b.x1 * s,
    y0: b.y * s,
    y1: (b.y + 62) * s,
    alpha: b.alpha,
  }));

  return render(size, size, (px, py) => {
    // 超椭圆边缘：外圈 1px 抗锯齿
    const se = superellipse(px, py, cx, cy, a, a, n);
    // se 在 1 附近是边界；用有限差分求距离更准，这里用阈值渐变近似
    const edge = 1 / (2.6 * s);
    const shape = clamp01((1.0 - se) / edge + 0.5);
    if (shape <= 0) return [0, 0, 0, 0];

    // 背景渐变（沿对角线）
    const t = clamp01(((px - cx) / a + (py - cy) / a) / 2 + 0.5);
    let r = mix(top[0], bottom[0], t);
    let g = mix(top[1], bottom[1], t);
    let b = mix(top[2], bottom[2], t);

    // 顶部高光，模拟玻璃反射
    const gloss = clamp01(1 - (py - (cy - a)) / (a * 1.1)) ** 2 * 0.28;
    r = mix(r, 255, gloss);
    g = mix(g, 255, gloss);
    b = mix(b, 255, gloss);

    // 底部轻微压暗，增加体积感
    const shade = clamp01((py - (cy + a * 0.1)) / (a * 1.1)) ** 2 * 0.18;
    r = mix(r, 60, shade);
    g = mix(g, 30, shade);
    b = mix(b, 25, shade);

    // 三条便签横线
    for (const bar of bars) {
      const d = sdRoundRect(px, py, bar.x0, bar.y0, bar.x1, bar.y1, 31 * s);
      const c = cover(d) * bar.alpha;
      if (c > 0) {
        r = mix(r, 255, c);
        g = mix(g, 253, c);
        b = mix(b, 250, c);
      }
    }

    return [r, g, b, shape];
  });
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

const icon = encodePNG(1024, 1024, appIcon(1024));
fs.writeFileSync(path.join(ICONS, "icon.png"), icon);
console.log(`✓ src-tauri/icons/icon.png  (1024×1024, ${(icon.length / 1024).toFixed(1)} KB)`);

const tray = encodePNG(44, 44, trayIcon(44));
fs.writeFileSync(path.join(ICONS, "tray.png"), tray);
console.log(`✓ src-tauri/icons/tray.png  (44×44, ${(tray.length / 1024).toFixed(1)} KB)`);

import { useEffect, useState, useRef } from "react";

interface CoverColor {
  hex: string;       // 主色 hex 值
  isLight: boolean;  // 是否为浅色
  rgb: [number, number, number];
}

/**
 * 从图片提取主导颜色
 * 使用 Canvas 采样像素，计算平均颜色
 */
function extractDominantColor(img: HTMLImageElement): CoverColor {
  const canvas = document.createElement("canvas");
  const size = 100; // 缩小到 100x100 加速采样
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (!ctx) return { hex: "#1a1a2e", isLight: false, rgb: [26, 26, 46] };

  ctx.drawImage(img, 0, 0, size, size);
  const data = ctx.getImageData(0, 0, size, size).data;

  let r = 0, g = 0, b = 0;
  let count = 0;

  // 采样每个像素，加权中心区域（忽略边缘）
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const idx = (y * size + x) * 4;
      const alpha = data[idx + 3];
      if (alpha < 128) continue; // 跳过透明像素

      // 中心区域权重更高
      const cx = Math.abs(x - size / 2) / (size / 2);
      const cy = Math.abs(y - size / 2) / (size / 2);
      const weight = 1 - Math.min(cx + cy, 1) * 0.5;

      r += data[idx] * weight;
      g += data[idx + 1] * weight;
      b += data[idx + 2] * weight;
      count += weight;
    }
  }

  if (count === 0) return { hex: "#1a1a2e", isLight: false, rgb: [26, 26, 46] };

  r = Math.round(r / count);
  g = Math.round(g / count);
  b = Math.round(b / count);

  // 增强饱和度
  const avg = (r + g + b) / 3;
  r = Math.min(255, Math.max(0, Math.round(r + (r - avg) * 0.35)));
  g = Math.min(255, Math.max(0, Math.round(g + (g - avg) * 0.35)));
  b = Math.min(255, Math.max(0, Math.round(b + (b - avg) * 0.35)));

  const hex = "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");

  // 计算相对亮度 (WCAG)
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  const isLight = luminance > 0.55;

  return { hex, isLight, rgb: [r, g, b] };
}

/**
 * 从专辑封面提取主色
 */
export function useCoverColor(coverUrl: string): CoverColor {
  const [color, setColor] = useState<CoverColor>({ hex: "#1a1a2e", isLight: false, rgb: [26, 26, 46] });
  const lastUrlRef = useRef("");

  useEffect(() => {
    if (!coverUrl || coverUrl === lastUrlRef.current) return;
    lastUrlRef.current = coverUrl;

    const img = new Image();
    img.crossOrigin = "anonymous";

    img.onload = () => {
      const c = extractDominantColor(img);
      setColor(c);
    };

    img.onerror = () => {
      setColor({ hex: "#1a1a2e", isLight: false, rgb: [26, 26, 46] });
    };

    img.src = coverUrl;
  }, [coverUrl]);

  return color;
}

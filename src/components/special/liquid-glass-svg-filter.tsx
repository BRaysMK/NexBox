"use client";

import { useEffect, useRef, useState, useCallback } from "react";

/**
 * 真实液态玻璃 SVG 折射滤镜系统
 *
 * 参考 Mineradio 项目的实现：
 * 1. 使用 feDisplacementMap 对 backdrop 内容进行位移，实现边缘折射
 * 2. RGB 三通道分别位移不同量，产生色差效果
 * 3. 生成一张通用位移贴图，通过 preserveAspectRatio="none" 拉伸适配所有元素
 * 4. 通过 backdrop-filter: url(#filter) 应用到背景
 */

const FILTER_ID = "nexbox-liquid-glass-filter";

/**
 * 检测当前浏览器是否支持 backdrop-filter: url(#filter)
 */
export function supportsSvgBackdropFilter(): boolean {
  try {
    const ua = navigator.userAgent || "";
    if ((/Safari/.test(ua) && !/Chrome/.test(ua)) || /Firefox/.test(ua)) return false;
    const div = document.createElement("div");
    div.style.backdropFilter = `url(#${FILTER_ID})`;
    return div.style.backdropFilter !== "";
  } catch {
    return false;
  }
}

/**
 * 生成通用位移贴图 SVG data URI
 *
 * 使用固定尺寸 400x200 生成，通过 preserveAspectRatio="none" 拉伸到任意元素。
 * 这样所有元素共享同一张贴图，不会因为尺寸不匹配而产生割裂。
 *
 * 贴图原理（桶形畸变 = 放大镜效果）：
 * - Red 通道（水平梯度）：R=0 在左边缘 → 内容左移（向外）；R=255 在右边缘 → 内容右移（向外）
 * - Blue 通道（垂直梯度）：B=0 在顶部 → 内容上移（向外）；B=255 在底部 → 内容下移（向外）
 * - 中心区域用灰色（128）填充并模糊，使中心位移为零，仅边缘有放大折射
 * - 使用 screen 混合模式让 R/B 通道独立互不干扰
 */
function generateUniversalDisplacementMap(): string {
  const width = 400;
  const height = 200;
  const radius = 24;
  const borderWidth = 0.04;
  const edge = Math.min(width, height) * (borderWidth * 0.5);
  const innerW = Math.max(1, width - edge * 2);
  const innerH = Math.max(1, height - edge * 2);

  const svg =
    `<svg viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg">` +
    `<defs>` +
    // R 通道：左=0（向外推）→ 中=128（零位移）→ 右=255（向外推）= 桶形畸变
    `<linearGradient id="glass-red" x1="0%" y1="0%" x2="100%" y2="0%">` +
    `<stop offset="0%" stop-color="rgb(0,0,0)"/>` +
    `<stop offset="50%" stop-color="rgb(128,0,0)"/>` +
    `<stop offset="100%" stop-color="rgb(255,0,0)"/>` +
    `</linearGradient>` +
    // B 通道：上=0（向外推）→ 中=128（零位移）→ 下=255（向外推）= 桶形畸变
    `<linearGradient id="glass-blue" x1="0%" y1="0%" x2="0%" y2="100%">` +
    `<stop offset="0%" stop-color="rgb(0,0,0)"/>` +
    `<stop offset="50%" stop-color="rgb(0,0,128)"/>` +
    `<stop offset="100%" stop-color="rgb(0,0,255)"/>` +
    `</linearGradient>` +
    `</defs>` +
    // 黑色底，圆角外区域为零位移
    `<rect x="0" y="0" width="${width}" height="${height}" fill="black"/>` +
    // R 梯度层
    `<rect x="0" y="0" width="${width}" height="${height}" rx="${radius}" fill="url(#glass-red)"/>` +
    // B 梯度层：screen 混合使 R/B 独立叠加
    `<rect x="0" y="0" width="${width}" height="${height}" rx="${radius}" fill="url(#glass-blue)" style="mix-blend-mode:screen"/>` +
    // 中心灰块：128,128,128 = 零位移，模糊产生平滑过渡
    `<rect x="${edge.toFixed(2)}" y="${edge.toFixed(2)}" width="${innerW.toFixed(2)}" height="${innerH.toFixed(2)}" rx="${radius}" fill="rgb(128,128,128)" style="filter:blur(15px)"/>` +
    `</svg>`;

  return "data:image/svg+xml," + encodeURIComponent(svg);
}

/**
 * 全局 SVG 滤镜定义组件
 *
 * 滤镜链（极简版，避免通道分离导致灰色问题）：
 * 1. feImage: 加载通用位移贴图（拉伸适配）
 * 2. feDisplacementMap: 单次位移，直接对完整彩色内容折射
 * 3. feGaussianBlur: 轻微模糊柔化边缘
 */
export function LiquidGlassSvgFilter() {
  const mapHref = useRef<string>("");

  useEffect(() => {
    mapHref.current = generateUniversalDisplacementMap();
    const img = document.getElementById("nexbox-glass-displacement-map");
    if (img) {
      img.setAttribute("href", mapHref.current);
      try {
        img.setAttributeNS("http://www.w3.org/1999/xlink", "href", mapHref.current);
      } catch {
        // ignore
      }
    }
  }, []);

  return (
    <svg
      className="real-liquid-glass-svg-defs"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <filter
          id={FILTER_ID}
          colorInterpolationFilters="sRGB"
          x="-30%"
          y="-30%"
          width="160%"
          height="160%"
        >
          {/* 通用位移贴图：preserveAspectRatio="none" 拉伸到任意尺寸 */}
          <feImage
            id="nexbox-glass-displacement-map"
            x="0"
            y="0"
            width="100%"
            height="100%"
            preserveAspectRatio="none"
            result="map"
          />

          {/* 单次位移：直接对完整彩色内容折射，不做通道分离 */}
          <feDisplacementMap
            in="SourceGraphic"
            in2="map"
            scale="8"
            xChannelSelector="R"
            yChannelSelector="B"
          />

          {/* 轻微模糊柔化边缘 */}
          <feGaussianBlur stdDeviation="0.6" />
        </filter>
      </defs>
    </svg>
  );
}

/**
 * React Hook：检测 SVG backdrop-filter 支持
 *
 * 由于使用通用贴图，不再需要按元素更新贴图。
 */
export function useLiquidGlassRefraction(enabled: boolean) {
  const [svgSupported] = useState(() => supportsSvgBackdropFilter());

  return {
    svgSupported,
  };
}

export function hexToRgba(hex: string, alpha: number): string {
  const cleanHex = hex.replace("#", "");
  const r = parseInt(cleanHex.slice(0, 2), 16);
  const g = parseInt(cleanHex.slice(2, 4), 16);
  const b = parseInt(cleanHex.slice(4, 6), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

export function getContrastColor(hex: string): string {
  const cleanHex = hex.replace("#", "");
  const r = parseInt(cleanHex.slice(0, 2), 16);
  const g = parseInt(cleanHex.slice(2, 4), 16);
  const b = parseInt(cleanHex.slice(4, 6), 16);
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luminance > 0.5 ? "#1a1a1a" : "#ffffff";
}

export function isValidHexColor(hex: string): boolean {
  return /^#([A-Fa-f0-9]{6}|[A-Fa-f0-9]{3})$/.test(hex);
}

export function normalizeHexColor(hex: string): string {
  if (hex.startsWith("#") && hex.length === 4) {
    const r = hex[1];
    const g = hex[2];
    const b = hex[3];
    return `#${r}${r}${g}${g}${b}${b}`;
  }
  return hex;
}

export function hexToHsv(hex: string): { h: number; s: number; v: number } {
  const clean = hex.replace("#", "");
  const r = parseInt(clean.slice(0, 2), 16) / 255;
  const g = parseInt(clean.slice(2, 4), 16) / 255;
  const b = parseInt(clean.slice(4, 6), 16) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    switch (max) {
      case r: h = ((g - b) / d + (g < b ? 6 : 0)) / 6; break;
      case g: h = ((b - r) / d + 2) / 6; break;
      case b: h = ((r - g) / d + 4) / 6; break;
    }
  }
  return { h: h * 360, s: max === 0 ? 0 : (d / max) * 100, v: max * 100 };
}

export function hsvToHex(h: number, s: number, v: number): string {
  s /= 100;
  v /= 100;
  const i = Math.floor(h / 60);
  const f = h / 60 - i;
  const p = v * (1 - s);
  const q = v * (1 - s * f);
  const t = v * (1 - s * (1 - f));
  let r = 0, g = 0, b = 0;
  switch (i % 6) {
    case 0: r = v; g = t; b = p; break;
    case 1: r = q; g = v; b = p; break;
    case 2: r = p; g = v; b = t; break;
    case 3: r = p; g = q; b = v; break;
    case 4: r = t; g = p; b = v; break;
    case 5: r = v; g = p; b = q; break;
  }
  const toHex = (n: number) => Math.round(n * 255).toString(16).padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`.toUpperCase();
}

export const PRESET_COLORS = [
  { name: "cyan", value: "#98DDD0", labelKey: "settings.appearanceSettings.presetColors.cyan" },
  { name: "blue", value: "#60A5FA", labelKey: "settings.appearanceSettings.presetColors.blue" },
  { name: "purple", value: "#A78BFA", labelKey: "settings.appearanceSettings.presetColors.purple" },
  { name: "pink", value: "#F472B6", labelKey: "settings.appearanceSettings.presetColors.pink" },
  { name: "orange", value: "#FB923C", labelKey: "settings.appearanceSettings.presetColors.orange" },
  { name: "green", value: "#4ADE80", labelKey: "settings.appearanceSettings.presetColors.green" },
  { name: "red", value: "#F87171", labelKey: "settings.appearanceSettings.presetColors.red" },
  { name: "yellow", value: "#FBBF24", labelKey: "settings.appearanceSettings.presetColors.yellow" },
];

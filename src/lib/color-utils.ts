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
  return luminance > 0.5 ? "#1a1a1a" : "#1a1a1a";
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

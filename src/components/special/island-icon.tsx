/**
 * 自定义灵动岛图标 —— 横向圆角条（药丸形），外观贴近 iPhone Dynamic Island
 */
export function IslandIcon({
  size = 24,
  color = "currentColor",
}: {
  size?: number;
  strokeWidth?: number;
  color?: string;
}) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* 外层药丸轮廓 */}
      <rect
        x="2"
        y="7.5"
        width="20"
        height="9"
        rx="4.5"
        fill={color}
        opacity="0.18"
      />
      <rect
        x="2"
        y="7.5"
        width="20"
        height="9"
        rx="4.5"
        stroke={color}
        strokeWidth="1.8"
      />
      {/* 内部小圆点，模拟灵动岛的传感器 */}
      <circle cx="8" cy="12" r="1.4" fill={color} />
      {/* 右侧短横线，模拟动态内容区 */}
      <rect x="13" y="11" width="6" height="2" rx="1" fill={color} opacity="0.7" />
    </svg>
  );
}

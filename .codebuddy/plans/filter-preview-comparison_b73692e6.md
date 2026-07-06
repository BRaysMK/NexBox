---
name: filter-preview-comparison
overview: 在显示器滤镜页面添加图片预览对比功能（左右分割对比、可拖拽调节分割线），并将选择滤镜预设后默认改为不自动开启滤镜开关。
design:
  architecture:
    framework: react
  styleKeywords:
    - 对比预览
    - 液态玻璃
    - 沉浸式
    - 专业调色
    - 动态交互
  fontSystem:
    fontFamily: MiSans-Medium
    heading:
      size: 24px
      weight: 600
    subheading:
      size: 16px
      weight: 500
    body:
      size: 14px
      weight: 400
  colorSystem:
    primary:
      - "#98DDD0"
      - "#00D9FF"
      - "#FF6B9D"
    background:
      - "#111111"
      - "#1a1a1a"
      - "#222222"
    text:
      - "#ffffff"
      - "#e0e0e0"
      - "#888888"
    functional:
      - "#48BB78"
      - "#F56565"
      - "#ECC94B"
todos:
  - id: copy-preview-image
    content: 将根目录 icc.png 复制到 public/icc-preview.png 并验证引用路径
    status: completed
  - id: add-preview-translations
    content: 为 zh/zh-TW/en/ja/de/fr 添加预览相关多语言翻译词条
    status: completed
  - id: build-filter-preview-component
    content: 在 DisplayFilterPage 顶部实现可拖拽分割线的滤镜预览对比组件（CSS filter 近似 + 鼠标拖拽逻辑）
    status: completed
    dependencies:
      - copy-preview-image
  - id: remove-auto-enable
    content: 修改 applyPreset、openCustom、saveAndApply、resetToDefault、handleApplyIcc，移除自动设置 is_active 为 true 的逻辑
    status: completed
  - id: integrate-preview-and-polish
    content: 将预览组件嵌入页面正确位置，兼容液态玻璃主题，优化拖拽性能与边界处理，最终验证构建
    status: completed
    dependencies:
      - build-filter-preview-component
      - remove-auto-enable
---

## 产品概述

在 NexBox 的显示器滤镜页面（DisplayFilterPage）新增滤镜预览功能，并调整滤镜开关的交互逻辑。

## 核心功能

1. **滤镜预览对比器**：在 DisplayFilterPage 页面顶部添加一个图片对比预览区域，使用 icc.png 作为演示素材。

- 左半部分显示原始图片（无滤镜）
- 右半部分显示应用当前滤镜参数后的效果
- 中间有一条垂直分割线，支持拖拽调节左右比例
- 分割线带有明显的拖拽手柄指示器

2. **滤镜开关交互调整**：选择内置预设、自定义设置、ICC 配置或重置默认时，不再自动打开滤镜总开关，保持当前开关状态，仅更新参数；用户必须手动点击开关才能启用滤镜。
3. **预览效果实时同步**：预览区域右侧的滤镜效果根据当前选中的预设/自定义/ICC 参数实时计算并渲染（使用 CSS filter 近似模拟色温、亮度、对比度、饱和度、Gamma 等效果）。

## 技术栈

- 前端：React + TypeScript + Chakra UI + Framer Motion
- 桌面端：Tauri (Rust)
- 构建工具：Vite

## 实现方案

### 预览对比器

- 在 `DisplayFilterPage` 顶部插入一个固定高度的预览卡片组件。
- 使用双层图片叠加布局：
- 底层：原图 `<img>`（无滤镜）
- 上层：带滤镜的 `<img>`，通过 CSS `clip-path: inset(0 0 0 X%)` 或外层 `overflow: hidden` + 绝对定位实现左裁切，只显示右侧部分。
- 或更简洁的方式：两张完全重叠的图片，底层原图始终全宽，上层滤镜图使用 `clip-path: inset(0 0 0 var(--split-position))` 只显示右侧。
- 中间拖拽线：绝对定位的垂直 2px 高亮线条，线条中央放置圆形拖拽手柄。
- 拖拽逻辑：监听 `mousedown`/`touchstart` -> 计算容器相对 X 坐标 -> 设置 CSS 变量 `--split-position`（0~100%），过程中使用 `requestAnimationFrame` 或 React state 更新，松开时停止监听。
- 滤镜效果：使用 CSS `filter` 组合属性近似后端滤镜：
- `brightness()` 对应亮度
- `contrast()` 对应对比度
- `saturate()` 对应饱和度
- 色温：通过 `sepia()` + `hue-rotate()` 或预计算好的颜色矩阵近似，或使用 `drop-shadow` + 混合模式叠加一层暖/冷色 `div`（更直观）。
- Gamma / S-Curve / RGB Boost：使用额外的亮度/对比度微调近似，或在 `mix-blend-mode: overlay` 层叠加微小颜色偏移。
- 由于预览是近似效果，不需要 Tauri 后端参与，纯前端 CSS 即可。

### 开关逻辑调整

- 修改 `applyPreset`、`openCustom`、`saveAndApply`、`resetToDefault`、`handleApplyIcc` 函数：
- 移除对 `is_active` 的强制设为 `true`
- 在更新 `setSettings` 时保留原 `is_active` 值（`...prev, is_active: prev.is_active` 或仅更新参数字段）
- 如果之前没有激活滤镜，仅更新参数，toast 提示改为“参数已更新，请手动启用滤镜”。
- `toggleFilter` 方法保持不变，继续负责实际切换后端滤镜的激活状态。

### 多语言

- 为所有 6 种语言（zh、zh-TW、en、ja、de、fr）补充 `displayFilter.previewTitle`、`displayFilter.previewHint` 等翻译条目。

### 静态资源

- 将根目录 `icc.png` 复制到 `public/icc-preview.png`，确保前端可以通过 `/icc-preview.png` 直接引用。

## 实现要点

- 避免在拖拽时频繁重渲染整个页面：使用 `useRef` 直接操作 DOM 的 `style` 和 `clip-path`，不经过 React state 循环。
- 保持现有页面结构：预览卡片插入在“显示器选择”和“预设”之间，不破坏原有布局。
- 兼容 `liquidGlassEnabled`：预览卡片背景跟随当前毛玻璃设置。
- 回退安全：如果浏览器不支持 `clip-path`，回退到 `overflow: hidden` + 宽度调整。

## 设计概述

在 DisplayFilterPage 页面顶部新增一个视觉冲击感强的滤镜预览对比器，整体延续 NexBox 现有的深色/浅色主题和液态玻璃（Liquid Glass）设计语言。

## 页面布局

1. **预览卡片（顶部）**

- 全宽卡片，圆角 `xl`，高度约 240px（或自适应 16:9）
- 内部图片填充，`object-fit: cover`
- 左侧原始图，右侧带滤镜图
- 中间垂直分割线：2px 宽，使用主题主色高亮，中央带 24px 圆形拖拽手柄（白色背景 + 主色边框 + 左右箭头图标）
- 分割线底部显示小标签："原始 | 滤镜"
- 卡片边缘有 subtle glow / 液态玻璃边框

2. **交互细节**

- 拖拽时整个分割线高亮变粗（4px），手柄放大
- 释放时平滑缩回
- 未启用滤镜时，右侧预览图自动应用参数但带半透明遮罩或提示 "未启用滤镜"
- 鼠标悬停预览区域，光标变为 `ew-resize`

3. **下方控件不变**

- 预设、ICC、自定义参数、开关顺序保持不变
- 开关移至更显眼位置（靠近预览区）

## 风格关键词

- 沉浸式对比
- 液态玻璃
- 动态交互
- 专业级调色工具感
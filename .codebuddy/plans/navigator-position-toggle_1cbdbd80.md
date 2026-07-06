---
name: navigator-position-toggle
overview: 实现导航栏左侧/顶部位置切换功能：默认左侧，可在设置→通用→导航栏位置中切换至顶部，所有页面内容区域自适应布局。
todos:
  - id: add-i18n-keys
    content: 在 6 个语言文件中添加导航栏位置相关翻译 key（navPositionLabel/navPositionDesc/navPositionLeft/navPositionTop）
    status: completed
  - id: update-sidebar
    content: 扩展 Sidebar 组件，支持 position 属性和水平布局，读取 localStorage + 监听 CustomEvent 实现实时切换
    status: completed
    dependencies:
      - add-i18n-keys
  - id: update-main-layout
    content: 修改 MainLayout，根据导航栏位置动态调整内容区域布局和 body 溢出行为
    status: completed
    dependencies:
      - update-sidebar
  - id: update-settings-page
    content: 在 SettingsPage 的 GeneralSettings 导航栏分组中添加位置选择器（CustomSelect 下拉框），持久化并广播变更事件
    status: completed
    dependencies:
      - add-i18n-keys
---

## 用户需求

为 NexBox 应用添加导航栏位置切换功能，允许用户在左侧侧边栏和顶部水平导航之间切换。

## 产品概述

在现有左侧导航栏的基础上，增加顶部水平导航栏模式。用户可以在设置 → 通用 → 导航栏位置中选择「侧边」或「顶部」。默认保持左侧模式，切换到顶部模式后，导航栏固定在标题栏下方，内容区域滚动时不会覆盖导航栏。所有主内容页面需要自动适配两种导航栏模式。

## 核心功能

- **导航栏位置切换**：在设置 → 通用 → 导航栏分组中，使用下拉选择器切换导航栏位置为「侧边」或「顶部」
- **持久化存储**：通过 localStorage（key: `nexbox_nav_position`）保存用户选择，应用重启后保持
- **实时切换**：选择后通过 CustomEvent 广播变更，导航栏和主布局即时响应，无需刷新页面
- **左侧模式**：导航栏固定在窗口左侧居中，内容区域左边距 96px，顶部边距 56px（标题栏高度）
- **顶部模式**：导航栏固定在标题栏下方（top: 48px），水平排列所有导航项，内容区域顶部边距约 104px，无边距约束
- **滚动隔离**：顶部模式下，内容区域独立滚动，导航栏区域不可滚动
- **国际化**：6 种语言完整翻译支持

## 技术选型

- **前端框架**：React 18 + TypeScript（沿用现有）
- **UI 组件库**：Chakra UI（沿用现有）
- **状态管理**：localStorage + CustomEvent 广播（沿用现有模式）
- **国际化**：i18next + react-i18next（沿用现有）

## 实现方案

### 核心策略

不引入新的状态管理库或 Context，完全沿用项目现有的 `localStorage + CustomEvent` 模式，确保与现有代码风格一致、零学习成本。通过给 Sidebar 组件增加 `position` prop，配合 MainLayout 中条件渲染不同的布局样式，实现导航栏位置切换。

### 关键设计决策

1. **复用现有 Sidebar 组件**：不创建新组件，而是扩展 Sidebar 支持两种布局模式（垂直/水平），通过 props 控制。这避免了代码重复，保持毛玻璃效果、主题色、激活状态等逻辑统一。
2. **MainLayout 响应式布局**：MainLayout 读取 localStorage 确定导航栏位置，动态调整内容区域的 margin/padding。使用 CSS 属性而非 JS 动态计算，避免 reflow。
3. **滚动隔离方案**：顶部模式下，导航栏使用 `position: fixed; top: 48px`，内容区域使用 `overflow-y: auto; height: calc(100vh - 104px); margin-top: 104px`。确保内容滚动条在导航栏下方区域，不会覆盖导航栏。
4. **设置入口放置**：在现有「导航栏」分组（`settings.navigation`）中已有「导航栏标签」选项，将「导航栏位置」选择器放在其上方，保持分组语义一致。

### 性能考量

- 切换导航栏位置时，仅涉及 CSS 布局变更，不触发组件卸载/重挂载，性能开销极小
- CustomEvent 监听仅在 Sidebar 和 MainLayout 两个组件中，不影响其他页面
- 顶部模式下导航项列表不变，仍为 10 项，水平排列不会产生性能问题

## 实施细节

### 修改清单

```
d:/NexBox/
├── src/
│   ├── components/
│   │   └── ui/
│   │       ├── sidebar.tsx          # [MODIFY] 扩展支持 position 属性（"left"|"top"），
│   │       │                         读取 localStorage + 监听 CustomEvent，顶部模式下使用水平 Flex 布局
│   │       └── main-layout.tsx      # [MODIFY] 读取 navPosition，动态切换内容区域样式：
│   │                                  left模式: ml="96px" pt="56px"；top模式: pt="104px" ml=0
│   ├── pages/
│   │   └── SettingsPage.tsx         # [MODIFY] GeneralSettings 中导航栏分组增加位置选择器
│   └── locales/
│       ├── zh.json                  # [MODIFY] 新增 navPositionLabel/navPositionDesc/navPositionLeft/navPositionTop
│       ├── en.json                  # [MODIFY] 同上
│       ├── ja.json                  # [MODIFY] 同上
│       ├── fr.json                  # [MODIFY] 同上
│       ├── de.json                  # [MODIFY] 同上
│       └── zh-TW.json               # [MODIFY] 同上
```

### 翻译 Key 定义

```
settings.generalSettings.navPositionLabel     → "导航栏位置"
settings.generalSettings.navPositionDesc      → "选择导航栏显示在侧边还是顶部"
settings.generalSettings.navPositionLeft      → "侧边"
settings.generalSettings.navPositionTop       → "顶部"
```

### 布局计算细节

**左侧模式（现有，不变）**：

- Sidebar: `position: fixed; left: 6; top: 50%; transform: translateY(-50%); z-index: 40`
- 内容区: `ml="96px" pt="56px" overflowY="auto" h="calc(100vh)"`

**顶部模式（新增）**：

- Sidebar: `position: fixed; top: 48px; left: 0; right: 0; z-index: 40; height: 56px`，Flex direction: row，水平居中排列导航项
- 内容区: `pt="104px" overflowY="auto" h="calc(100vh - 104px)"`，html/body 需设置 `overflow: hidden` 防止双滚动条

**关键注意事项**：

- 顶部模式下，TitleBar z-index 为 999，Sidebar z-index 为 40，内容区无 z-index，确保层级正确
- 顶部 Sidebar 需设置 `bg` 背景色或毛玻璃效果，防止内容穿透可见
- body 的 overflow 需由 MainLayout 管理：顶部模式下 body overflow hidden，内容区自身 overflow auto

### 向后兼容

- localStorage 不存在 `nexbox_nav_position` 时，默认值为 `"left"`，保持现有用户体验
- 所有现有功能（导航栏标签显示/隐藏、毛玻璃效果、主题色）在两种模式下均正常工作
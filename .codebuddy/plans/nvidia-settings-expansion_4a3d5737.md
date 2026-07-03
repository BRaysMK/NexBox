---
name: nvidia-settings-expansion
overview: 在 NvidiaDriverPage 中添加约 14 个新的 NVAPI 驱动调节项，完善三大分类（同步与显示、画质与纹理、电源与性能），同时修改后端 nvapi.rs 增加对应常量和选项列表。
todos:
  - id: add-backend-data
    content: 在 nvapi.rs 中添加 15 个新设置常量、选项值和 description
    status: completed
  - id: add-frontend-data
    content: 在 NvidiaDriverPage.tsx 中添加 15 个新 SETTING_IDS 和 5 个分组的重新组织
    status: completed
  - id: add-special-controls
    content: 为新增的 Slider(PRERENDERLIMIT) 和开关类设置添加特殊 UI 控件
    status: completed
    dependencies:
      - add-frontend-data
  - id: verify-build
    content: 运行 cargo check 和 npx tsc 验证编译通过
    status: completed
    dependencies:
      - add-special-controls
---

## 用户需求

在 NVIDIA 显卡设置页面中加入更多的调节项目，并且做好分类。

### 当前状态

目前仅有 7 个设置项，分散在 3 个分组。需扩充至 21 项并重新组织为更清晰的分类。

### 新分组结构（5组）

**组1: 同步与显示** (6项)

- 垂直同步 (已有)
- 最大帧速率 (已有)
- G-Sync 应用控制 (新增 `0x10A879CF`)
- G-Sync 全局启用 (新增 `0x1194F158`)
- 首选刷新率 (新增 `0x0064B541`)
- 垂直同步撕裂控制 (新增 `0x005A375C`)

**组2: 画质与纹理** (8项)

- 纹理过滤质量 (已有)
- 各向异性过滤 (已有)
- 抗锯齿模式 (已有)
- FXAA 快速近似抗锯齿 (已有)
- 环境光遮蔽 (新增 `0x00667329`)
- MFAA 多帧采样 (新增 `0x0098C1AC`)
- 着色器缓存 (新增 `0x00198FFF`)
- 抗锯齿模式选择 (新增 `0x107EFC5B`)

**组3: 纹理过滤优化** (4项)

- 各向异性采样优化 (新增 `0x00E73211`)
- 纹理过滤负LOD偏移 (新增 `0x0019BB68`)
- 驱动控制LOD偏移 (新增 `0x00638E8F`)
- 透明度多重采样 (新增 `0x10FC2D9C`)

**组4: 电源与性能** (3项)

- 电源管理模式 (已有)
- 最大预渲染帧数 (新增 `0x007BA09E`)
- OpenGL 线程优化 (新增 `0x20C1221E`)

## 技术方案

### 技术栈

- 后端: Rust (Tauri) + NVAPI DRS API (静态链接 nvapi64.lib)
- 前端: React + TypeScript + Chakra UI + lucide-react

### 实现策略

采用**增量添加**策略：在现有的 `nvapi.rs` 和 `NvidiaDriverPage.tsx` 中追加新设置的定义和分组，不改变现有架构。

### 修改详单

#### 后端 `src-tauri/src/nvapi.rs`

**1. 添加 15 个新常量**（在 `SETTING_FXAA_ENABLE` 之后）

```rust
const SETTING_VRR_APP_OVERRIDE: NvU32 = 0x10A879CF;
const SETTING_VRR_MODE: NvU32 = 0x1194F158;
const SETTING_REFRESH_RATE_OVERRIDE: NvU32 = 0x0064B541;
const SETTING_VSYNCTEARCONTROL: NvU32 = 0x005A375C;
const SETTING_AO_MODE: NvU32 = 0x00667329;
const SETTING_MFAA: NvU32 = 0x0098C1AC;
const SETTING_SHADER_CACHE: NvU32 = 0x00198FFF;
const SETTING_AA_MODE_SELECTOR: NvU32 = 0x107EFC5B;
const SETTING_AA_ALPHATOCOVERAGE: NvU32 = 0x10FC2D9C;
const SETTING_ANISO_OPT: NvU32 = 0x00E73211;
const SETTING_NO_NEG_LODBIAS: NvU32 = 0x0019BB68;
const SETTING_AUTO_LODBIAS: NvU32 = 0x00638E8F;
const SETTING_PRERENDERLIMIT: NvU32 = 0x007BA09E;
const SETTING_OGL_THREAD_CTRL: NvU32 = 0x20C1221E;
const SETTING_OGL_TRIPLE_BUFFER: NvU32 = 0x20FDD1F9;
```

**2. 扩展 `TARGET_SETTINGS` 数组**（从 7 项到 22 项）

**3. 在 `settings_options()` 中添加各新设置的选项值**（数值从 `NvApiDriverSettings.h` 提取）:

- `VRR_APP_OVERRIDE`: 允许=0, 强制关闭=1, 禁止=2, ULMB=3, 固定刷新=4
- `VRR_MODE`: 关闭=0, 仅全屏=1, 全屏+窗口=2
- `REFRESH_RATE_OVERRIDE`: 应用控制=0, 最高可用=1
- `VSYNCTEARCONTROL`: 禁用=0x96861077, 启用=0x99941284
- `AO_MODE`: 关=0, 低=1, 中=2, 高=3
- `MFAA`: 关=0, 开=1
- `SHADER_CACHE`: 关=0, 开=1
- `AA_MODE_SELECTOR`: 应用控制=0, 覆盖=1, 增强=2
- `AA_ALPHATOCOVERAGE`: 关=0x00000000, 开=0x00000004
- `ANISO_OPT`: 关=0, 开=1
- `NO_NEG_LODBIAS`: 关=0, 开=1
- `AUTO_LODBIAS`: 关=0, 开=1
- `PRERENDERLIMIT`: 应用控制=0, 1, 2, 3, 4 (Slider)
- `OGL_THREAD_CTRL`: 默认=0, 启用=1, 禁用=2
- `OGL_TRIPLE_BUFFER`: 关=0, 开=1

**4. 在 `list_nvidia_settings()` 的 description match 中添加新设置的中文描述**

#### 前端 `src/pages/NvidiaDriverPage.tsx`

**1. 在 `SETTING_IDS` 映射中添加 15 个新 ID**

**2. 替换现有 3 个分组为 5 个分组**:

```
renderSettingGroup("同步与显示", <Monitor .../>, [...])
renderSettingGroup("画质与纹理", <Settings2 .../>, [...])
renderSettingGroup("纹理过滤优化", <Sliders .../>, [...])  // 新增图标
renderSettingGroup("电源与性能", <Cpu .../>, [...])
```

**3. 在 `renderSettingControl` 中添加特殊控件处理**:

- PRERENDERLIMIT: Slider (0-4)
- 所有开关类 (0/1): Switch
- 多选项: CustomSelect

#### 前端导入

- 添加 `Sliders` 图标从 `lucide-react`（用于"纹理过滤优化"分组）

### 性能考虑

- TARGET_SETTINGS 从 7 项增加到 22 项，单次调用 O(n)，可忽略
- 前端分组过滤 O(n)，21 项无压
- 后端对不支持的设置返回错误并 `continue` 跳过

### 错误处理

- NvAPI_DRS_GetSetting 失败时 `continue` 跳过，分组内不显示不支持的项目
- renderSettingGroup 对空结果返回 null，不会破坏 UI 布局

## Agent Extensions

### SubAgent

- **code-explorer**: 用于探索代码库中的现有设置结构、SDK 头文件中的选项值、以及 UI 渲染逻辑
- **Expected outcome**: 确认所有 15 个新设置的 ID 和选项值，验证前后端代码结构
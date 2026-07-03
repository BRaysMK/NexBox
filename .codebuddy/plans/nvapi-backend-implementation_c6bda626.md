---
name: nvapi-backend-implementation
overview: 为内置工具页面的显卡设置实现 NVAPI 后端，通过动态加载 nvapi64.dll 调用 NVIDIA DRS（驱动设置）API，支持纹理质量、垂直同步、抗锯齿、帧率限制等 7 项 3D 设置的读取、修改和恢复默认值。
todos:
  - id: create-nvapi-module
    content: 在 src-tauri/src/ 下创建 nvapi.rs 模块，实现 NVAPI FFI 类型定义、函数指针声明、动态 DLL 加载、全局会话管理和 6 个 Tauri 命令
    status: completed
  - id: register-backend
    content: 在 lib.rs 中注册 nvapi 模块（mod 声明 + invoke_handler 6 条命令 + cleanup 钩子）
    status: completed
    dependencies:
      - create-nvapi-module
  - id: uncomment-frontend
    content: 解除 BuiltinToolsPage.tsx 中 nvidia-driver 条目的注释，使显卡设置入口可见
    status: completed
    dependencies:
      - create-nvapi-module
  - id: build-and-verify
    content: 编译项目并验证 NVAPI 后端与前端联调正常
    status: completed
    dependencies:
      - register-backend
      - uncomment-frontend
---

## 用户需求

为内置工具页面的显卡设置功能实现 NVAPI 后端，通过 NVAPI DRS（Driver Settings）接口实现以下 NVIDIA 显卡 3D 设置的可读写操作：

### 可调节的 7 项核心设置

| 设置名称 | 设置 ID | 说明 |
| --- | --- | --- |
| 垂直同步 (VSync) | 0x00A879CF | 被动/强制关闭/强制开启/半刷新率等 7 种模式 |
| 纹理过滤 - 质量 | 0x00CE2691 | 高质量/质量/性能/高性能 4 种级别 |
| 各向异性过滤 | 0x101E61A9 | 关闭/2x/4x/8x/16x |
| 抗锯齿 - 模式 | 0x10D773D2 | 无/2x/4x/8x MSAA/SGSSAA 等多种模式 |
| 帧率限制 (FPS) | 0x10835002 | 0(关闭) ~ 255+ FPS |
| 电源管理模式 | 0x1057EB71 | 自适应/最高性能/驱动程序控制/稳定性能/最低功耗/最佳功耗 |
| FXAA | 0x1074C972 | 开/关 |


### 需要实现的 6 个 Tauri 后端命令

- `get_nvapi_status` — 检测 NVAPI 是否可用（DLL 加载 + 初始化状态）
- `diagnose_nvapi` — 详细诊断（DLL 路径、导出函数列表、候选 DLL 枚举等）
- `get_nvidia_driver_version` — 获取 NVIDIA 驱动版本号和分支信息
- `list_nvidia_settings` — 列出全局 3D 设置：每个设置包含 ID、名称、当前值、默认值、可选值列表
- `set_nvidia_setting` — 修改单个设置（传入 settingId 和 value）
- `reset_nvidia_settings` — 恢复所有全局 3D 设置为驱动默认值

### 前端入口恢复

解除 `BuiltinToolsPage.tsx` 中 `nvidia-driver` 条目的注释，使显卡设置卡片在内置工具页面中可见。

## 技术栈

- **语言**: Rust（Tauri 后端）
- **FFI**: `windows-sys` crate（`Win32_System_LibraryLoader`）动态加载 `nvapi64.dll`
- **NVAPI SDK**: R560-developer 头文件（nvapi.h、NvApiDriverSettings.h）作为 C 结构体和枚举的参考
- **已有依赖复用**: `serde`（序列化）、`log`（日志）、`thiserror`（错误处理）

## 实现方案

### 整体策略

通过 `windows-sys` 的 `LoadLibraryW` / `GetProcAddress` 动态加载系统目录下的 `nvapi64.dll`，手动声明 NVAPI 函数指针类型并解析符号，构建一个类型安全的 Rust 封装层。采用「懒初始化 + 全局单例」模式管理 NVAPI 会话生命周期。

### 核心架构决策

1. **动态加载而非静态链接**：NVAPI 是 NVIDIA 驱动提供的 DLL，不存在 `.lib` 导入库，只能运行时动态加载。项目已有 `Win32_System_LibraryLoader` feature，无需新增依赖。
2. **全局单例会话管理**：使用 `std::sync::Mutex<Option<NvapiSession>>` 在首次调用时初始化 NVAPI + DRS Session，应用退出时在 `lib.rs` 的 `RunEvent::Exit` 中清理，避免每次调用都重复 Init/Unload。
3. **设置值与选项的硬编码映射**：NVIDIA DRS 设置的值是特殊的十六进制 magic number（如 VSync 的 `0x60925292` 表示被动模式），无法从 API 动态获取枚举定义。因此直接在 Rust 中按照 `NvApiDriverSettings.h` 的枚举定义硬编码所有选项映射表。

### 性能考虑

- 首次调用 `get_nvapi_status` 时完成 DLL 加载和 NVAPI 初始化（~10-50ms），后续调用复用已初始化的会话
- `list_nvidia_settings` 一次性枚举所有设置（7 项），避免 N+1 查询
- `set_nvidia_setting` 每次修改后立即 `SaveSettings` 持久化，确保设置不丢失

### 错误处理

- 所有 NVAPI 调用返回 `NvAPI_Status`，通过 `NvAPI_GetErrorMessage` 转换为可读字符串
- 前端接口返回 `Result<T, String>`，错误信息直接透传到 UI 的 toast 提示中
- 诊断命令 `diagnose_nvapi` 即使失败也返回完整诊断数据（不抛错），方便前端展示排查信息

## 实现细节

### NVAPI 函数指针声明（主要）

```rust
type NvAPI_Initialize_t = unsafe extern "C" fn() -> i32;
type NvAPI_Unload_t = unsafe extern "C" fn() -> i32;
type NvAPI_GetErrorMessage_t = unsafe extern "C" fn(i32, *mut u16) -> i32;
type NvAPI_DRS_CreateSession_t = unsafe extern "C" fn(*mut *mut std::ffi::c_void) -> i32;
type NvAPI_DRS_DestroySession_t = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
type NvAPI_DRS_LoadSettings_t = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
type NvAPI_DRS_SaveSettings_t = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
type NvAPI_DRS_GetBaseProfile_t = unsafe extern "C" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32;
type NvAPI_DRS_EnumSettings_t = unsafe extern "C" fn(*mut std::ffi::c_void, u32, *mut u32, *mut NVDRS_SETTING_V1) -> i32;
type NvAPI_DRS_GetSetting_t = unsafe extern "C" fn(*mut std::ffi::c_void, u32, *mut NVDRS_SETTING_V1) -> i32;
type NvAPI_DRS_SetSetting_t = unsafe extern "C" fn(*mut std::ffi::c_void, u32, u32) -> i32;
type NvAPI_DRS_RestoreProfileDefault_t = unsafe extern "C" fn(*mut std::ffi::c_void, u32) -> i32;
type NvAPI_SYS_GetDriverAndBranchVersion_t = unsafe extern "C" fn(*mut u32, *mut u8) -> i32;
```

### 设置选项映射策略

每个设置维护一个 `&[(u32, &str)]` 的 options 常量数组：

- `VSYNCMODE_OPTIONS`: [PASSIVE(0x60925292, "被动"), FORCEOFF(0x08416747, "关闭"), FORCEON(0x47814940, "开启"), FLIPINTERVAL2("半刷新率"), ...]
- `QUALITY_ENHANCEMENTS_OPTIONS`: [HIGHQUALITY(0xfffffff6, "高质量"), QUALITY(0, "质量"), PERFORMANCE(10, "性能"), HIGHPERFORMANCE(20, "高性能")]
- `ANISO_OPTIONS`: [NONE(0, "关闭"), LINEAR(1, "线性"), 2x(2), 4x(4), 8x(8), 16x(16)]
- `AA_OPTIONS`: 常用选项 NONE(0), MSAA_2x(0x0E), MSAA_4x(0x10), MSAA_8x(0x25), SGSSAA_2x(0x20), SGSSAA_4x(0x22)
- `PSTATE_OPTIONS`: ADAPTIVE(0), PREFER_MAX(1), DRIVER_CONTROLLED(2), CONSISTENT(3), PREFER_MIN(4), OPTIMAL_POWER(5)
- `FXAA_OPTIONS`: OFF(0), ON(1)

### 诊断命令实现

`diagnose_nvapi` 命令执行以下步骤并返回完整诊断报告：

1. 枚举系统中所有 `nvapi64.dll` 候选路径（System32、DriverStore、NVIDIA 安装目录等）
2. 对每个候选文件读取文件属性（大小、公司名、产品名、版本）
3. 通过 `LoadLibraryW` 尝试加载并检查导出函数（`NvAPI_Initialize` 是否存在）
4. 尝试调用 `NvAPI_Initialize` 验证可用性
5. 汇总结论和建议（驱动未安装/版本过旧/库损坏等）

### Blast Radius 控制

- 新增 `nvapi.rs` 模块完全独立，不影响任何现有模块
- 仅在 `lib.rs` 中新增 `mod nvapi` 声明和 6 行 `invoke_handler` 注册
- 仅在 `BuiltinToolsPage.tsx` 中解除 8 行注释
- 所有 NVAPI 调用均通过 `#[cfg(target_os = "windows")]` 条件编译保护

## 目录结构

```
d:\NexBox\src-tauri\src\
├── nvapi.rs                          # [NEW] NVAPI 后端核心模块
│   ├── 类型定义：NvAPI_Status, NVDRS_SETTING_V1 等 FFI 结构体
│   ├── 函数指针类型：NVAPI DRS 系列函数的 Rust 签名
│   ├── 全局会话管理：NvapiSession 单例（Mutex 保护）
│   ├── DLL 加载与符号解析：load_nvapi_dll() 函数
│   ├── 设置选项常量表：7 个设置的 options 映射数组
│   ├── 6 个 Tauri 命令实现：get_nvapi_status, diagnose_nvapi,
│   │   get_nvidia_driver_version, list_nvidia_settings,
│   │   set_nvidia_setting, reset_nvidia_settings
│   └── cleanup 函数：应用退出时释放 NVAPI 资源
├── lib.rs                            # [MODIFY] 新增 mod nvapi + 注册 6 个命令
│   └── invoke_handler 中新增 nvapi::get_nvapi_status 等 6 行
│   └── RunEvent::Exit 中调用 nvapi::cleanup()
d:\NexBox\src\pages\
└── BuiltinToolsPage.tsx              # [MODIFY] 解除 nvidia-driver 条目的注释
    └── 第 84-91 行：取消注释，添加 Settings2 icon 引用
```

## 关键代码结构

### NvapiSession 结构

```rust
pub struct NvapiSession {
    dll_handle: *mut std::ffi::c_void,
    drs_session: *mut std::ffi::c_void,
    initialized: bool,
    // 缓存的函数指针...
    fn_initialize: NvAPI_Initialize_t,
    fn_unload: NvAPI_Unload_t,
    fn_create_session: NvAPI_DRS_CreateSession_t,
    // ... 其他函数指针
}
```

### 设置项返回结构

```rust
#[derive(Serialize)]
pub struct NvidiaSetting {
    pub id: u32,           // 设置 ID（如 0x00A879CF）
    pub name: String,      // 显示名称（如 "垂直同步"）
    pub description: String,
    pub current_value: u32,
    pub default_value: u32,
    pub options: Vec<SettingOption>,
}

#[derive(Serialize)]
pub struct SettingOption {
    pub value: u32,
    pub label: String,
}
```
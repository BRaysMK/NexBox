# NexBox 悬浮框 FPS 切游戏后失效修复计划

> **文档版本**: v1.0
> **创建日期**: 2026-07-19
> **问题类型**: Bug 修复 — FPS 监控在切出/切入游戏后失效
> **核心症状**: 切出游戏再切回来后 FPS 显示 1 左右，必须按 Win 键再返回游戏窗口才能恢复

---

## 一、问题现象

### 1.1 复现路径

1. 启动悬浮框（Win32 overlay），进入游戏（尤其是独占全屏 / exclusive fullscreen）
2. FPS 获取正常（显示真实帧率）
3. **切出游戏**（Alt+Tab、点桌面、呼出其它窗口）
4. **切回游戏**（Alt+Tab 回去、或点击游戏窗口）
5. FPS 降至 **1 左右**，且长时间不再恢复
6. 必须按下 **Win 键**（弹出开始菜单），再点回游戏窗口，FPS 才恢复正常

### 1.2 关键观察

| 观察 | 含义 |
|------|------|
| 大部分时间获取正常 | 说明 ETW 会话、Provider 启用、帧事件匹配逻辑本身是 OK 的 |
| 切出再切回后才失效 | 与"前台窗口切换"路径强相关 |
| FPS 显示 1（而非 0） | `fps_counter_loop` 里 `raw_count≈0`，经 `0.3*cur+0.7*old` 平滑后衰减到 1 |
| 按 Win 键再回来才恢复 | Win 键 + 点击产生的是**标准前台变更**，而 Alt+Tab 切回独占全屏游戏的变更不可靠 |

---

## 二、现有 FPS 监控架构分析

### 2.1 涉及文件

| 文件 | 作用 |
|------|------|
| `src-tauri/src/game_fps.rs` | FPS 监控核心：ETW 会话、前台 Hook、帧计数、平滑 |
| `src-tauri/src/overlay_panel.rs` | 悬浮框主逻辑：启动 overlay 线程、消息循环、采集硬件数据 |

### 2.2 数据流总览

```
┌──────────────────────────────────────────────────────────────────┐
│  start_overlay() (overlay_panel.rs:1687)                        │
│    └─ thread::spawn {                                            │
│         start_fps_monitor()  ───────────────┐                    │
│         创建 overlay 窗口 + 消息循环          │                    │
│         (PeekMessage/DispatchMessage 50ms)   │                    │
│    }                                         │                    │
│                                              ▼                    │
│  game_fps::start_fps_monitor():                                   │
│    1. get_initial_foreground_pid()  → CURRENT_PID 初始化          │
│    2. register_foreground_hook()    → SetWinEventHook            │
│       (EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT)            │
│    3. thread::spawn { start_etw_trace() }  → ProcessTrace 阻塞   │
│    4. thread::spawn { fps_counter_loop() } → 每 500ms 计算一次    │
└──────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────────────┐
│  前台窗口变化时                                                      │
│  on_foreground_changed(hwnd):                                     │
│    pid = GetWindowThreadProcessId(hwnd)                           │
│    CURRENT_PID = pid         ← 关键！                            │
│    CURRENT_HWND = hwnd       ← 关键！                            │
│    重置 DXGI/DWM 帧计数器                                          │
└──────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────────────┐
│  ETW 事件回调 on_etw_event(event_record):                          │
│    target_pid = CURRENT_PID                                       │
│    if pid == target_pid && (DXGI Present || D3D9 Present):       │
│        DXGI_FRAME_COUNTER++                                       │
│    if ProviderId == DWM && UserData 包含 target_hwnd/同 pid 窗口: │
│        DWM_FRAME_COUNTER++                                        │
└──────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────────────┐
│  fps_counter_loop()（每 500ms）:                                    │
│    dxgi = DXGI_FRAME_COUNTER.swap(0)                              │
│    dwm  = DWM_FRAME_COUNTER.swap(0)                               │
│    raw  = dxgi>0 ? dxgi : dwm                                     │
│    fps  = raw * 2.0                                               │
│    smoothed = 0.3*fps + 0.7*smoothed                              │
│    SMOOTHED_FPS = smoothed                                        │
└──────────────────────────────────────────────────────────────────┘
```

### 2.3 关键代码定位

**前台 Hook 注册**（`game_fps.rs:193-211`）：
```rust
pub unsafe fn register_foreground_hook() -> bool {
    let hook = SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND,
        EVENT_SYSTEM_FOREGROUND,
        ptr::null_mut(),
        Some(on_foreground_changed),
        0, 0,
        WINEVENT_OUTOFCONTEXT,
    );
    ...
}
```

**前台变更回调**（`game_fps.rs:60-91`）：
```rust
unsafe extern "system" fn on_foreground_changed(
    _hook, _event, hwnd: HWND, _id_object, _id_child, _id_event_thread, _dw_event_time,
) {
    if !FPS_ACTIVE.load(Ordering::SeqCst) { return; }
    // 跳过 overlay 自身
    let overlay_hwnd = OVERLAY_HWND.load(Ordering::Relaxed) as usize;
    if overlay_hwnd != 0 && hwnd as usize == overlay_hwnd { return; }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid != 0 {
        CURRENT_PID.store(pid, Ordering::Relaxed);      // ← 唯一更新点
        CURRENT_HWND.store(hwnd as u64, Ordering::Relaxed);
        DXGI_FRAME_COUNTER.store(0, ...);
        DWM_FRAME_COUNTER.store(0, ...);
        ... // 重置计数器
    }
}
```

**ETW 事件过滤**（`game_fps.rs:119-191`）：
```rust
unsafe extern "system" fn on_etw_event(event_record: *mut EVENT_RECORD) {
    let target_pid = CURRENT_PID.load(Ordering::Relaxed);
    if target_pid == 0 { return; }
    let pid = record.EventHeader.ProcessId;
    if pid == target_pid && (dxgi_present || d3d9_present) {
        DXGI_FRAME_COUNTER.fetch_add(1, ...);   // ← 仅当 pid 匹配才计数
    }
    if provider == DWM && UserData 含 target_hwnd/同 pid 窗口 {
        DWM_FRAME_COUNTER.fetch_add(1, ...);
    }
}
```

**FPS 计算**（`game_fps.rs:374-418`）：
```rust
fn fps_counter_loop() {
    let mut smoothed: f64 = -1.0;
    while FPS_ACTIVE.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(500));
        let dxgi_count = DXGI_FRAME_COUNTER.swap(0, Ordering::Relaxed);
        let dwm_count  = DWM_FRAME_COUNTER.swap(0, Ordering::Relaxed);
        let raw_count = if dxgi_count > 0 { dxgi_count } else { dwm_count };
        let current_fps = raw_count as f64 * 2.0;
        smoothed = 0.3 * current_fps + 0.7 * smoothed;  // 衰减平滑
        SMOOTHED_FPS.store(smoothed.round() as u32, Ordering::Relaxed);
    }
}
```

---

## 三、根本原因深度分析

### 3.1 核心结论

> **`CURRENT_PID` / `CURRENT_HWND` 在"切回独占全屏游戏"时未被正确更新，导致 ETW 事件过滤全部失配，帧计数器恒为 0，经平滑后表现为 FPS≈1。**

整个 FPS 系统对目标进程的识别**完全依赖 `EVENT_SYSTEM_FOREGROUND` 这一条 WinEvent Hook**。一旦该 Hook 在某次切换中漏发，`CURRENT_PID` 就会"卡"在错误的旧值，FPS 立刻归零。

### 3.2 为什么 Alt+Tab 切回独占全屏游戏时 Hook 会漏发？

这是本 Bug 最关键的部分，需结合 Windows 窗口管理机制深入理解：

#### (1) 独占全屏（Exclusive Fullscreen）的特殊性

独占全屏游戏绕过 DWM 直接接管显示输出。当 Alt+Tab 切出时：
- Windows 强制游戏退出独占模式，桌面/合成器（DWM）接管
- 前台窗口变更为桌面窗口 → `EVENT_SYSTEM_FOREGROUND` 正常触发
- `CURRENT_PID` 被更新为桌面/explorer 的 PID（此时是"正确"的）

当 Alt+Tab 切回游戏时：
- 游戏需要**重新获取独占全屏**（display mode switch）
- 这个 mode switch 走的是 **DirectX/Graphics Kernel 的特殊路径**，而非标准窗口管理路径
- `EVENT_SYSTEM_FOREGROUND` 在此路径下**可能被抑制、合并或以瞬态窗口句柄上报**

#### (2) Alt+Tab 切换器的中间态

Alt+Tab 会短暂显示切换器窗口（属于 `explorer.exe` / `dwm.exe`）：
1. 切换器窗口成为前台 → Hook 触发，`CURRENT_PID` = explorer/dwm
2. 松开 Alt 键，游戏"接管"独占全屏 → **此步 Hook 可能不触发**
3. 结果：`CURRENT_PID` 卡在 explorer/dwm 的 PID

#### (3) 为什么"按 Win 键再点回来"能恢复？

按 Win 键：
1. 开始菜单成为前台 → **标准前台变更**，Hook 必触发 → `CURRENT_PID` = explorer
2. 点击游戏窗口 → **标准前台变更**，Hook 必触发 → `CURRENT_PID` = 游戏的 PID
3. 此时 DXGI Present 事件的 `pid` 与 `target_pid` 匹配 → 帧计数恢复

**关键差异**：Win 键 + 点击走的是"普通窗口前台切换"路径，事件可靠；Alt+Tab 切回独占全屏走的是"显示模式切换"路径，事件不可靠。

### 3.3 为什么 FPS 显示"1"而不是"0"？

看平滑公式：
```rust
smoothed = 0.3 * current_fps + 0.7 * smoothed;
```
- 切回后 `raw_count=0` → `current_fps=0`
- 但 `smoothed` 此前是真实 FPS（如 120）
- 衰减序列：120 → 0.7×120=84 → 0.7×84=58.8 → 41 → 28.8 → 20 → 14 → 10 → 7 → 5 → 3.5 → 2.4 → 1.7 → **1**
- 每 500ms 一步，约 **5~7 秒**衰减到 1 并"卡"在那里
- 这与用户"显示 1 左右、不再变化"的描述完全吻合

### 3.4 加剧问题的设计缺陷

| 缺陷 | 说明 |
|------|------|
| **无兜底轮询** | 完全依赖单条 Hook，无 `GetForegroundWindow()` 定期同步，Hook 漏发即永久失效 |
| **无自愈机制** | `fps_counter_loop` 检测到低 FPS 也不触发重新查询前台窗口 |
| **无进程存活校验** | 不校验 `CURRENT_PID` 对应进程是否还在运行 / 是否还是前台 |
| **单事件类型 Hook** | 只监听 `EVENT_SYSTEM_FOREGROUND`，未监听 `EVENT_SYSTEM_SWITCHEND`（Alt+Tab 结束）等 |
| **独占全屏过渡期未处理** | mode switch 期间 HWND 可能变化，无重查机制 |
| **消息泵阻塞风险** | overlay 线程消息循环用 `PeekMessage`+`sleep(50)`，重载下可能延迟 Hook 回调派发 |

### 3.5 排除其他假设

| 假设 | 结论 |
|------|------|
| ETW 会话崩溃 | ❌ 排除。`ProcessTrace` 仍在阻塞运行，`ETW_STARTED=true`；若是会话挂掉则"大部分时间正常"不成立 |
| Provider 被禁用 | ❌ 排除。Provider 启用是一次性的，不会因切窗而失效 |
| DXGI Present 事件 ID 错误 | ❌ 排除。事件 ID=42 是固定的，且"正常时能获取"说明匹配正确 |
| 游戏 PID 变化 | ❌ 基本排除。游戏进程通常不会因 Alt+Tab 重启 |
| overlay 窗口被误识别为前台 | ❌ 已有 `OVERLAY_HWND` 跳过逻辑 |
| 消息泵完全不工作 | ❌ 排除。"大部分时间正常"说明 Hook 回调能被派发 |

---

## 四、解决方案

采用**三层防御**组合策略，任一层即可兜底，组合后近乎不可能再卡死。

### 4.1 方案总览

| 层 | 手段 | 目的 | 代价 |
|----|------|------|------|
| **L1 主动同步** | `fps_counter_loop` 每 ~1s 轮询 `GetForegroundWindow()` 与 `CURRENT_HWND` 比对 | 兜底所有 Hook 漏发场景 | 极低（1 次 Win32 调用/s） |
| **L2 自愈恢复** | 连续 N tick（≈1.5s）FPS≤2 时强制重查前台窗口 | 精准命中"卡在 1"的病态 | 几乎为零 |
| **L3 事件补全** | 额外注册 `EVENT_SYSTEM_SWITCHEND` | 捕获 Alt+Tab 结束时刻 | 1 个额外 Hook |

### 4.2 方案 L1：前台窗口轮询兜底

在 `game_fps.rs` 的 `win32_fps` 模块内新增同步函数，并在 `fps_counter_loop` 定期调用。

**新增函数**：
```rust
/// 主动查询前台窗口并同步 CURRENT_PID/CURRENT_HWND。
/// 当传入 force=true 时无视一致性直接刷新；
/// 否则仅在发现前台窗口与当前记录不一致时刷新。
pub unsafe fn sync_foreground_if_stale(force: bool) -> bool {
    let fg = GetForegroundWindow();
    if fg.is_null() { return false; }

    // 跳过 overlay 自身
    let overlay_hwnd = OVERLAY_HWND.load(Ordering::Relaxed) as usize;
    if overlay_hwnd != 0 && fg as usize == overlay_hwnd { return false; }

    let cur_hwnd = CURRENT_HWND.load(Ordering::Relaxed) as usize;
    if !force && fg as usize == cur_hwnd { return false; }

    let mut pid = 0u32;
    GetWindowThreadProcessId(fg, &mut pid);
    if pid == 0 { return false; }

    let cur_pid = CURRENT_PID.load(Ordering::Relaxed);
    if !force && pid == cur_pid { return false; }

    // 真正发生变化，执行与 on_foreground_changed 相同的刷新
    log::info!(
        "FPS监控(轮询): 前台变化 detected hwnd={:#X} pid={} (旧 pid={} hwnd={:#X})",
        fg as usize, pid, cur_pid, cur_hwnd
    );
    CURRENT_PID.store(pid, Ordering::Relaxed);
    CURRENT_HWND.store(fg as u64, Ordering::Relaxed);
    DXGI_FRAME_COUNTER.store(0, Ordering::Relaxed);
    DWM_FRAME_COUNTER.store(0, Ordering::Relaxed);
    DXGI_PID_HITS.store(0, Ordering::Relaxed);
    DWM_PID_HITS.store(0, Ordering::Relaxed);
    DWM_TOTAL_EVENTS.store(0, Ordering::Relaxed);
    TOTAL_EVENTS.store(0, Ordering::Relaxed);
    LAST_DWM_TS.store(0, Ordering::Relaxed);
    DWM_SAMPLE_LOGGED.store(false, Ordering::Relaxed);
    true
}
```

> 注意：为避免 `on_foreground_changed` 与轮询刷新逻辑重复，建议把刷新体抽成一个 `refresh_target(hwnd, pid)` 内部函数，两处共用。

**`fps_counter_loop` 集成**（伪代码，对照原 374-418 行）：
```rust
fn fps_counter_loop() {
    let mut smoothed: f64 = -1.0;
    let mut tick_count: u32 = 0;
    let mut low_fps_streak: u32 = 0;          // L2 用
    const LOW_FPS_THRESHOLD: u32 = 2;        // ≤2 视为异常
    const LOW_FPS_TRIGGER_TICKS: u32 = 3;    // 连续 3 tick(1.5s) 触发自愈
    const SYNC_EVERY_TICKS: u32 = 2;         // L1 每 1s 轮询一次

    while FPS_ACTIVE.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(500));
        if !FPS_ACTIVE.load(Ordering::SeqCst) { break; }

        // —— L1: 每 2 tick(≈1s) 主动同步前台窗口 ——
        #[cfg(target_os = "windows")]
        unsafe {
            if tick_count > 0 && tick_count % SYNC_EVERY_TICKS == 0 {
                let _ = win32_fps::sync_foreground_if_stale(false);
            }
        }

        // —— 原有帧计数采集 ——
        let dxgi_count = DXGI_FRAME_COUNTER.swap(0, Ordering::Relaxed);
        let dwm_count  = DWM_FRAME_COUNTER.swap(0, Ordering::Relaxed);
        ...
        let raw_count = if dxgi_count > 0 { dxgi_count } else { dwm_count };
        let current_fps = raw_count as f64 * 2.0;
        smoothed = 0.3 * current_fps + 0.7 * smoothed.max(0.0);
        ...

        // —— L2: 低 FPS 自愈 ——
        #[cfg(target_os = "windows")]
        unsafe {
            if current_fps <= LOW_FPS_THRESHOLD as f64 {
                low_fps_streak += 1;
                if low_fps_streak >= LOW_FPS_TRIGGER_TICKS {
                    log::warn!("FPS监控: 连续 {} tick 低帧率({})，触发前台重查",
                        low_fps_streak, current_fps);
                    let changed = win32_fps::sync_foreground_if_stale(true);
                    if changed {
                        // 重置平滑值，让新目标立即生效，避免历史平滑拖累
                        smoothed = -1.0;
                    }
                    low_fps_streak = 0;
                }
            } else {
                low_fps_streak = 0;
            }
        }

        CURRENT_FPS.store(current_fps.round() as u32, Ordering::Relaxed);
        SMOOTHED_FPS.store(smoothed.round() as u32, Ordering::Relaxed);
        tick_count += 1;
    }
}
```

### 4.3 方案 L2：低 FPS 自愈（已并入 L1 伪代码）

逻辑要点：
- 仅当 `current_fps ≤ 2` **且** `ETW_STARTED=true`（确认 ETW 没崩）时才计入"低帧连击"
- 连击达阈值后调用 `sync_foreground_if_stale(true)`（强制刷新）
- 刷新成功则 `smoothed = -1.0`（下次循环会用新 raw 值初始化，避免被旧平滑值拖住）
- 连击计数重置

> 这样即使 L1 轮询间隔内发生漏发，最多 1.5s 内也能自愈，用户体验上最多看到"1"闪一下即恢复。

### 4.4 方案 L3：补充 WinEvent 事件类型

在 `register_foreground_hook` 之外，额外注册一个覆盖 `EVENT_SYSTEM_SWITCHSTART`/`EVENT_SYSTEM_SWITCHEND` 的 Hook，专门感知 Alt+Tab 交互的开始与结束：

```rust
const EVENT_SYSTEM_SWITCHSTART: u32 = 0x0014;
const EVENT_SYSTEM_SWITCHEND:   u32 = 0x0015;

pub unsafe fn register_switch_hook() -> bool {
    let hook = SetWinEventHook(
        EVENT_SYSTEM_SWITCHEND,
        EVENT_SYSTEM_SWITCHEND,
        ptr::null_mut(),
        Some(on_switch_end),
        0, 0,
        WINEVENT_OUTOFCONTEXT,
    );
    if !hook.is_null() {
        let mut lock = SWITCH_HOOK_HANDLE.lock().unwrap();
        *lock = hook as usize;
        true
    } else {
        log::warn!("FPS监控: SWITCHEND Hook 注册失败");
        false
    }
}

unsafe extern "system" fn on_switch_end(
    _hook, _event, _hwnd, _id_object, _id_child, _id_event_thread, _dw_event_time,
) {
    if !FPS_ACTIVE.load(Ordering::SeqCst) { return; }
    // Alt+Tab 切换结束，延迟 150ms 等独占全屏 mode switch 完成后重查前台
    let h = thread::spawn(|| {
        thread::sleep(Duration::from_millis(150));
        if FPS_ACTIVE.load(Ordering::SeqCst) {
            unsafe { let _ = sync_foreground_if_stale(true); }
        }
    });
    let _ = h.join();
}
```

> 延迟 150ms 是为了等独占全屏完成 mode switch，避免在过渡期拿到瞬态窗口句柄。
> 注意 `on_switch_end` 是在 overlay 线程消息循环里被派发的，若用 `thread::spawn`+`join` 会阻塞消息循环；实现时建议改成"记一个时间戳，由 `fps_counter_loop` 在下一个 tick 执行重查"，或用 detached 线程 + atomic 标志。

**更优实现（非阻塞）**：
```rust
static PENDING_RESYNC: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn on_switch_end(...) {
    if !FPS_ACTIVE.load(Ordering::SeqCst) { return; }
    PENDING_RESYNC.store(true, Ordering::Relaxed);
}
// fps_counter_loop 开头检测：
// if PENDING_RESYNC.swap(false, Relaxed) { sync_foreground_if_stale(true); }
```

### 4.5 方案 L4（可选增强）：进程存活与 ETW 健康校验

在 L1 轮询里附加：
- `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, CURRENT_PID)` 成功性校验，进程已退出则清零 `CURRENT_PID` 并触发重查
- `ETW_STARTED` 为 false 时尝试重建 ETW 会话（极端兜底）

此项为"锦上添花"，非本 Bug 必需，可在后续迭代加入。

### 4.6 方案 L5（可选增强）：overlay 窗口白名单优化

当前 `on_foreground_changed` 用 `OVERLAY_HWND` 跳过 overlay 自身。但**竖排 overlay（Tauri 窗口）**和 **crosshair 准星窗口**也可能短暂成为前台。建议扩展为一个"自身窗口白名单"，避免误把这些窗口当游戏目标。本 Bug 不直接由它引起，但切窗频繁时值得加固。

---

## 五、任务拆解与实施步骤

### 阶段 0：准备与基线

| # | 任务 | 产物 |
|---|------|------|
| 0.1 | 复现并录制日志基线：开启 overlay 进入独占全屏游戏，Alt+Tab 切出再切回，收集 `game_fps` 日志 | `fps-bug-baseline.log`，确认 `PID=` 字段在切回时未更新 |
| 0.2 | 确认日志中 `etw=true total=高 dxgi=0 dwm=0/低`（印证"ETW 在跑但目标失配"） | 日志片段 |

### 阶段 1：核心修复（L1 + L2）

| # | 任务 | 文件 | 验收 |
|---|------|------|------|
| 1.1 | 抽取 `refresh_target(hwnd, pid)` 内部函数，`on_foreground_changed` 与新轮询共用 | `game_fps.rs` | 编译通过 |
| 1.2 | 新增 `win32_fps::sync_foreground_if_stale(force: bool)` | `game_fps.rs` | 单测/日志验证 |
| 1.3 | `fps_counter_loop` 集成 L1 每 2 tick 轮询 | `game_fps.rs` | 日志见 `FPS监控(轮询)` |
| 1.4 | `fps_counter_loop` 集成 L2 低 FPS 自愈（连击 3 tick + 强制重查 + 重置平滑） | `game_fps.rs` | 复现路径下 FPS 1.5s 内恢复 |
| 1.5 | 全量回归：正常游戏 / 窗口化 / 独占全屏切出切入 / 多显示器 | — | 无回退 |

### 阶段 2：事件补全（L3）

| # | 任务 | 文件 | 验收 |
|---|------|------|------|
| 2.1 | 新增 `EVENT_SYSTEM_SWITCHEND` Hook + `PENDING_RESYNC` 标志 | `game_fps.rs` | Hook 注册成功 |
| 2.2 | `fps_counter_loop` 开头消费 `PENDING_RESYNC`，触发强制重查 | `game_fps.rs` | Alt+Tab 后下一 tick 即恢复 |
| 2.3 | `stop_fps_monitor` / `cleanup` 中 `unregister` 新 Hook | `game_fps.rs` | 无句柄泄漏 |
| 2.4 | 回归 Alt+Tab 快速连切场景 | — | 无误判、无抖动 |

### 阶段 3：可选增强（L4/L5）

| # | 任务 | 文件 | 验收 |
|---|------|------|------|
| 3.1 | L1 轮询中加入 `CURRENT_PID` 进程存活校验 | `game_fps.rs` | 游戏退出后能切到下一个前台目标 |
| 3.2 | ETW 崩溃自愈：`ETW_STARTED=false` 时重建会话 | `game_fps.rs` | 极端场景兜底 |
| 3.3 | 自身窗口白名单（overlay / vertical overlay / crosshair） | `game_fps.rs` | 频繁切窗无误目标 |

### 阶段 4：测试与发布

| # | 任务 | 验收 |
|---|------|------|
| 4.1 | 独占全屏游戏（如《三角洲行动》《CS2》）Alt+Tab 切出切回 20 次 | 每次 FPS ≤1.5s 内恢复 |
| 4.2 | 窗口化/无边框窗口游戏同上 | 无回退 |
| 4.3 | 多显示器 + 主副屏切换 | 目标跟随正确 |
| 4.4 | 性能分析：`GetForegroundWindow` 1 次/s 的开销 | 可忽略 |
| 4.5 | 日志噪声评估：轮询日志按需降级（变化才打印） | 日志可读 |
| 4.6 | 发版备注更新 | changelog |

---

## 六、关键代码改动清单（落地参考）

### 6.1 `game_fps.rs` 改动点

```rust
// 1. 新增静态变量
static PENDING_RESYNC: AtomicBool = AtomicBool::new(false);
static SWITCH_HOOK_HANDLE: Mutex<usize> = Mutex::new(0);

// 2. 抽取刷新逻辑
unsafe fn refresh_target(hwnd: HWND, pid: u32) {
    CURRENT_PID.store(pid, Ordering::Relaxed);
    CURRENT_HWND.store(hwnd as u64, Ordering::Relaxed);
    DXGI_FRAME_COUNTER.store(0, Ordering::Relaxed);
    DWM_FRAME_COUNTER.store(0, Ordering::Relaxed);
    DXGI_PID_HITS.store(0, Ordering::Relaxed);
    DWM_PID_HITS.store(0, Ordering::Relaxed);
    DWM_TOTAL_EVENTS.store(0, Ordering::Relaxed);
    TOTAL_EVENTS.store(0, Ordering::Relaxed);
    LAST_DWM_TS.store(0, Ordering::Relaxed);
    DWM_SAMPLE_LOGGED.store(false, Ordering::Relaxed);
}

// 3. on_foreground_changed 内部调用 refresh_target
// 4. 新增 sync_foreground_if_stale(force)
// 5. 新增 register_switch_hook / on_switch_end / unregister_switch_hook
// 6. fps_counter_loop 增加 L1/L2/PENDING_RESYNC 消费
// 7. start_fps_monitor 调用 register_switch_hook
// 8. stop_fps_monitor 调用 unregister_switch_hook
```

### 6.2 `overlay_panel.rs` 无需改动

本次修复完全在 `game_fps.rs` 内闭环，`overlay_panel.rs` 的 overlay 线程消息循环已能派发新增的 SWITCHEND Hook 回调（同为 `WINEVENT_OUTOFCONTEXT`）。

### 6.3 竖排 overlay（vertical_overlay）影响评估

竖排 overlay 走 Tauri 窗口方案，FPS 数据同样来自 `game_fps::get_cached_fps()`，本修复对其同样生效，无需额外改动。

---

## 七、风险与回滚

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 轮询误把非游戏前台当目标（如刚切到桌面） | 中 | 短暂显示桌面"0 FPS" | L1 仅在"前台 hwnd/pid 与记录不一致"时刷新；桌面本就无 DXGI Present，自然为 0，切回游戏即恢复，可接受 |
| `sync_foreground_if_stale` 与 Hook 回调竞争 | 低 | 计数器被多清一次 | 用 atomic store 即可，多清一次不影响正确性 |
| SWITCHEND 在某些系统不上报 | 低 | L3 失效 | L1+L2 兜底，L3 非必需 |
| 平滑值重置导致 FPS 跳变 | 低 | 视觉跳一下 | 仅在自愈触发时重置，属可接受的快速恢复 |
| 回滚 | — | — | 全部改动集中在 `game_fps.rs`，git revert 单文件即可 |

---

## 八、验收标准

1. **主路径**：独占全屏游戏 Alt+Tab 切出再切回，**无需任何按键**，FPS 在 1.5s 内自动恢复真实值。
2. **无回退**：正常游戏期间 FPS 数值与修复前一致，无抖动、无目标错乱。
3. **日志**：`FPS监控(轮询)` 仅在前台真正变化时打印，无刷屏。
4. **资源**：新增开销 < 0.1% CPU，无内存增长。
5. **稳定性**：连续切窗 50 次无崩溃、无 Hook 句柄泄漏。

---

## 九、附：日志诊断字段速查

修复后日志将包含如下可观测字段，便于定位：

```
FPS监控: PID=12345 hwnd=0x1A0E etw=true total=820 dxgi=60(60) dwm=0/0/0 fps=120 (tick=42)
FPS监控(轮询): 前台变化 detected hwnd=0x2B1F pid=9988 (旧 pid=12345 hwnd=0x1A0E)
FPS监控: 连续 3 tick 低帧率(0), 触发前台重查
```

- `PID` / `hwnd`：当前目标进程与窗口
- `etw`：ETW 会话是否在处理
- `total`：ETW 总事件数（高=会话健康）
- `dxgi` / `dwm`：匹配到的帧数
- `轮询` / `低帧率`：L1/L2 触发记录

---

**结论**：本 Bug 的根因是 FPS 目标识别**单点依赖** `EVENT_SYSTEM_FOREGROUND` Hook，而该 Hook 在 Alt+Tab 切回独占全屏游戏时不可靠。通过引入"定期前台轮询兜底 + 低 FPS 自愈 + SWITCHEND 事件补全"三层防御，可在不改变 ETW 采集主链路的前提下彻底消除"卡在 1"的病态，且改动收敛在 `game_fps.rs` 单文件内，风险可控、可快速回滚。

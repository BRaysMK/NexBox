# NexBox Software Introduction Video — Expanded Prompt

## Title + Style
**NexBox: 游戏玩家的专属桌面工具箱**
White-themed glassmorphism design. BG: #f5f7fb, FG: #16163a, Accent: #5865f2/#7c3aed. Glass cards with backdrop-filter blur(24px). Space Grotesk 900 headlines, DM Sans 300 body, JetBrains Mono for data.

## Rhythm Declaration
`hook-SLIDE-breathe-BUILD-breathe-CASCADE-breathe-CTA`

## Global Rules
- Every scene has 2-5 floating glass decorative shapes with slow ambient drift
- Radial accent glow (5865f2 at 12% opacity) behind content in each scene
- Subtle grid pattern (2px lines at 5% opacity) as background texture
- Blur crossfade transitions (0.6s, power2.inOut) between all scenes
- All content cards use glassmorphism: rgba(255,255,255,0.45), blur(24px), border rgba(255,255,255,0.65)
- Gradient accent line on top of each glass card

## Per-Scene Beats

### Scene 1: Opening Title (0-7s)
- **Concept**: NexBox logo emerges from frosted glass, revealing the product name and tagline
- **Mood**: Clean, confident, premium gaming
- **Depth layers**:
  - BG: 4 floating glass shapes (circles, rounded rects) at 20-40% opacity, slow drift
  - BG: Radial accent glow centered, subtle grid pattern
  - MG: NexBox logo (white version) on glass card, tagline "游戏玩家的专属桌面工具箱"
  - FG: Gradient accent bar, monospace version label "v2.0"
- **Animation**: Logo scales from 0.8→1 with power3.out. Tagline slides from y:30 with expo.out. Glass shapes drift with sine.inOut ambient. Gradient bar scaleX from 0→1.

### Scene 2: Home Page (7-14s)
- **Concept**: Dashboard-style home page with 5 feature modules
- **Mood**: Organized, inviting
- **Depth layers**:
  - BG: Floating glass shapes, radial glow
  - MG: 5 glass feature cards in a row: 今日人气, 公告, 随机一言, 快捷启动, 自定义组件
  - FG: Section label "首页", gradient accent
- **Animation**: Title slides from left with back.out. Cards stagger in from bottom with power3.out (0.08s stagger). Cards have subtle scale bounce.

### Scene 3: Hardware Monitoring (14-21s)
- **Concept**: Real-time hardware dashboard with CPU/GPU/Memory stats
- **Mood**: Technical, precise, data-rich
- **Depth layers**:
  - BG: Grid pattern enhanced, floating glass shapes
  - MG: 3 large stat cards (CPU 45%, GPU 72%, RAM 8.2/16GB) with animated counters
  - FG: Section label "硬件监控", data labels in JetBrains Mono
- **Animation**: Title slides from right. Stat cards cascade from bottom with elastic.out. Numbers count up from 0. Accent glow pulses.

### Scene 4: Built-in Tools (21-28s)
- **Concept**: Tool grid showcasing NexBox's built-in utilities
- **Mood**: Functional, organized
- **Depth layers**:
  - BG: Floating glass shapes, radial glow
  - MG: 6 glass tool cards: 准心工具, 悬浮框, 显示器滤镜, 分辨率转换器, 显卡改写, DLSS预设
  - FG: Section label "内置工具"
- **Animation**: Title slides from left with back.out. Cards appear in 2-row grid, stagger from bottom with power3.out. Each card has icon + name.

### Scene 5: System Optimization (28-35s)
- **Concept**: Performance optimization tools
- **Mood**: Powerful, efficient
- **Depth layers**:
  - BG: Floating glass shapes, radial glow
  - MG: 6 optimization cards: 存储清理, 内存清理, ACE优化, 内存限制, 着色器缓存, 电源管理
  - FG: Section label "系统优化", performance indicators
- **Animation**: Title slides from right. Cards cascade with power3.out. Performance bars animate from 0→width.

### Scene 6: Test Tools (35-42s)
- **Concept**: Gaming skill testing suite
- **Mood**: Competitive, fun
- **Depth layers**:
  - BG: Floating glass shapes, radial glow
  - MG: 5 test cards: 反应速度测试, 瞄准测试, 专注力测试, CPS测试, 舒尔特方格
  - FG: Section label "测试工具"
- **Animation**: Title slides from left. Cards stagger with back.out. Subtle pulse on each card.

### Scene 7: Gaming Zone (42-49s)
- **Concept**: P2P gaming, Delta Force zone, Epic free games
- **Mood**: Social, exciting
- **Depth layers**:
  - BG: Floating glass shapes, enhanced radial glow
  - MG: 3 large feature cards: P2P联机, 三角洲行动专区, Epic免费游戏
  - FG: Section label "游戏专区"
- **Animation**: Title slides from right with expo.out. Cards scale in from 0.9→1 with elastic.out. P2P card has connection animation.

### Scene 8: Outro/CTA (49-56s)
- **Concept**: Brand close with download call-to-action
- **Mood**: Inviting, conclusive
- **Depth layers**:
  - BG: All floating glass shapes converge, radial glow intensifies
  - MG: NexBox logo large, "立即下载" glass button, tagline
  - FG: Version info, social links
- **Animation**: Logo scales up with power3.out. CTA button pulses gently. Glass shapes converge with sine.inOut. Final fade to glass overlay.

## Recurring Motifs
- Floating glass shapes (circles, rounded rectangles) at 20-40% opacity throughout
- Gradient accent bar (#5865f2→#7c3aed) on top of glass cards
- Monospace labels for technical data
- Subtle grid pattern as background texture
- Radial accent glow behind content areas

## Negative Prompt
- No dark backgrounds
- No pure #000 or #fff
- No solid opaque cards (must use glass effect)
- No static decorative elements (everything must have ambient motion)
- No jump cuts between scenes (blur crossfade required)
- No web-sized elements (video scale: 60px+ headlines, 24px+ body)

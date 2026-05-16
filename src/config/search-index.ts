import {
  Home,
  Cpu,
  Wrench,
  Package,
  Crosshair,
  TrendingUp,
  Heart,
  Settings,
  Palette,
  Layout,
  Zap,
  Target,
  Focus,
  MousePointerClick,
  Ban,
  Grid3X3,
  Monitor,
  Download,
  Play,
  Network,
  Bot,
  Volume2,
  Trash2,
  MemoryStick,
  Gauge,
  Gamepad2,
} from "lucide-react";
import type { ComponentType } from "react";

export type SearchCategory = "page" | "builtin-tool" | "test" | "optimization" | "thirdparty-tool";

export interface SearchItem {
  id: string;
  nameKey: string;
  path: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  category: SearchCategory;
  keywords?: string[];
  customIcon?: string;
  action?: "navigate" | "run-tool";
  toolId?: string;
}

export const searchIndex: SearchItem[] = [
  {
    id: "home",
    nameKey: "sidebar.home",
    path: "/",
    icon: Home,
    category: "page",
    keywords: ["首页", "主页", "home", "main"],
  },
  {
    id: "hardware",
    nameKey: "sidebar.hardware",
    path: "/hardware",
    icon: Cpu,
    category: "page",
    keywords: ["硬件", "信息", "hardware", "cpu", "gpu", "显卡", "处理器"],
  },
  {
    id: "tools",
    nameKey: "sidebar.tools",
    path: "/tools",
    icon: Wrench,
    category: "page",
    keywords: ["工具", "tools", "工具箱"],
  },
  {
    id: "builtin-tools",
    nameKey: "sidebar.builtinTools",
    path: "/builtin-tools",
    icon: Package,
    category: "page",
    keywords: ["内置", "工具", "builtin", "tools"],
  },
  {
    id: "tests",
    nameKey: "sidebar.tests",
    path: "/tests",
    icon: Crosshair,
    category: "page",
    keywords: ["测试", "tests", "训练"],
  },
  {
    id: "optimization",
    nameKey: "sidebar.optimization",
    path: "/optimization",
    icon: TrendingUp,
    category: "page",
    keywords: ["优化", "optimization", "性能"],
  },
  {
    id: "delta-force",
    nameKey: "sidebar.deltaForce",
    path: "/delta-force",
    icon: Crosshair,
    category: "page",
    keywords: ["三角洲", "delta", "force", "密码", "改枪码"],
  },
  {
    id: "mood",
    nameKey: "sidebar.mood",
    path: "/mood",
    icon: Heart,
    category: "page",
    keywords: ["心境", "mood", "心情"],
  },
  {
    id: "epic-free",
    nameKey: "sidebar.epicFree",
    path: "/epic-free",
    icon: Gamepad2,
    category: "page",
    keywords: ["epic", "喜加一", "免费", "游戏", "free", "games", "白嫖"],
  },
  {
    id: "sponsor",
    nameKey: "settings.sponsor",
    path: "/settings",
    icon: Heart,
    category: "page",
    keywords: ["赞助", "sponsor", "支持", "捐赠", "打赏", "donate", "support"],
  },
  {
    id: "settings",
    nameKey: "sidebar.settings",
    path: "/settings",
    icon: Settings,
    category: "page",
    keywords: ["设置", "settings", "配置"],
  },
  {
    id: "display-filter",
    nameKey: "sidebar.displayFilter",
    path: "/display-filter",
    icon: Palette,
    category: "builtin-tool",
    keywords: ["滤镜", "显示器", "display", "filter", "色彩", "色温"],
  },
  {
    id: "crosshair",
    nameKey: "sidebar.crosshair",
    path: "/crosshair",
    icon: Crosshair,
    category: "builtin-tool",
    keywords: ["准心", "准星", "crosshair", "瞄准"],
  },
  {
    id: "overlay-panel",
    nameKey: "sidebar.overlayPanel",
    path: "/overlay-panel",
    icon: Layout,
    category: "builtin-tool",
    keywords: ["悬浮", "overlay", "面板", "监控"],
  },
  {
    id: "gpu-rename",
    nameKey: "sidebar.gpuRename",
    path: "/gpu-rename",
    icon: Cpu,
    category: "builtin-tool",
    keywords: ["显卡", "改写", "gpu", "rename", "伪装"],
  },
  {
    id: "resolution-converter",
    nameKey: "sidebar.resolutionConverter",
    path: "/resolution-converter",
    icon: Monitor,
    category: "builtin-tool",
    keywords: ["分辨率", "换算", "resolution", "converter", "比例"],
  },
  {
    id: "dlss-preset",
    nameKey: "sidebar.dlssPreset",
    path: "/dlss-preset",
    icon: Zap,
    category: "builtin-tool",
    keywords: ["dlss", "预设", "preset", "三角洲", "delta", "force", "nvidia", "模型"],
  },
  {
    id: "reaction-test",
    nameKey: "tests.reactionTitle",
    path: "/tests/reaction",
    icon: Zap,
    category: "test",
    keywords: ["反应", "反射弧", "reaction", "测试"],
  },
  {
    id: "aim-test",
    nameKey: "tests.aimTitle",
    path: "/tests/aim",
    icon: Target,
    category: "test",
    keywords: ["瞄准", "aim", "点击", "测试"],
  },
  {
    id: "focus-test",
    nameKey: "tests.focusTitle",
    path: "/tests/focus",
    icon: Focus,
    category: "test",
    keywords: ["专注", "focus", "追踪", "测试"],
  },
  {
    id: "choice-test",
    nameKey: "tests.choiceTitle",
    path: "/tests/choice",
    icon: MousePointerClick,
    category: "test",
    keywords: ["选择", "choice", "测试", "反应"],
  },
  {
    id: "inhibit-test",
    nameKey: "tests.inhibitTitle",
    path: "/tests/inhibit",
    icon: Ban,
    category: "test",
    keywords: ["抑制", "inhibit", "冲动", "测试"],
  },
  {
    id: "schulte-test",
    nameKey: "tests.schulteTitle",
    path: "/tests/schulte",
    icon: Grid3X3,
    category: "test",
    keywords: ["舒尔特", "schulte", "方格", "专注", "测试"],
  },
  {
    id: "windows-optimize",
    nameKey: "optimization.windowsTitle",
    path: "/optimize/windows",
    icon: Monitor,
    category: "optimization",
    keywords: ["windows", "优化", "系统", "性能"],
  },
  {
    id: "memory-limit",
    nameKey: "optimization.memoryLimit.title",
    path: "/optimize/memory-limit",
    icon: Cpu,
    category: "optimization",
    keywords: ["内存", "限制", "memory", "limit", "优化"],
  },
  {
    id: "memory-cleanup",
    nameKey: "optimization.memoryCleanup.title",
    path: "/optimize/memory-cleanup",
    icon: MemoryStick,
    category: "optimization",
    keywords: ["内存", "清理", "memory", "cleanup", "释放", "优化"],
  },
  {
    id: "ace-optimize",
    nameKey: "optimization.aceOptimize.title",
    path: "/optimize/ace-optimize",
    icon: Gauge,
    category: "optimization",
    keywords: ["ace", "优化", "反作弊", "游戏", "进程", "三角洲", "delta"],
  },
  {
    id: "shader-cache",
    nameKey: "shaderCache.title",
    path: "/optimize/shader-cache",
    icon: Trash2,
    category: "optimization",
    keywords: ["着色器", "缓存", "shader", "cache", "清理", "nvidia", "amd"],
  },
];

export const thirdPartyToolIcons: Record<string, ComponentType<{ size?: number; strokeWidth?: number }>> = {
  "memreduct": Zap,
  "windows-core-optimizer": Cpu,
  "optimizer": TrendingUp,
  "cpu-z": Cpu,
  "gpu-z": Monitor,
  "clash-verge": Network,
  "gamepp": Bot,
  "fxsound": Volume2,
  "process-lasso": Cpu,
};

export function getThirdPartyToolIcon(toolId: string): ComponentType<{ size?: number; strokeWidth?: number }> {
  return thirdPartyToolIcons[toolId] || Wrench;
}

export const categoryLabels: Record<SearchCategory, string> = {
  "page": "search.categories.pages",
  "builtin-tool": "search.categories.builtinTools",
  "test": "search.categories.tests",
  "optimization": "search.categories.optimization",
  "thirdparty-tool": "search.categories.thirdpartyTools",
};

export const categoryOrder: SearchCategory[] = [
  "page",
  "builtin-tool",
  "test",
  "optimization",
  "thirdparty-tool",
];

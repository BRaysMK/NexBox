import { extendTheme, type ThemeConfig } from "@chakra-ui/react";

const config: ThemeConfig = {
  initialColorMode: "light",
  useSystemColorMode: false,
};

const theme = extendTheme({
  config,
  fonts: {
    heading: 'var(--app-font-family, "Sinter-Regular"), Inter, system-ui, sans-serif',
    body: 'var(--app-font-family, "Sinter-Regular"), Inter, system-ui, sans-serif',
  },
  colors: {
    brand: {
      50: "#eef2ff",
      100: "#e0e7ff",
      200: "#c7d2fe",
      300: "#a5b4fc",
      400: "#818cf8",
      500: "#6366f1",
      600: "#4f46e5",
      700: "#4338ca",
      800: "#3730a3",
      900: "#312e81",
    },
  },
  styles: {
    global: (props: any) => ({
      body: {
        bg: props.colorMode === 'dark' ? '#000000' : '#f7fafc',
        color: props.colorMode === 'dark' ? '#e0e0e0' : '#1a202c',
      },
    }),
  },
  components: {
    Button: {
      baseStyle: {
        borderRadius: "lg",
        fontWeight: "medium",
      },
    },
    Card: {
      baseStyle: {
        container: {
          borderRadius: "xl",
          bg: undefined,
          borderColor: undefined,
          borderWidth: undefined,
        },
      },
    },
    Tooltip: {
      baseStyle: {
        // 由 BackgroundProvider 通过 CSS 变量驱动：跟随液态玻璃开关与深浅主题
        bg: "var(--nexbox-tooltip-bg, #ffffff)",
        color: "var(--nexbox-tooltip-fg, #1a1a1a)",
        // 覆盖默认 css var，使箭头颜色与胶囊背景一致
        "--tooltip-bg": "var(--nexbox-tooltip-bg, #ffffff)",
        "--tooltip-fg": "var(--nexbox-tooltip-fg, #1a1a1a)",
        "--popper-arrow-bg": "var(--nexbox-tooltip-bg, #ffffff)",
        px: "8px",
        py: "2px",
        borderRadius: "full", // 胶囊全圆角
        fontWeight: "medium",
        fontSize: "xs",
        backdropFilter: "var(--nexbox-tooltip-blur, none)",
        WebkitBackdropFilter: "var(--nexbox-tooltip-blur, none)",
        border: "1px solid",
        borderColor: "var(--nexbox-tooltip-border, rgba(0,0,0,0.10))",
        maxW: "xs",
        zIndex: "tooltip",
      } as any,
      // 全局兜底：点一下提示框即关闭、关闭零延迟，避免部分情况下悬浮提示常驻不消失
      defaultProps: {
        closeOnMouseDown: true,
        closeDelay: 0,
        openDelay: 150,
      } as any,
    },
  },
});

export default theme;

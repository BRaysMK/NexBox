import { extendTheme, type ThemeConfig } from "@chakra-ui/react";

const config: ThemeConfig = {
  initialColorMode: "light",
  useSystemColorMode: false,
};

const theme = extendTheme({
  config,
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
    global: {
      body: {
        bg: "#000000",
        color: "#e0e0e0",
      },
    },
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
          bg: "#111111",
          borderColor: "#333333",
          borderWidth: "1px",
        },
      },
    },
  },
});

export default theme;

import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, Window } from "@tauri-apps/api/window";
import { Box, useColorModeValue } from "@chakra-ui/react";
import { Monitor, RefreshCw, LogOut } from "lucide-react";
import { motion } from "framer-motion";

interface MenuItemProps {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  color?: string;
}

function MenuItem({ icon, label, onClick, color }: MenuItemProps) {
  const hoverBg = useColorModeValue("rgba(255,255,255,0.15)", "rgba(255,255,255,0.08)");
  const labelColor = color ?? useColorModeValue("#e0e0e0", "#e0e0e0");

  return (
    <Box
      as="button"
      w="full"
      display="flex"
      alignItems="center"
      gap={2}
      px={2.5}
      h="44px"
      border="none"
      bg="transparent"
      color={labelColor}
      fontSize="13px"
      cursor="pointer"
      transition="background 0.15s ease"
      _hover={{ bg: hoverBg }}
      _active={{ bg: "rgba(255,255,255,0.12)" }}
      onClick={onClick}
    >
      <Box display="flex" alignItems="center" justifyContent="center" w="18px">
        {icon}
      </Box>
      <span>{label}</span>
    </Box>
  );
}

export default function TrayMenuPage() {
  const bg = "#1a1a1a";
  const borderColor = "rgba(255,255,255,0.08)";

  // 确保窗口背景完全透明（该窗口设置了 transparent: true）
  useEffect(() => {
    const html = document.documentElement;
    const body = document.body;
    const root = document.getElementById("root");
    const prevHtmlBg = html.style.background;
    const prevBodyBg = body.style.background;
    const prevRootBg = root?.style.background;

    html.style.background = "transparent";
    body.style.background = "transparent";
    if (root) root.style.background = "transparent";

    return () => {
      html.style.background = prevHtmlBg;
      body.style.background = prevBodyBg;
      if (root) root.style.background = prevRootBg || "";
    };
  }, []);

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged((event) => {
      if (!event.payload) {
        getCurrentWindow().hide();
      }
    });

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        getCurrentWindow().hide();
      }
    };
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  const handleShowWindow = async () => {
    const mainWindow = await Window.getByLabel("main");
    if (mainWindow) {
      await mainWindow.show();
      await mainWindow.unminimize();
      await mainWindow.setFocus();
    }
    await getCurrentWindow().hide();
  };

  const handleCheckUpdate = () => {
    invoke("check_update_and_show");
    getCurrentWindow().hide();
  };

  const handleExit = () => {
    invoke("exit_app");
  };

  return (
    <Box w="100vw" h="100vh" p={0} m={0} bg="transparent">
      <motion.div
        initial={{ opacity: 0, scale: 0.95, y: -4 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.12, ease: "easeOut" }}
        style={{ height: "100%" }}
      >
        <Box
          w="full"
          h="full"
          bg={bg}
          border="1px solid"
          borderColor={borderColor}
          borderRadius="12px"
          overflow="hidden"
        >
          <MenuItem
            icon={<Monitor size={16} strokeWidth={2} />}
            label="打开主窗口"
            onClick={handleShowWindow}
          />
          <Box h="1px" bg="rgba(255,255,255,0.06)" mx={3} />
          <MenuItem
            icon={<RefreshCw size={16} strokeWidth={2} />}
            label="检查更新"
            onClick={handleCheckUpdate}
          />
          <Box h="1px" bg="rgba(255,255,255,0.06)" mx={3} />
          <MenuItem
            icon={<LogOut size={16} strokeWidth={2} />}
            label="退出"
            onClick={handleExit}
            color="#e74c3c"
          />
        </Box>
      </motion.div>
    </Box>
  );
}

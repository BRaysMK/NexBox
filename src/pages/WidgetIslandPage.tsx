import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, currentMonitor, LogicalPosition, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";
import { listen, emit } from "@tauri-apps/api/event";
import { Menu, MenuItem } from "@tauri-apps/api/menu";

// ─── Types ───────────────────────────────────────────────────────
type NetworkStatus = "good" | "warning" | "error";

interface ToastData {
  app_name: string;
  title: string;
  body: string;
  aumid: string;
}

// ─── Helpers ─────────────────────────────────────────────────────
const formatSpeed = (bytes: number) => {
  if (bytes < 1024) return bytes + " B/s";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB/s";
  return (bytes / (1024 * 1024)).toFixed(1) + " MB/s";
};

const getAppIcon = (appName: string): string => {
  const name = appName.toLowerCase();
  if (name.includes("qq") || name.includes("腾讯")) return "/icons/qq.png";
  if (name.includes("dingtalk") || name.includes("钉钉")) return "/icons/dingtalk.png";
  if (name.includes("wechat") || name.includes("微信") || name.includes("weixin") || name.includes("wx")) return "/icons/wechat.png";
  if (name.includes("mail") || name.includes("outlook") || name.includes("邮件")) return "/icons/mail.png";
  if (name.includes("alipay") || name.includes("支付宝")) return "/icons/alipay.jpg";
  // ── 常见应用（浏览器/游戏/社交/笔记等）──
  if (name.includes("chrome") || name.includes("google")) return "/icons/browser.svg";
  if (name.includes("edge") || name.includes("bing")) return "/icons/browser.svg";
  if (name.includes("steam") || name.includes("epic")) return "/icons/game.svg";
  if (name.includes("discord") || name.includes("telegram")) return "/icons/chat.svg";
  if (name.includes("onenote") || name.includes("notion") || name.includes("evernote") || name.includes("笔记")) return "/icons/note.svg";
  if (name.includes("bilibili") || name.includes("哔哩哔哩")) return "/icons/bilibili.svg";
  // 未知应用 → 使用默认中性图标（不再是 QQ 图标！）
  return "/logo/NexBoxW.png";
};

// ─── Spring Animation ────────────────────────────────────────────
const springDecay = (t: number, freq = 2.0, decay = 10.5): number => {
  return 1 - Math.cos(freq * t * 2 * Math.PI) * Math.exp(-decay * t);
};

const animateWithSpring = (
  duration: number,
  onFrame: (progress: number) => void,
  onDone: () => void,
) => {
  const start = performance.now();
  const run = (time: number) => {
    const elapsed = (time - start) / 1000;
    const t = Math.min(elapsed / (duration / 1000), 1);
    const spring = springDecay(elapsed);
    onFrame(Math.min(spring, 1));
    if (t < 1) {
      requestAnimationFrame(run);
    } else {
      onFrame(1);
      onDone();
    }
  };
  requestAnimationFrame(run);
};

// ─── Component ───────────────────────────────────────────────────
export default function WidgetIslandPage() {
  // Visibility
  const [isVisible, setIsVisible] = useState(false);
  const [isMenuOpen, setIsMenuOpen] = useState(false);

  // Dims
  const [currentWidth, setCurrentWidth] = useState(260);
  const [currentHeight, setCurrentHeight] = useState(42);
  const currentWidthRef = useRef(260);
  const currentHeightRef = useRef(42);
  const trackedPos = useRef({ x: 0, y: 0 });

  // Theme & opacity
  const [opacity, setOpacity] = useState(() => Number(localStorage.getItem("nsd_island_opacity") || "100"));
  const [theme, setTheme] = useState(() => localStorage.getItem("nsd_island_theme") || "black");

  // features
  const [isPinned, setIsPinned] = useState(() => localStorage.getItem("nsd_pin_taskbar") === "true");
  const [showMusic, setShowMusic] = useState(() => localStorage.getItem("nsd_music_ctrl") !== "false");
  const [showHardware, setShowHardware] = useState(() => localStorage.getItem("nsd_hardware_mon") === "true");
  const [glowBorder, setGlowBorder] = useState(() => localStorage.getItem("nsd_glow_border") !== "false");
  const [msgEnabled] = useState(() => localStorage.getItem("nsd_msg_notify") === "true");

  // Speed
  const [upload, setUpload] = useState("0 KB/s");
  const [download, setDownload] = useState("0 KB/s");
  const [highUpload, setHighUpload] = useState(false);
  const [highDownload, setHighDownload] = useState(false);
  const [netStatus, setNetStatus] = useState<NetworkStatus>("good");
  const lastRx = useRef(0);
  const lastTx = useRef(0);

  // Hardware
  const [cpu, setCpu] = useState("0%");
  const [gpu, setGpu] = useState("0%");
  const [mem, setMem] = useState("0%");

  // Beijing Time
  const [beijingTime, setBeijingTime] = useState("--:--:--");

  // Music
  const [isPlaying, setIsPlaying] = useState(false);
  const [showInfo, setShowInfo] = useState(true);
  const [trackInfo, setTrackInfo] = useState("未在播放歌曲 - 未知歌手");
  const [coverUrl, setCoverUrl] = useState("");
  const isClickingToggle = useRef(false);
  const coverCache = useRef<Map<string, string>>(new Map());
  const musicBoxKey = useRef(0);

  // Notification
  const [isMsgActive, setIsMsgActive] = useState(false);
  const [msgTitle, setMsgTitle] = useState("");
  const [msgBody, setMsgBody] = useState("");
  const [msgIcon, setMsgIcon] = useState("");
  const msgTimer = useRef<number | null>(null);
  const isMsgActiveRef = useRef(false);

  // Timers
  const speedTimer = useRef<number | null>(null);
  const pingTimer = useRef<number | null>(null);

  const lowTrafficStart = useRef(Date.now());
  const RED_DELAY = 5000;

  // ─── Style Computations ──────────────────────────────────────
  const linearAlpha = Math.pow(opacity / 100, 1 / 2.2);
  const bgColor = theme === "white"
    ? `rgba(255, 255, 255, ${linearAlpha})`
    : `rgba(0, 0, 0, ${linearAlpha})`;
  const textColor = theme === "white" ? "#000000" : "#ffffff";

  // ─── Window Position Helpers ────────────────────────────────
  const snapToBottomLeft = useCallback(async () => {
    try {
      const appWindow = getCurrentWindow();
      await new Promise((r) => setTimeout(r, 150));
      const monitor = await currentMonitor();
      if (!monitor) return;
      const sf = window.devicePixelRatio;
      const w = currentWidthRef.current;
      const h = currentHeightRef.current;
      await appWindow.setSize(new PhysicalSize(Math.ceil(w * sf), Math.ceil(h * sf)));
      const x = monitor.position.x + 10 * sf;
      const y = monitor.position.y + monitor.size.height - (h + 3) * sf;
      await appWindow.hide();
      await appWindow.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));
      await appWindow.show();
      trackedPos.current = { x: Math.round(x), y: Math.round(y) };
    } catch (e) {
      console.error("snapToBottomLeft failed:", e);
    }
  }, []);

  const adjustPosition = useCallback(async () => {
    try {
      const appWindow = getCurrentWindow();
      await new Promise((r) => setTimeout(r, 150));
      const monitor = await currentMonitor();
      if (!monitor) return;
      const sf = window.devicePixelRatio;
      const w = currentWidthRef.current;
      const h = currentHeightRef.current;
      await appWindow.setSize(new PhysicalSize(Math.ceil(w * sf), Math.ceil(h * sf)));
      const winSize = await appWindow.innerSize();
      const mw = monitor.size.width;
      const ml = monitor.position.x;
      const x = ml + (mw - winSize.width) / 2;
      const y = monitor.position.y + 12 * sf;
      await appWindow.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));
      trackedPos.current = { x: Math.round(x), y: Math.round(y) };
    } catch (e) {
      console.error("adjustPosition failed:", e);
    } finally {
      await getCurrentWindow().show().catch(() => {});
    }
  }, []);

  // ─── Island Size Animation ──────────────────────────────────
  const animateIslandSize = useCallback((targetW: number, targetH: number) => {
    const startW = currentWidthRef.current;
    const startH = currentHeightRef.current;
    const appWindow = getCurrentWindow();
    const dpr = window.devicePixelRatio;
    const centerX = trackedPos.current.x + (startW * dpr) / 2;
    const originY = trackedPos.current.y;

    // Pin tracked pos to center during animation so reposition works
    trackedPos.current = { ...trackedPos.current };

    animateWithSpring(600,
      (spring) => {
        const newW = startW + (targetW - startW) * spring;
        const newH = startH + (targetH - startH) * spring;
        currentWidthRef.current = newW;
        currentHeightRef.current = newH;
        setCurrentWidth(newW);
        setCurrentHeight(newH);
        const leftX = Math.round(centerX - (newW * dpr) / 2);
        appWindow.setPosition(new PhysicalPosition(leftX, originY)).catch(() => {});
        appWindow.setSize(new PhysicalSize(Math.ceil(newW * dpr), Math.ceil(newH * dpr))).catch(() => {});
      },
      () => {
        currentWidthRef.current = targetW;
        currentHeightRef.current = targetH;
        setCurrentWidth(targetW);
        setCurrentHeight(targetH);
        trackedPos.current.x = Math.round(centerX - (targetW * dpr) / 2);
        appWindow.setPosition(new PhysicalPosition(trackedPos.current.x, originY)).catch(() => {});
        appWindow.setSize(new PhysicalSize(Math.ceil(targetW * dpr), Math.ceil(targetH * dpr))).catch(() => {});
      },
    );
  }, []);

  // ─── Data Fetching ──────────────────────────────────────────
  const fetchSpeed = useCallback(async () => {
    try {
      const [rx, tx] = await invoke<[number, number]>("get_network_stats");
      if (lastRx.current !== 0) {
        const rxDiff = rx - lastRx.current;
        const txDiff = tx - lastTx.current;
        setDownload(formatSpeed(rxDiff));
        setUpload(formatSpeed(txDiff));
        const limit = 1024 * 1024;
        setHighDownload(rxDiff >= limit);
        setHighUpload(txDiff >= limit);
        if (rxDiff >= limit || txDiff >= limit) {
          lowTrafficStart.current = Date.now();
        }
      }
      lastRx.current = rx;
      lastTx.current = tx;
    } catch (e) {
      console.error("fetchSpeed failed:", e);
    }
  }, []);

  const checkLatency = useCallback(async () => {
    try {
      const lat = await invoke<number>("get_network_latency");
      setNetStatus(lat < 150 ? "good" : "warning");
    } catch {
      if (highDownload || highUpload) {
        setNetStatus("warning");
      } else if (Date.now() - lowTrafficStart.current < RED_DELAY) {
        setNetStatus("warning");
      } else {
        setNetStatus("error");
      }
    }
  }, [highDownload, highUpload]);

  const fetchHardware = useCallback(async () => {
    try {
      const [cpuLoad, usedMem, totalMem] = await invoke<[number, number, number]>("get_hardware_stats");
      setCpu(Math.round(cpuLoad) + "%");
      if (totalMem > 0) {
        setMem(Math.round((usedMem / totalMem) * 100) + "%");
      }
      // GPU estimation (same approach as reference)
      const estimatedGpu = Math.min(Math.max(Math.round(cpuLoad * 0.4) + Math.floor(Math.random() * 5), 1), 99);
      setGpu(estimatedGpu + "%");
    } catch (e) {
      console.error("fetchHardware failed:", e);
    }
  }, []);

  const syncMusic = useCallback(async () => {
    try {
      const res = await invoke<[string, string, boolean] | null>("fetch_netease_music_info");
      if (res) {
        const [song, artist, playing] = res;
        const info = `${song} - ${artist}`;
        setTrackInfo((prev) => {
          if (prev !== info) {
            if (coverCache.current.has(info)) {
              setCoverUrl(coverCache.current.get(info)!);
            } else {
              invoke<string>("get_random_cover_url", { songName: song, artistName: artist })
                .then((url) => {
                  coverCache.current.set(info, url);
                  if (coverCache.current.size > 50) coverCache.current.clear();
                  setCoverUrl(url);
                })
                .catch(() => setCoverUrl(""));
            }
          }
          return info;
        });
        if (!isClickingToggle.current) setIsPlaying(playing);
      } else {
        setTrackInfo("未在播放歌曲 - 网易云音乐");
        setIsPlaying(false);
        setCoverUrl("");
      }
    } catch (e) {
      console.error("syncMusic failed:", e);
    }
  }, []);

  // ─── Timers ─────────────────────────────────────────────────
  useEffect(() => {
    speedTimer.current = window.setInterval(async () => {
      if (isPinned && isVisible && !isMenuOpen) {
        invoke("force_window_topmost").catch(() => {});
      }
      fetchSpeed();
      if (showMusic) syncMusic();
      if (showHardware) fetchHardware();

      // Check notifications
      if (msgEnabled) {
        try {
          const res = await invoke<ToastData | null>("fetch_latest_notification");
          if (res) {
            setMsgTitle(res.app_name);
            setMsgBody(res.title + (res.body ? ": " + res.body : ""));
            setMsgIcon(getAppIcon(res.app_name));
            if (!isMsgActiveRef.current) {
              isMsgActiveRef.current = true;
              setIsMsgActive(true);
              if (!isPinned) animateIslandSize(360, 65);
              // Start auto-dismiss timer only on first show
              if (msgTimer.current) clearTimeout(msgTimer.current);
              msgTimer.current = window.setTimeout(() => {
                isMsgActiveRef.current = false;
                setIsMsgActive(false);
                animateIslandSize(260, 42);
              }, 2000);
            }
          }
        } catch (e) {
          console.error("notif check failed:", e);
        }
      }
    }, 1000);

    pingTimer.current = window.setInterval(checkLatency, 5500);

    return () => {
      if (speedTimer.current) clearInterval(speedTimer.current);
      if (pingTimer.current) clearInterval(pingTimer.current);
    };
  }, [isPinned, isVisible, isMenuOpen, showMusic, showHardware, msgEnabled, fetchSpeed, syncMusic, fetchHardware, checkLatency]);

  // Track window position after move
  useEffect(() => {
    const appWindow = getCurrentWindow();
    appWindow.onMoved((evt) => {
      trackedPos.current = { x: evt.payload.x, y: evt.payload.y };
    }).catch(() => {});
  }, []);

  // Beijing Time updater (UTC+8)
  useEffect(() => {
    const update = () => {
      const now = new Date();
      const beijing = new Date(now.getTime() + 8 * 3600000);
      const h = String(beijing.getUTCHours()).padStart(2, "0");
      const m = String(beijing.getUTCMinutes()).padStart(2, "0");
      const s = String(beijing.getUTCSeconds()).padStart(2, "0");
      setBeijingTime(`${h}:${m}:${s}`);
    };
    update();
    const id = window.setInterval(update, 1000);
    return () => clearInterval(id);
  }, []);

  // Initialization
  useEffect(() => {
    let unlistenFns: Array<() => void> = [];
    let isCancelled = false;

    const init = async () => {
      // ─── Event listeners ────────────────────────────────
      // MUST be registered BEFORE the visibility check so that the settings page
      // can always communicate with the widget, even on first launch.
      const u1 = await listen<{ enabled: boolean }>("control-music-ctl", (evt) => {
        setShowMusic(evt.payload.enabled);
        if (evt.payload.enabled) {
          if (localStorage.getItem("nsd_glow_border") === null) {
            setGlowBorder(true);
            localStorage.setItem("nsd_glow_border", "true");
          }
          setShowInfo(true);
          musicBoxKey.current++;
        }
      });

      const u2 = await listen<{ opacity: number }>("control-island-opacity", (evt) => {
        setOpacity(evt.payload.opacity);
      });

      const u3 = await listen<{ theme: string }>("control-island-theme", (evt) => {
        setTheme(evt.payload.theme);
      });

      const u4 = await listen<{ enabled: boolean }>("control-pin-taskbar", async (evt) => {
        setIsPinned(evt.payload.enabled);
        if (evt.payload.enabled) {
          await snapToBottomLeft();
        } else {
          await adjustPosition();
        }
      });

      const u5 = await listen<{ enabled: boolean }>("control-hardware-mon", (evt) => {
        setShowHardware(evt.payload.enabled);
      });

      const u6 = await listen<{ show: boolean }>("control-island-visibility", async (evt) => {
        if (evt.payload.show) {
          // Position before showing (first open uses correct default position)
          const pinned = localStorage.getItem("nsd_pin_taskbar") === "true";
          if (pinned) {
            await snapToBottomLeft();
          } else {
            await adjustPosition();
          }
          await getCurrentWindow().setAlwaysOnTop(true);
          setIsVisible(true);
          // Sync content state immediately on first open
          setShowInfo(true);
          if (localStorage.getItem("nsd_music_ctrl") !== "false") {
            syncMusic();
          }
          if (localStorage.getItem("nsd_hardware_mon") === "true") {
            fetchHardware();
          }
        } else {
          setIsVisible(false);
        }
      });

      unlistenFns = [u1, u2, u3, u4, u5, u6];

      if (isCancelled) {
        unlistenFns.forEach((fn) => fn());
        return;
      }

      // Check if island was enabled before showing
      const wasVisible = localStorage.getItem("nsd_island_visible") === "true";
      if (!wasVisible) {
        return; // Stay hidden, the settings page will show via event
      }

      // Position
      if (isPinned) {
        await snapToBottomLeft();
      } else {
        await adjustPosition();
      }
      await getCurrentWindow().show();
      setIsVisible(true);

      fetchSpeed();
      checkLatency();
    };

    init();

    return () => {
      isCancelled = true;
      unlistenFns.forEach((fn) => fn());
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // ─── Callbacks for music ────────────────────────────────────
  const togglePlay = useCallback(async () => {
    setIsPlaying((p) => !p);
    isClickingToggle.current = true;
    await invoke("control_system_media", { action: "play_pause" });
    setTimeout(() => { isClickingToggle.current = false; }, 1500);
  }, []);

  const prevTrack = useCallback(async () => {
    await invoke("control_system_media", { action: "prev" });
  }, []);
  const nextTrack = useCallback(async () => {
    await invoke("control_system_media", { action: "next" });
  }, []);

  // ─── Mouse / Context handlers ───────────────────────────────
  const handleMouseDown = useCallback(async (e: React.MouseEvent) => {
    if (isPinned) return;
    if ((e.target as HTMLElement).closest(".ctl-btn")) return;
    if ((e.target as HTMLElement).closest(".msg-box")) return;
    if (e.button === 0) {
      try {
        await getCurrentWindow().startDragging();
      } catch (err) {
        console.error("drag failed:", err);
      }
    }
  }, [isPinned]);

  const handleContextMenu = useCallback(async (e: React.MouseEvent) => {
    e.preventDefault();
    const resetItem = await MenuItem.new({
      text: isPinned ? "重置位置 (已锁定)" : "重置位置",
      id: "reset_position",
      enabled: !isPinned,
      action: () => adjustPosition(),
    });
    const glowItem = await MenuItem.new({
      text: glowBorder ? "关闭流光边框" : "开启流光边框",
      id: "toggle_glow",
      action: () => {
        setGlowBorder((v) => !v);
        localStorage.setItem("nsd_glow_border", String(!glowBorder));
      },
    });
    const closeItem = await MenuItem.new({
      text: "关闭",
      id: "close",
      action: () => setIsVisible(false),
    });
    const menu = await Menu.new();
    await menu.append(glowItem);
    await menu.append(resetItem);
    await menu.append(closeItem);
    setIsMenuOpen(true);
    try {
      await menu.popup(new LogicalPosition(e.clientX, e.clientY));
    } catch (err) {
      console.error("menu popup failed:", err);
    } finally {
      setIsMenuOpen(false);
    }
  }, [isPinned, glowBorder, adjustPosition]);


  // ─── Animation ───────────────────────────────────────────────
  const [animScale, setAnimScale] = useState(0);
  const [animOpacity, setAnimOpacity] = useState(0);

  useEffect(() => {
    if (isVisible) {
      animateWithSpring(600,
        (p) => { setAnimScale(p); setAnimOpacity(Math.min(1, p * 4)); },
        () => { setAnimScale(1); setAnimOpacity(1); },
      );
    } else {
      // leave animation
      const start = performance.now();
      const run = (time: number) => {
        const t = Math.min((time - start) / 300, 1);
        const s = 1 - Math.pow(t, 3);
        setAnimScale(Math.max(0, s));
        setAnimOpacity(Math.max(0, 1 - t * 1.5));
        if (t < 1) {
          requestAnimationFrame(run);
        } else {
          getCurrentWindow().hide().catch(() => {});
          emit("island-status-sync", { visible: false });
        }
      };
      requestAnimationFrame(run);
    }
  }, [isVisible]);

  // ─── Render ──────────────────────────────────────────────────
  if (!isVisible && animOpacity === 0) return null;

  const isActiveContent = showMusic || showHardware || isMsgActive;

  return (
    <div
      style={{
        width: currentWidth,
        height: currentHeight,
        backgroundColor: bgColor,
        color: textColor,
        borderRadius: 100,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 2,
        userSelect: "none",
        overflow: "hidden",
        boxSizing: "border-box",
        transform: `scale(${animScale})`,
        opacity: animOpacity,
        transformOrigin: "center top",
        position: "relative",
        transition: "background 0.4s ease",
      }}
      onMouseDown={handleMouseDown}
      onContextMenu={handleContextMenu}
    >
      {/* Rainbow Border */}
      {glowBorder && (
        <div
          style={{
            position: "absolute",
            width: 500,
            height: 500,
            top: "calc(50% - 250px)",
            left: "calc(50% - 250px)",
            zIndex: 1,
            background: "conic-gradient(from 0deg, #ff3b30, #ff9500, #ffcc00, #4cd964, #5ac8fa, #007aff, #5856d6, #ff3b30)",
            animation: "nsd-rainbow 3s linear infinite",
            opacity: Math.pow(opacity / 100, 1 / 2.2),
          }}
        />
      )}

      {/* Core Content */}
      <div
        style={{
          position: "relative",
          zIndex: 2,
          width: "100%",
          height: "100%",
          borderRadius: 98,
          backdropFilter: "blur(20px)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 14px",
          overflow: "hidden",
          backgroundColor: bgColor,
        }}
      >
        {/* Inner wrapper for content switching */}
        <div style={{ position: "relative", flexGrow: 1, height: "100%", display: "flex", alignItems: "center" }}>
          {/* Notification */}
          {isMsgActive && (
            <div
              className="msg-box"
              style={{
                position: "absolute", left: 0, top: 0, width: "100%", height: "100%",
                display: "flex", alignItems: "center", padding: "0 45px 0 0", gap: 12,
                zIndex: 10,
              }}
            >
              <img src={msgIcon} alt="" style={{ width: 30, height: 30, borderRadius: "50%", objectFit: "cover" }} />
              <div style={{ display: "flex", flexDirection: "column", justifyContent: "center", overflow: "hidden", flexGrow: 1 }}>
                <div style={{ fontSize: 14, fontWeight: 700, lineHeight: 1.4, opacity: 0.95, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {msgTitle}
                </div>
                <div style={{ fontSize: 12.5, lineHeight: 1.4, opacity: 0.75, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {msgBody}
                </div>
              </div>
            </div>
          )}

          {/* Hardware Monitor */}
          {showHardware && !isMsgActive && (
            <div
              style={{
                position: "absolute", left: 0, top: 0, width: "100%", height: "100%",
                display: "flex", alignItems: "center", gap: 2,
              }}
            >
              <HardwareItem label="CPU" value={cpu} textColor={textColor} />
              <Divider />
              <HardwareItem label="GPU" value={gpu} textColor={textColor} />
              <Divider />
              <HardwareItem label="RAM" value={mem} textColor={textColor} />
            </div>
          )}

          {/* Music Control */}
          {showMusic && !isMsgActive && (
            <div
              style={{
                position: "absolute", left: 0, top: 0, width: "100%", height: "100%",
                display: "flex", alignItems: "center",
              }}
              onMouseEnter={() => setShowInfo(false)}
              onMouseLeave={() => setShowInfo(true)}
            >
              {/* Album Cover */}
              <div
                style={{
                  width: 24, height: 24, borderRadius: "50%", flexShrink: 0, overflow: "hidden",
                  border: "2px solid " + (theme === "white" ? "rgba(0,0,0,0.15)" : "rgba(255,255,255,0.5)"),
                  transform: isPlaying ? "scale(1.08) translateX(-5px)" : "translateX(-5px)",
                  transition: "all 0.3s cubic-bezier(0.175, 0.885, 0.32, 1.275)",
                }}
              >
                <div
                  style={{
                    width: "100%", height: "100%",
                    background: coverUrl ? `url(${coverUrl}) center/cover` : "linear-gradient(135deg, #a8edea, #fed6e3)",
                    animation: isPlaying ? "nsd-spin 8s linear infinite" : "none",
                  }}
                />
              </div>

              {/* Controls or Track Info */}
              {!showInfo && (
                <div style={{ position: "absolute", left: "50%", transform: "translateX(-50%)", display: "flex", gap: 12, alignItems: "center" }}>
                  <button className="ctl-btn" onClick={prevTrack} style={ctlBtnStyle}>
                    <svg viewBox="0 0 24 24" fill="currentColor" width={16} height={16}><path d="M6 6h2v12H6zm3.5 6l8.5 6V6z" /></svg>
                  </button>
                  <button className="ctl-btn" onClick={togglePlay} style={{ ...ctlBtnStyle, padding: 6 }}>
                    {isPlaying ? (
                      <svg viewBox="0 0 24 24" fill="currentColor" width={20} height={20}><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" /></svg>
                    ) : (
                      <svg viewBox="0 0 24 24" fill="currentColor" width={20} height={20} style={{ transform: "translateX(1px)" }}><path d="M8 5v14l11-7z" /></svg>
                    )}
                  </button>
                  <button className="ctl-btn" onClick={nextTrack} style={ctlBtnStyle}>
                    <svg viewBox="0 0 24 24" fill="currentColor" width={16} height={16}><path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z" /></svg>
                  </button>
                </div>
              )}

              {showInfo && (
                <div
                  style={{
                    position: "absolute", left: 24, right: 20, height: "100%",
                    display: "flex", alignItems: "center", overflow: "hidden", paddingLeft: 8,
                    maskImage: "linear-gradient(to right, #000 75%, transparent 100%)",
                    WebkitMaskImage: "linear-gradient(to right, #000 75%, transparent 100%)",
                  }}
                >
                  <div style={{ fontSize: 12.5, fontWeight: 500, whiteSpace: "nowrap", overflow: "hidden", opacity: 0.9 }}>
                    {trackInfo}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Beijing Time */}
          {!showMusic && !showHardware && !isMsgActive && (
            <div
              style={{
                position: "absolute", left: 0, top: 0, width: "100%", height: "100%",
                display: "flex", alignItems: "center", justifyContent: "center",
              }}
            >
              <span style={{ fontSize: 14, fontWeight: 600, letterSpacing: "0.5px", fontVariantNumeric: "tabular-nums", opacity: 0.95 }}>
                {beijingTime}
              </span>
            </div>
          )}
        </div>

        {/* Status Dot */}
        {isActiveContent && (
          <div
            style={{
              width: 6, height: 6, borderRadius: "50%", flexShrink: 0, marginLeft: 4,
              backgroundColor: netStatus === "good" ? "#34C759" : netStatus === "warning" ? "#FFCC00" : "#FF3B30",
              boxShadow: netStatus === "good" ? "0 0 10px rgba(52,199,89,0.5)" : netStatus === "warning" ? "0 0 10px rgba(255,204,0,0.5)" : "0 0 10px rgba(255,59,48,0.5)",
              transition: "background-color 0.4s ease",
            }}
          />
        )}
      </div>
    </div>
  );
}

// ─── Sub-components ───────────────────────────────────────────────

const ctlBtnStyle: React.CSSProperties = {
  background: "transparent", border: "none", color: "inherit",
  cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "center",
  padding: 6, borderRadius: "50%", transition: "background-color 0.2s ease, opacity 0.2s ease, transform 0.1s ease",
  outline: "none",
};

function SpeedItem({ label, value, high, textColor }: { label: string; value: string; high: boolean; textColor: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
      <span
        style={{
          fontSize: 10, fontWeight: "bold", padding: "2px 4px", borderRadius: 4,
          color: textColor, opacity: high ? 0.9 : 0.4,
          background: high ? "rgba(255,255,255,0.15)" : "transparent",
          transition: "all 0.3s ease",
        }}
      >
        {label}
      </span>
      <span style={{ fontSize: 12, fontWeight: "bold", minWidth: 52, letterSpacing: "-0.2px" }}>
        {value}
      </span>
    </div>
  );
}

function HardwareItem({ label, value, textColor }: { label: string; value: string; textColor: string }) {
  const isHigh = parseInt(value) >= 90;
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6, marginLeft: 5 }}>
      <span style={{ fontSize: 10, fontWeight: "bold", opacity: 0.5 }}>{label}</span>
      <span style={{ fontSize: 13, fontWeight: "bold", minWidth: 36, letterSpacing: "-0.2px", color: isHigh ? "#f06861" : textColor, transition: "color 0.3s ease" }}>
        {value}
      </span>
    </div>
  );
}

function Divider() {
  return (
    <span style={{ width: 1, height: 14, backgroundColor: "currentColor", opacity: 0.2 }} />
  );
}

// ─── Global Styles (injected once) ─────────────────────────────
const styleId = "nsd-widget-styles";
if (!document.getElementById(styleId)) {
  const sheet = document.createElement("style");
  sheet.id = styleId;
  sheet.textContent = `
    @keyframes nsd-rainbow {
      from { transform: rotate(0deg); }
      to { transform: rotate(360deg); }
    }
    @keyframes nsd-spin {
      from { transform: rotate(0deg); }
      to { transform: rotate(360deg); }
    }
    html, body {
      background-color: transparent !important;
      background: transparent !important;
      overflow: hidden;
      margin: 0;
      padding: 0;
    }
    .ctl-btn:hover { background-color: rgba(255,255,255,0.15); }
  `;
  document.head.appendChild(sheet);
}

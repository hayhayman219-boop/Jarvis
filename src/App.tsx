import { useEffect, useState } from "react";
import "./styles/theme.css";
import "./styles/orb.css";
import "./styles/pearl.css";
import { Orb } from "./components/hud/Orb";
import { ChatOverlay } from "./components/chat/ChatOverlay";
import { SidePanel } from "./components/hud/SidePanel";
import { Dashboard } from "./components/hud/Dashboard";
import { GaugesRow } from "./components/widgets/GaugesRow";
import { TimersWidget } from "./components/widgets/TimersWidget";
import { ScreenLauncher } from "./components/hud/ScreenLauncher";
import { ScreenHost } from "./components/hud/ScreenHost";
import { useChatStore } from "./state/chatStore";
import { useSettingsStore } from "./state/settingsStore";
import { useSystemStatusStore } from "./state/systemStatusStore";
import { startBackgroundVoiceLoop, stopBackgroundVoiceLoop } from "./state/voiceStore";
import { startTimersTicker } from "./state/timersStore";
import { registerDesktopEventListeners } from "./lib/desktopEvents";
import { usePolling } from "./hooks/usePolling";

const SYSTEM_STATUS_POLL_MS = 2000;

function App() {
  const status = useChatStore((s) => s.status);
  const hydrateChat = useChatStore((s) => s.hydrate);
  const model = useSettingsStore((s) => s.model);
  const hydrate = useSettingsStore((s) => s.hydrate);
  const refreshSystemStatus = useSystemStatusStore((s) => s.refresh);
  const [chatOpen, setChatOpen] = useState(false);

  useEffect(() => {
    hydrate();
    hydrateChat();
  }, [hydrate, hydrateChat]);

  useEffect(() => {
    registerDesktopEventListeners();
    startBackgroundVoiceLoop();
    startTimersTicker();
    return () => {
      stopBackgroundVoiceLoop();
    };
  }, []);

  usePolling(() => refreshSystemStatus(model), SYSTEM_STATUS_POLL_MS, [model]);

  // Surface the conversation only when Jarvis is actually engaged — thinking
  // or speaking. Passive "listening" (the always-on wake-word loop fires this
  // on any ambient sound) must NOT reopen the panel, or the user can never
  // close it: every noise after closing would spring it back open.
  useEffect(() => {
    if (status === "thinking" || status === "speaking") setChatOpen(true);
  }, [status]);

  return (
    <div style={{ position: "relative", width: "100vw", height: "100vh", overflow: "hidden" }}>
      {/* The orb is the whole screen; clicking it opens the conversation. */}
      <div onClick={() => setChatOpen(true)} style={{ cursor: "pointer" }}>
        <Orb status={status} />
      </div>

      <SidePanel />

      {/* Left-docked info column: weather, schedule, reminders. Hidden while
          the conversation overlay is up to keep that view clean. */}
      {!chatOpen && <Dashboard />}

      {/* Live system monitor gauges (bottom-right). */}
      {!chatOpen && <GaugesRow />}

      {/* Active timers/alarms (bottom-left) — visible even over the chat. */}
      <TimersWidget />

      {/* Pop-up screen launchers (one button per registered screen). */}
      <ScreenLauncher />

      {/* Any open pop-up screens (Notion, and future screens). */}
      <ScreenHost />

      {chatOpen && <ChatOverlay onClose={() => setChatOpen(false)} />}

      {!chatOpen && (
        <div
          style={{
            position: "fixed",
            bottom: 20,
            left: 0,
            right: 0,
            textAlign: "center",
            color: "#bfe9ff",
            letterSpacing: 4,
            textTransform: "uppercase",
            fontSize: 12,
            opacity: 0.55,
            pointerEvents: "none",
          }}
        >
          Say "Jarvis" — or click the core
        </div>
      )}
    </div>
  );
}

export default App;

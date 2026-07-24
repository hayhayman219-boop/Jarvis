import { useEffect, useRef, useState } from "react";
import { PopupScreen } from "../hud/PopupScreen";

// Live webcam feed in a pop-up. Uses the browser getUserMedia API (works in
// the Tauri webview); the stream is stopped on unmount so the camera light
// goes off when the screen is closed.
export function WebcamViewer({
  onClose,
  index,
}: {
  onClose: () => void;
  index: number;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let stream: MediaStream | null = null;
    let cancelled = false;
    (async () => {
      try {
        const md = navigator.mediaDevices;
        if (!md) throw new Error("No media devices available");
        // Get permission first so device labels are populated.
        let s = await md.getUserMedia({ video: true, audio: false });
        // Prefer the external Logitech C170 over the internal camera.
        const devices = await md.enumerateDevices();
        const preferred = devices.find(
          (d) => d.kind === "videoinput" && /c170|logitech/i.test(d.label),
        );
        const current = s.getVideoTracks()[0]?.getSettings().deviceId;
        if (preferred?.deviceId && preferred.deviceId !== current) {
          s.getTracks().forEach((t) => t.stop());
          s = await md.getUserMedia({
            video: { deviceId: { exact: preferred.deviceId } },
            audio: false,
          });
        }
        if (cancelled) {
          s.getTracks().forEach((t) => t.stop());
          return;
        }
        stream = s;
        if (videoRef.current) videoRef.current.srcObject = s;
      } catch (e: unknown) {
        if (!cancelled) setError(String((e as Error)?.message ?? e));
      }
    })();
    return () => {
      cancelled = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, []);

  return (
    <PopupScreen title="Webcam" onClose={onClose} index={index} width={640} height={520}>
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "#000",
        }}
      >
        {error ? (
          <div style={{ padding: 24, color: "var(--jarvis-red)", fontSize: 13, textAlign: "center", lineHeight: 1.5 }}>
            Could not access the camera.
            <div style={{ color: "var(--jarvis-text-dim)", marginTop: 8 }}>{error}</div>
          </div>
        ) : (
          <video
            ref={videoRef}
            autoPlay
            playsInline
            muted
            style={{ maxWidth: "100%", maxHeight: "100%" }}
          />
        )}
      </div>
    </PopupScreen>
  );
}

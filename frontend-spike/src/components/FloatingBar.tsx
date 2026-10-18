import { THEME } from "../theme";

/**
 * FloatingBar — the app's launch UI, exactly like Recordly.
 *
 * In the real Tauri app this is a SECOND window ("control-bar"):
 *   decorations: false, alwaysOnTop: true, transparent: true,
 *   positioned bottom-center at runtime (see WINDOWS_SPEC.md).
 * In this mock it renders as a pill floating over a fake desktop.
 */
interface Props {
  recording: boolean;
  paused: boolean;
  elapsed: string;
  sources: Array<{ id: string; label: string }>;
  sourceId: string;
  micOn: boolean;
  onSourceChange: (id: string) => void;
  onMicToggle: () => void;
  onRecord: () => void;
  onPause: () => void;
  onResume: () => void;
  onStop: () => void;
  onOpenEditor: () => void;
}

export default function FloatingBar(p: Props) {
  return (
    <div
      style={{
        position: "fixed",
        left: "50%",
        bottom: 28,
        transform: "translateX(-50%)",
        zIndex: 50,
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "10px 14px",
        background: "rgba(21, 27, 37, 0.92)",
        backdropFilter: "blur(12px)",
        border: `1px solid ${THEME.border}`,
        borderRadius: 999,
        boxShadow:
          "0 12px 40px rgba(0,0,0,0.55), 0 2px 8px rgba(0,0,0,0.4)",
      }}
    >
      <img
        src="logo-transparent.png"
        alt="FlickCut"
        style={{ height: 30, width: "auto" }}
      />

      {!p.recording ? (
        <>
          <select
            className="input"
            value={p.sourceId}
            onChange={(e) => p.onSourceChange(e.target.value)}
            style={{
              maxWidth: 190,
              borderRadius: 999,
              background: THEME.input,
            }}
            title="Capture source"
          >
            {p.sources.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
          <button
            className="icon-btn"
            title={p.micOn ? "Microphone on" : "Microphone off"}
            onClick={p.onMicToggle}
            style={{
              width: 34,
              height: 34,
              borderRadius: "50%",
              border: `1px solid ${p.micOn ? THEME.success : THEME.border}`,
              color: p.micOn ? THEME.success : THEME.faint,
              fontSize: 15,
            }}
          >
            {p.micOn ? "🎙" : "🔇"}
          </button>
          <button
            onClick={p.onRecord}
            title="Start recording"
            style={{
              width: 46,
              height: 46,
              borderRadius: "50%",
              border: "none",
              cursor: "pointer",
              background: THEME.danger,
              boxShadow: "0 0 0 4px rgba(255,59,48,0.25)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <span
              style={{
                width: 16,
                height: 16,
                borderRadius: "50%",
                background: "#fff",
              }}
            />
          </button>
        </>
      ) : (
        <>
          <span
            title="Recording"
            style={{
              width: 12,
              height: 12,
              borderRadius: "50%",
              background: THEME.danger,
              animation: "pulse 1.2s ease-in-out infinite",
            }}
          />
          <span
            style={{
              fontFamily: "Consolas, monospace",
              fontSize: 17,
              fontWeight: 700,
              color: THEME.text,
              minWidth: 58,
            }}
          >
            {p.elapsed}
          </span>
          <button
            className="icon-btn"
            title={p.paused ? "Resume" : "Pause"}
            onClick={p.paused ? p.onResume : p.onPause}
            style={{
              width: 34,
              height: 34,
              borderRadius: "50%",
              border: `1px solid ${THEME.border}`,
              color: THEME.text,
              fontSize: 14,
            }}
          >
            {p.paused ? "▶" : "⏸"}
          </button>
          <button
            onClick={p.onStop}
            title="Stop recording"
            style={{
              width: 40,
              height: 40,
              borderRadius: "50%",
              border: "none",
              cursor: "pointer",
              background: THEME.danger,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <span
              style={{ width: 14, height: 14, background: "#fff", borderRadius: 2 }}
            />
          </button>
          <button
            className="icon-btn"
            title="Open editor"
            onClick={p.onOpenEditor}
            style={{
              width: 34,
              height: 34,
              borderRadius: "50%",
              border: `1px solid ${THEME.border}`,
              color: THEME.muted,
              fontSize: 14,
            }}
          >
            ⤢
          </button>
        </>
      )}
      <style>{`@keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.35; } }`}</style>
    </div>
  );
}

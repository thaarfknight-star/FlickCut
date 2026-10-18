import { THEME } from "../theme";

interface Props {
  recording: boolean;
}

export default function PreviewPane({ recording }: Props) {
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "#05070B",
        border: `1px solid ${THEME.border}`,
        borderRadius: 12,
        color: THEME.muted,
        fontSize: 14,
        minHeight: 320,
        position: "relative",
        overflow: "hidden",
      }}
    >
      {/* mock gameplay frame */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          background:
            "radial-gradient(ellipse at 50% 40%, #1a2333 0%, #05070B 70%)",
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          opacity: 0.15,
          backgroundImage:
            "linear-gradient(#26303F 1px, transparent 1px), linear-gradient(90deg, #26303F 1px, transparent 1px)",
          backgroundSize: "48px 48px",
        }}
      />
      <div style={{ position: "relative", textAlign: "center" }}>
        <div style={{ fontSize: 40, marginBottom: 8 }}>🎮</div>
        <div>
          {recording
            ? "Recording… live preview"
            : "Preview — press Record to start"}
        </div>
        <div style={{ fontSize: 11, color: THEME.faint, marginTop: 4 }}>
          (mock frame — real video via Tauri in the full app)
        </div>
      </div>
    </div>
  );
}

import { THEME } from "../theme";

interface Props {
  elapsed: string;
  recording: boolean;
  version: string;
}

export default function HeaderBar({ elapsed, recording, version }: Props) {
  return (
    <header
      style={{
        height: 64,
        flexShrink: 0,
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "0 16px",
        background: THEME.surface,
        borderBottom: `1px solid ${THEME.border}`,
      }}
    >
      <img
        src="logo-transparent.png"
        alt="FlickCut"
        style={{ height: 44, width: "auto" }}
      />
      <div style={{ lineHeight: 1.1 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
          <span
            style={{
              fontSize: 22,
              fontWeight: 900,
              letterSpacing: 3,
              color: THEME.text,
            }}
          >
            FlickCut
          </span>
          <span style={{ fontSize: 11, color: THEME.muted }}>v{version}</span>
        </div>
        <div
          style={{
            fontSize: 10,
            letterSpacing: 2,
            color: THEME.muted,
          }}
        >
          AUTO HIGHLIGHT EDITOR
        </div>
      </div>
      <div style={{ flex: 1 }} />
      <span
        style={{
          fontSize: 20,
          fontWeight: 700,
          fontFamily: "Consolas, monospace",
          color: THEME.text,
        }}
      >
        {elapsed}
      </span>
      <span
        style={{
          borderRadius: 12,
          padding: "6px 16px",
          fontWeight: 700,
          fontSize: 12,
          background: recording ? "#3A0F1E" : "#1A2230",
          color: recording ? THEME.danger : THEME.muted,
          border: `1px solid ${recording ? THEME.danger : THEME.border}`,
        }}
      >
        {recording ? "● REC" : "READY"}
      </span>
      <button className="btn btn-sm">Check for Updates</button>
      <button className="btn btn-sm">About</button>
    </header>
  );
}

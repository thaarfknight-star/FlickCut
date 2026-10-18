import { THEME } from "../theme";
import { EFFECT_LABELS } from "../mockData";
import type { EffectType, Region } from "../ipc/tauriApi";

interface Props {
  region: Region | null;
  recording: boolean;
  busy: boolean;
  progress: number;
  log: string[];
  onEffectChange: (e: EffectType) => void;
  onIntensityChange: (v: number) => void;
  onBoundsChange: (start: number, end: number) => void;
  onAddRegion: () => void;
  onDeleteRegion: () => void;
  onToggleRecording: () => void;
  onImport: () => void;
  onAutoEdit: () => void;
  onExport: () => void;
}

export default function SettingsDock(p: Props) {
  const r = p.region;
  return (
    <aside className="dock" style={{ width: 300 }}>
      <div className="section">REGION</div>
      <select
        className="input"
        value={r?.effect ?? "slowmo"}
        disabled={!r}
        onChange={(e) => p.onEffectChange(e.target.value as EffectType)}
      >
        {(Object.keys(EFFECT_LABELS) as EffectType[]).map((e) => (
          <option key={e} value={e}>
            {EFFECT_LABELS[e]}
          </option>
        ))}
      </select>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span className="section" style={{ padding: 0 }}>
          INTENSITY
        </span>
        <span className="section" style={{ padding: 0 }}>
          {r ? `${r.intensity}%` : "—"}
        </span>
      </div>
      <input
        type="range"
        min={0}
        max={100}
        value={r?.intensity ?? 70}
        disabled={!r}
        onChange={(e) => p.onIntensityChange(Number(e.target.value))}
        style={{ width: "100%", accentColor: THEME.accent }}
      />
      <div style={{ display: "flex", gap: 8 }}>
        <label style={{ flex: 1, fontSize: 12, color: THEME.muted }}>
          Start
          <input
            type="number"
            className="input"
            step={0.05}
            value={r?.start.toFixed(2) ?? ""}
            disabled={!r}
            onChange={(e) =>
              r && p.onBoundsChange(Number(e.target.value), r.end)
            }
          />
        </label>
        <label style={{ flex: 1, fontSize: 12, color: THEME.muted }}>
          End
          <input
            type="number"
            className="input"
            step={0.05}
            value={r?.end.toFixed(2) ?? ""}
            disabled={!r}
            onChange={(e) =>
              r && p.onBoundsChange(r.start, Number(e.target.value))
            }
          />
        </label>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <button className="btn" style={{ flex: 1 }} onClick={p.onAddRegion}>
          Add region
        </button>
        <button
          className="btn"
          style={{ flex: 1 }}
          disabled={!r}
          onClick={p.onDeleteRegion}
        >
          Delete region
        </button>
      </div>

      <div className="section" style={{ marginTop: 8 }}>
        SESSION
      </div>
      <button
        className={p.recording ? "btn btn-stop" : "btn btn-record"}
        onClick={p.onToggleRecording}
        disabled={p.busy}
      >
        {p.recording ? "■  Stop" : "●  Record"}
      </button>
      <button className="btn" onClick={p.onImport} disabled={p.busy}>
        ↓ Import video
      </button>
      <button
        className="btn btn-autoedit"
        onClick={p.onAutoEdit}
        disabled={p.busy}
      >
        ✦ Auto-Edit + Preview
      </button>
      <button className="btn btn-export" onClick={p.onExport} disabled={p.busy}>
        ↓ Export highlights
      </button>
      {p.busy && (
        <div className="progress">
          <div className="progress-fill" style={{ width: `${p.progress}%` }} />
        </div>
      )}
      <div className="log" style={{ flex: 1, minHeight: 80 }}>
        {p.log.map((line, i) => (
          <div key={i}>{line}</div>
        ))}
      </div>
    </aside>
  );
}

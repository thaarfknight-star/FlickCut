import { MOMENT_COLORS, THEME } from "../theme";
import { formatTime, describeRegion } from "../mockData";
import type { Moment, Region } from "../ipc/tauriApi";

interface Props {
  moments: Moment[];
  regions: Region[];
  selectedRegionId: string | null;
  onSelectRegion: (id: string | null) => void;
  onDeleteMoment: (id: string) => void;
  onClearMoments: () => void;
}

export default function LibraryDock({
  moments,
  regions,
  selectedRegionId,
  onSelectRegion,
  onDeleteMoment,
  onClearMoments,
}: Props) {
  return (
    <aside className="dock" style={{ width: 264 }}>
      <div className="section">MOMENTS</div>
      <div className="list" style={{ flex: 1, minHeight: 120 }}>
        {moments.map((m) => (
          <div key={m.id} className="list-row">
            <span
              className="diamond"
              style={{ background: MOMENT_COLORS[m.type] }}
            />
            <span style={{ flex: 1 }}>
              {formatTime(m.timestamp)} —{" "}
              {m.type.charAt(0).toUpperCase() + m.type.slice(1)}
            </span>
            <button
              className="icon-btn"
              title="Delete moment"
              onClick={() => onDeleteMoment(m.id)}
            >
              ✕
            </button>
          </div>
        ))}
        {moments.length === 0 && (
          <div style={{ color: THEME.faint, padding: 8 }}>No moments yet.</div>
        )}
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <button className="btn" style={{ flex: 1 }} onClick={onClearMoments}>
          Clear all
        </button>
      </div>
      <div className="section" style={{ marginTop: 8 }}>
        EDIT REGIONS
      </div>
      <div className="list" style={{ flex: 1, minHeight: 120 }}>
        {regions.map((r) => (
          <div
            key={r.id}
            className={`list-row ${r.id === selectedRegionId ? "selected" : ""}`}
            onClick={() =>
              onSelectRegion(r.id === selectedRegionId ? null : r.id)
            }
            style={{ cursor: "pointer" }}
          >
            <span style={{ flex: 1 }}>{describeRegion(r)}</span>
          </div>
        ))}
        {regions.length === 0 && (
          <div style={{ color: THEME.faint, padding: 8 }}>No regions yet.</div>
        )}
      </div>
    </aside>
  );
}

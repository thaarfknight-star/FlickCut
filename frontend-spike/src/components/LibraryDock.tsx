import { MOMENT_COLORS, THEME } from "../theme";
import { describeMarker, describeRegion } from "../mockData";
import type { Marker, Region } from "../ipc/tauriApi";

interface Props {
  markers: Marker[];
  regions: Region[];
  selectedRegion: number | null; // index into regions
  onSelectRegion: (index: number | null) => void;
  onDeleteMarker: (index: number) => void;
  onClearMarkers: () => void;
}

export default function LibraryDock({
  markers,
  regions,
  selectedRegion,
  onSelectRegion,
  onDeleteMarker,
  onClearMarkers,
}: Props) {
  return (
    <aside className="dock" style={{ width: 264 }}>
      <div className="section">MOMENTS</div>
      <div className="list" style={{ flex: 1, minHeight: 120 }}>
        {markers.map((m, i) => (
          <div key={i} className="list-row">
            <span
              className="diamond"
              style={{ background: MOMENT_COLORS[m.type] }}
            />
            <span style={{ flex: 1 }}>{describeMarker(m)}</span>
            <button
              className="icon-btn"
              title="Delete marker"
              onClick={() => onDeleteMarker(i)}
            >
              ✕
            </button>
          </div>
        ))}
        {markers.length === 0 && (
          <div style={{ color: THEME.faint, padding: 8 }}>No moments yet.</div>
        )}
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <button className="btn" style={{ flex: 1 }} onClick={onClearMarkers}>
          Clear all
        </button>
      </div>
      <div className="section" style={{ marginTop: 8 }}>
        EDIT REGIONS
      </div>
      <div className="list" style={{ flex: 1, minHeight: 120 }}>
        {regions.map((r, i) => (
          <div
            key={i}
            className={`list-row ${i === selectedRegion ? "selected" : ""}`}
            onClick={() => onSelectRegion(i === selectedRegion ? null : i)}
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

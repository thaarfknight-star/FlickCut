import { MOMENT_COLORS, REGION_COLORS, THEME } from "../theme";
import { DURATION, type Moment, type Region } from "../mockData";

interface Props {
  moments: Moment[];
  regions: Region[];
  selectedRegionId: string | null;
  playhead: number; // seconds
  onSelectRegion: (id: string | null) => void;
}

const BAR_H = 26;

export default function Timeline({
  moments,
  regions,
  selectedRegionId,
  playhead,
  onSelectRegion,
}: Props) {
  const x = (t: number) => `${(t / DURATION) * 100}%`;
  const w = (s: number, e: number) => `${((e - s) / DURATION) * 100}%`;

  return (
    <div
      className="card"
      style={{ margin: "0 12px 10px", padding: "10px 16px 14px" }}
    >
      <div className="section">TIMELINE</div>
      <div
        style={{
          position: "relative",
          height: 64,
          background: THEME.input,
          border: `1px solid ${THEME.border}`,
          borderRadius: 8,
          overflow: "hidden",
        }}
      >
        {/* region bars */}
        {regions.map((r) => {
          const selected = r.id === selectedRegionId;
          return (
            <div
              key={r.id}
              title={`${r.effect} ${r.start.toFixed(1)}s–${r.end.toFixed(1)}s`}
              onClick={() => onSelectRegion(selected ? null : r.id)}
              style={{
                position: "absolute",
                left: x(r.start),
                width: w(r.start, r.end),
                top: 19,
                height: BAR_H,
                background: REGION_COLORS[r.effect],
                opacity: selected ? 1 : 0.75,
                borderRadius: 6,
                cursor: "pointer",
                border: selected
                  ? `2px solid ${THEME.text}`
                  : "2px solid transparent",
                boxSizing: "border-box",
              }}
            />
          );
        })}
        {/* moment diamonds */}
        {moments.map((m) => (
          <div
            key={m.id}
            title={`${m.type} @ ${m.timestamp.toFixed(1)}s`}
            style={{
              position: "absolute",
              left: `calc(${x(m.timestamp)} - 6px)`,
              top: 24,
              width: 12,
              height: 12,
              background: MOMENT_COLORS[m.type],
              transform: "rotate(45deg)",
              borderRadius: 2,
              pointerEvents: "none",
            }}
          />
        ))}
        {/* playhead */}
        <div
          style={{
            position: "absolute",
            left: x(playhead),
            top: 0,
            bottom: 0,
            width: 2,
            background: THEME.text,
            pointerEvents: "none",
          }}
        />
      </div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          fontSize: 11,
          color: THEME.faint,
          marginTop: 4,
        }}
      >
        <span>0:00</span>
        <span>{DURATION.toFixed(0)}s mock video</span>
        <span>{Math.floor(DURATION / 60)}:{String(Math.floor(DURATION % 60)).padStart(2, "0")}</span>
      </div>
    </div>
  );
}

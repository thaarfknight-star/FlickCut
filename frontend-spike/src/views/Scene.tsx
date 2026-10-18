/** Game scene mock — the "capture frame" backdrop, ported from the mockup. */

export default function Scene({
  o = {},
}: {
  o?: { keys?: boolean; cam?: boolean };
}) {
  return (
    <div className="scene">
      <i />
      <div
        className="hud"
        style={{
          left: "4%",
          bottom: "6%",
          fontSize: 26,
          display: o.keys ? "none" : "block",
        }}
      >
        100 <span style={{ opacity: 0.6, fontSize: 14 }}>HP</span>
      </div>
      <div
        className="hud"
        style={{
          right: "4%",
          bottom: "6%",
          fontSize: 26,
          display: o.cam ? "none" : "block",
        }}
      >
        24 / 90
      </div>
      <div
        className="hud"
        style={{
          right: "3%",
          top: "14%",
          fontSize: 12,
          textAlign: "right",
          lineHeight: 1.6,
        }}
      >
        Player1 ▸ Player7
        <br />
        Player2 ▸ Player9
      </div>
    </div>
  );
}

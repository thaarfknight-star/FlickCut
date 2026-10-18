/** Shared mockup controls — ported 1:1 from Taha's UI/UX mockup. */

export const DUR = 90;
export const LAB = 92;

export const WALLS: Array<[string, string]> = [
  ["#1b2a4a", "#05070d"],
  ["#6d7684", "#0b0d10"],
  ["#3b1d2c", "#06060a"],
  ["#0e3a3f", "#030809"],
];

export const fmt = (s: number): string => {
  s = Math.max(0, s);
  const m = Math.floor(s / 60);
  const x = s % 60;
  return String(m).padStart(2, "0") + ":" + x.toFixed(1).padStart(4, "0");
};

export function Tg({
  on,
  set,
  label,
}: {
  on: boolean;
  set: (v: boolean) => void;
  label: string;
}) {
  return (
    <button
      className={"tg" + (on ? " on" : "")}
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => set(!on)}
    />
  );
}

export function Seg({
  v,
  set,
  opts,
}: {
  v: string;
  set: (v: string) => void;
  opts: string[];
}) {
  return (
    <div className="seg">
      {opts.map((o) => (
        <button key={o} className={o === v ? "on" : ""} onClick={() => set(o)}>
          {o}
        </button>
      ))}
    </div>
  );
}

export function Rg({
  label,
  v,
  set,
  min = 0,
  max = 100,
  unit = "px",
}: {
  label: string;
  v: number;
  set: (v: number) => void;
  min?: number;
  max?: number;
  unit?: string;
}) {
  return (
    <div style={{ marginBottom: 12 }}>
      <div className="row" style={{ margin: "0 0 2px" }}>
        <label>{label}</label>
        <output>
          {v}
          {unit}
        </output>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        value={v}
        onChange={(e) => set(+e.target.value)}
        aria-label={label}
      />
    </div>
  );
}

export function Opt({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="row">
      <label>{label}</label>
      {children}
    </div>
  );
}

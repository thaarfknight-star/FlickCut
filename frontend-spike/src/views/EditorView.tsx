import { useEffect, useState } from "react";
import Scene from "./Scene";
import { Tg, Seg, Rg, Opt, fmt, DUR, LAB, WALLS } from "./controls";
import {
  renderVideo,
  onRenderProgress,
  type Marker,
  type Region,
} from "../ipc/tauriApi";

/**
 * Editor view — frame-styled preview, inspector tabs, multi-track timeline.
 * Wired to IPC: highlights lane from analyzed markers, export via render_video.
 * Zoom clips map to contract Regions; cuts/speed clips are local edit state.
 */
export default function EditorView({
  markers,
  videoPath,
  duration,
  onLog,
}: {
  markers: Marker[];
  videoPath: string;
  duration: number;
  onLog: (line: string) => void;
}) {
  const span = Math.max(duration, 1);
  const [t, setT] = useState(0);
  const [play, setPlay] = useState(false);
  const [tab, setTab] = useState("Frame");
  const [sel, setSel] = useState<number | null>(null);
  const [z, setZ] = useState<Array<{ id: number; s: number; e: number }>>(() =>
    markers
      .filter((m) => m.type === "kill")
      .slice(0, 4)
      .map((m, i) => ({
        id: 100 + i,
        s: Math.max(0, m.timestamp - 0.6),
        e: Math.min(span, m.timestamp + 1.2),
      }))
  );
  const [cuts, setCuts] = useState<Array<{ id: number; s: number; e: number }>>(
    []
  );
  const [sp, setSp] = useState<Array<{ id: number; s: number; e: number }>>([]);
  const [wp, setWp] = useState(0);
  const [pad, setPad] = useState(28);
  const [rad, setRad] = useState(14);
  const [shd, setShd] = useState(40);
  const [ar, setAr] = useState("16:9");
  const [o, setO] = useState({ cam: true, keys: true, fps: true, auto: true });
  const [gv, setGv] = useState(85);
  const [mv, setMv] = useState(70);
  const [duck, setDuck] = useState(true);
  const [fmtX, setFmtX] = useState("MP4");
  const [q, setQ] = useState("High");
  const [prog, setProg] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!play) return;
    const i = setInterval(
      () => setT((x) => (x >= span ? (setPlay(false), span) : x + 0.1)),
      100
    );
    return () => clearInterval(i);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [play]);

  useEffect(() => {
    return onRenderProgress((p) => {
      setProg(p.percent);
      if (p.percent >= 100) {
        setBusy(false);
        onLog("Export complete.");
      }
    });
  }, [onLog]);

  const inZ = z.some((c) => t >= c.s && t <= c.e);
  const R: Record<string, number> = { "16:9": 16 / 9, "9:16": 9 / 16, "1:1": 1 };
  const add = (
    set: React.Dispatch<React.SetStateAction<Array<{ id: number; s: number; e: number }>>>,
    len: number
  ) => {
    const id = Date.now();
    set((a) => [...a, { id, s: t, e: Math.min(span, t + len) }]);
    setSel(id);
  };
  const del = () => {
    [setZ, setCuts, setSp].forEach((f) =>
      f((a) => a.filter((c) => c.id !== sel))
    );
    setSel(null);
  };
  const seek = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setT(
      Math.max(0, Math.min(span, ((e.clientX - r.left - LAB) / (r.width - LAB)) * span))
    );
  };
  const Lane = ({
    name,
    items,
    cls,
    label,
  }: {
    name: string;
    items: Array<{ id: number; s: number; e: number }>;
    cls: string;
    label: string;
  }) => (
    <div className="ln">
      <span>{name}</span>
      <div className="lane">
        {items.map((c) => (
          <button
            key={c.id}
            className={"clip " + cls + (sel === c.id ? " sel" : "")}
            style={{
              left: (c.s / span) * 100 + "%",
              width: ((c.e - c.s) / span) * 100 + "%",
            }}
            onClick={(e) => {
              e.stopPropagation();
              setSel(c.id);
            }}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
  const Wave = ({ name, seed }: { name: string; seed: number }) => (
    <div className="ln">
      <span>{name}</span>
      <div className="lane">
        <div className="wv">
          {Array.from({ length: 160 }, (_, i) => (
            <b
              key={i}
              style={{
                height:
                  18 +
                  Math.abs(Math.sin(i * seed) * Math.cos(i * 0.13)) * 80 +
                  "%",
              }}
            />
          ))}
        </div>
      </div>
    </div>
  );
  const so = (k: string) => (v: boolean) => setO({ ...o, [k]: v });
  const wl = WALLS[wp];

  const doExport = async () => {
    if (!videoPath) {
      onLog("Nothing to export — record first.");
      return;
    }
    const regions: Region[] = z.map((c) => ({
      start: c.s,
      end: c.e,
      effect: "zoomin",
      intensity: 65,
    }));
    setBusy(true);
    setProg(0);
    onLog(`Exporting ${regions.length} regions…`);
    try {
      const { output_path } = await renderVideo(videoPath, regions);
      onLog(`Exported: ${output_path}`);
    } catch (e) {
      onLog(`render_video failed: ${String(e)}`);
      setBusy(false);
    }
  };

  const highlights = markers.map((m) => ({
    t: m.timestamp,
    label: m.type.charAt(0).toUpperCase() + m.type.slice(1),
  }));

  return (
    <div className="main ed">
      <div className="pvwrap" style={{ gridArea: "pv" }}>
        <div
          className="stage"
          style={{
            ["--r" as string]: R[ar],
            background: `linear-gradient(135deg,${wl[0]},${wl[1]})`,
          }}
        >
          <div
            style={{
              position: "absolute",
              inset: pad + "px",
              borderRadius: rad,
              overflow: "hidden",
              boxShadow: `0 ${shd / 3}px ${shd}px #000d`,
            }}
          >
            <div
              className="frame"
              style={{
                transform: inZ ? "scale(1.45)" : "none",
                transformOrigin: "54% 46%",
              }}
            >
              <Scene o={o} />
            </div>
            {o.fps && <div className="fps">144 FPS</div>}
            {o.keys && (
              <div className="keys">
                <span className="w h">W</span>
                <span className="h">A</span>
                <span>S</span>
                <span>D</span>
              </div>
            )}
            {o.cam && <div className="cam" />}
          </div>
        </div>
      </div>
      <aside className="side" style={{ gridArea: "ins" }}>
        <div className="itabs">
          {["Frame", "Overlays", "Audio", "Export"].map((x) => (
            <button
              key={x}
              className={tab === x ? "on" : ""}
              onClick={() => setTab(x)}
            >
              {x}
            </button>
          ))}
        </div>
        {tab === "Frame" && (
          <>
            <h3>Background</h3>
            <div className="sw">
              {WALLS.map((w, i) => (
                <button
                  key={i}
                  aria-label={"Background " + (i + 1)}
                  className={wp === i ? "on" : ""}
                  style={{
                    background: `linear-gradient(135deg,${w[0]},${w[1]})`,
                  }}
                  onClick={() => setWp(i)}
                />
              ))}
            </div>
            <Rg label="Padding" v={pad} set={setPad} max={80} />
            <Rg label="Corner radius" v={rad} set={setRad} max={40} />
            <Rg label="Shadow" v={shd} set={setShd} />
            <Opt label="Aspect ratio">
              <Seg v={ar} set={setAr} opts={["16:9", "9:16", "1:1"]} />
            </Opt>
          </>
        )}
        {tab === "Overlays" && (
          <>
            <Opt label="Auto highlights">
              <Tg on={o.auto} set={so("auto")} label="Auto highlights" />
            </Opt>
            <Opt label="Webcam bubble">
              <Tg on={o.cam} set={so("cam")} label="Webcam" />
            </Opt>
            <Opt label="Key and mouse display">
              <Tg on={o.keys} set={so("keys")} label="Keys" />
            </Opt>
            <Opt label="FPS counter">
              <Tg on={o.fps} set={so("fps")} label="FPS" />
            </Opt>
            <p style={{ color: "var(--mut)", fontSize: 13 }}>
              Auto highlights places a marker on every audio spike and hit
              sound, so you can jump between kills.
            </p>
          </>
        )}
        {tab === "Audio" && (
          <>
            <Rg label="Game audio" v={gv} set={setGv} unit="%" />
            <Rg label="Microphone" v={mv} set={setMv} unit="%" />
            <Opt label="Lower game when I talk">
              <Tg on={duck} set={setDuck} label="Ducking" />
            </Opt>
          </>
        )}
        {tab === "Export" && (
          <>
            <Opt label="Format">
              <Seg v={fmtX} set={setFmtX} opts={["MP4", "GIF"]} />
            </Opt>
            <Opt label="Quality">
              <Seg v={q} set={setQ} opts={["Good", "High", "Max"]} />
            </Opt>
            <h3>Presets</h3>
            {[
              ["YouTube", "1080p · 60 fps", "16:9"],
              ["Shorts and TikTok", "1080×1920 · 60 fps", "9:16"],
              ["Square clip", "1080×1080 · 30 fps", "1:1"],
            ].map((p) => (
              <button
                key={p[0]}
                className={"pre" + (ar === p[2] ? " on" : "")}
                onClick={() => setAr(p[2])}
              >
                <b>{p[0]}</b>
                <small>{p[1]}</small>
              </button>
            ))}
            <button
              className="btn c"
              style={{ width: "100%", marginTop: 8 }}
              onClick={doExport}
              disabled={busy}
            >
              {prog === null
                ? "Export clip"
                : prog < 100
                  ? "Exporting " + prog + "%"
                  : "Exported · Show in folder"}
            </button>
          </>
        )}
      </aside>
      <div className="tl">
        <div className="tbar">
          <button className="btn c sm" onClick={() => setPlay(!play)}>
            {play ? "Pause" : "Play"}
          </button>
          <span className="t">{fmt(t)}</span>
          <button className="btn sm" onClick={() => add(setZ, 5)}>
            Add zoom
          </button>
          <button className="btn sm" onClick={() => add(setCuts, 4)}>
            Cut section
          </button>
          <button className="btn sm" onClick={() => add(setSp, 6)}>
            Change speed
          </button>
          <button
            className="btn sm"
            style={{ opacity: sel ? 1 : 0.4 }}
            disabled={!sel}
            onClick={del}
          >
            Delete
          </button>
        </div>
        <div className="lanes">
          <div className="in" onClick={seek}>
            <div className="ruler" />
            <div className="ln">
              <span>Highlights</span>
              <div className="lane">
                {o.auto &&
                  highlights.map((h) => (
                    <button
                      key={h.t}
                      className="hl"
                      title={h.label}
                      aria-label={h.label}
                      style={{ left: (h.t / span) * 100 + "%" }}
                      onClick={(e) => {
                        e.stopPropagation();
                        setT(h.t);
                      }}
                    />
                  ))}
              </div>
            </div>
            <Lane name="Zoom" items={z} cls="" label="Zoom 1.45×" />
            <Lane name="Cuts" items={cuts} cls="cut" label="Cut" />
            <Lane name="Speed" items={sp} cls="sp" label="0.5×" />
            <Wave name="Game audio" seed={1.7} />
            <Wave name="Microphone" seed={3.1} />
            <div
              className="ph"
              style={{ left: `calc(${LAB}px + (100% - ${LAB}px) * ${t / span})` }}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

// keep DUR referenced (timeline scale constant from the mockup)
export { DUR };

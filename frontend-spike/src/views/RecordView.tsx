import { useEffect, useState } from "react";
import Scene from "./Scene";
import { Tg, Seg, Opt, fmt } from "./controls";
import {
  listSources,
  startRecording,
  stopRecording,
  analyzeMoments,
  onRecordingTick,
} from "../ipc/tauriApi";

/**
 * Record view — source sidebar, preview with REC badge, quality/audio/
 * replay settings. Wired to IPC contract v1.1 for record/stop.
 * Quality + replay-buffer settings are UI-only (no contract command yet).
 */
export default function RecordView({
  onSession,
  onLog,
}: {
  onSession: (videoPath: string, durationSec: number, sessionId: string) => void;
  onLog: (line: string) => void;
}) {
  const [sources, setSources] = useState<
    Array<{ id: string; name: string; sub: string; det?: boolean }>
  >([]);
  const [src, setSrc] = useState("");
  const [on, setOn] = useState(false);
  const [sec, setSec] = useState(0);
  const [buf, setBuf] = useState(true);
  const [res, setRes] = useState("1440p");
  const [fps, setFps] = useState("144");
  const [mic, setMic] = useState(true);
  const [gm, setGm] = useState(true);
  const [sep, setSep] = useState(true);
  const [len, setLen] = useState("30s");

  useEffect(() => {
    listSources()
      .then((s) => {
        const list = [
          ...s.screens.map((x) => ({
            id: x.id,
            name: x.name,
            sub: "Full screen",
            det: false,
          })),
          ...s.windows.map((x) => ({
            id: x.id,
            name: x.title,
            sub: "Capture any app",
            det: false,
          })),
        ];
        setSources(list);
        if (list.length > 0) setSrc(list[0].id);
      })
      .catch((e) => onLog(`list_sources failed: ${String(e)}`));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!on) return;
    return onRecordingTick((t) => setSec(t.elapsed_sec));
  }, [on]);

  const start = async () => {
    try {
      const { session_id } = await startRecording(src);
      (window as unknown as { __fc_session: string }).__fc_session = session_id;
      setOn(true);
      setSec(0);
      onLog(`Recording started (${session_id}).`);
    } catch (e) {
      onLog(`start_recording failed: ${String(e)}`);
    }
  };

  const stop = async () => {
    try {
      const { video_path, duration_sec } = await stopRecording();
      const sid = (window as unknown as { __fc_session: string }).__fc_session;
      setOn(false);
      setSec(0);
      onLog(`Recording stopped — ${duration_sec}s.`);
      try {
        const { markers } = await analyzeMoments(sid);
        (window as unknown as { __fc_markers: unknown }).__fc_markers = markers;
        onLog(`Analyzed: ${markers.length} markers.`);
      } catch (e) {
        onLog(`analyze_moments failed: ${String(e)}`);
      }
      onSession(video_path, duration_sec, sid);
    } catch (e) {
      onLog(`stop_recording failed: ${String(e)}`);
    }
  };

  return (
    <div className="main rec">
      <aside className="side">
        <h3>Capture source</h3>
        {sources.map((s) => (
          <button
            key={s.id}
            className={"src" + (src === s.id ? " on" : "")}
            onClick={() => setSrc(s.id)}
          >
            <b>{s.name}</b>
            <small className={s.det ? "det" : ""}>{s.sub}</small>
          </button>
        ))}
        <h3 style={{ marginTop: 18 }}>Recent</h3>
        {["clutch-round-12", "ranked-night-3"].map((n) => (
          <button key={n} className="src">
            <b>{n}</b>
            <small>Open project</small>
          </button>
        ))}
      </aside>
      <section
        style={{ display: "grid", gridTemplateRows: "1fr auto", minHeight: 0 }}
      >
        <div className={"pvwrap" + (on ? " live" : "")}>
          <div className="stage" style={{ ["--r" as string]: 16 / 9 }}>
            <div className={"badge" + (on ? " live" : "")}>
              <span className="dot" />
              {on ? "REC " + fmt(sec).slice(0, 5) : "Ready"}
            </div>
            <div className="frame">
              <Scene />
            </div>
          </div>
        </div>
        <div className="ctl">
          <button className="btn">
            Mark highlight{" "}
            <small style={{ color: "var(--mut)" }}>F9</small>
          </button>
          <button
            className={"bullet" + (on ? " stop" : "")}
            onClick={on ? stop : start}
            aria-label={on ? "Stop recording" : "Start recording"}
          >
            <i />
          </button>
          <button
            className="btn"
            disabled={!buf}
            style={{ opacity: buf ? 1 : 0.4 }}
          >
            Save last {len}{" "}
            <small style={{ color: "var(--mut)" }}>F10</small>
          </button>
        </div>
      </section>
      <aside className="side">
        <h3>Quality</h3>
        <Opt label="Resolution">
          <Seg v={res} set={setRes} opts={["1080p", "1440p", "4K"]} />
        </Opt>
        <Opt label="Frame rate">
          <Seg v={fps} set={setFps} opts={["60", "120", "144"]} />
        </Opt>
        <h3 style={{ marginTop: 18 }}>Audio</h3>
        <Opt label="Game audio">
          <Tg on={gm} set={setGm} label="Game audio" />
        </Opt>
        <Opt label="Microphone">
          <Tg on={mic} set={setMic} label="Microphone" />
        </Opt>
        <Opt label="Separate tracks">
          <Tg on={sep} set={setSep} label="Separate tracks" />
        </Opt>
        <h3 style={{ marginTop: 18 }}>Instant replay</h3>
        <Opt label="Keep buffering">
          <Tg on={buf} set={setBuf} label="Replay buffer" />
        </Opt>
        <Opt label="Length">
          <Seg v={len} set={setLen} opts={["15s", "30s", "60s"]} />
        </Opt>
      </aside>
    </div>
  );
}

import { useEffect, useState } from "react";
import Scene from "./Scene";
import {
  listSources,
  onRecordingTick,
  pauseRecording,
  resumeRecording,
  startRecording,
  stopRecording,
  analyzeMoments,
} from "../ipc/tauriApi";

/**
 * Floating bar view — priority 1. In Tauri this is the `control-bar`
 * window (decorations: false, alwaysOnTop: true, bottom-center).
 * Here it floats over a fake desktop.
 *
 * Wired to IPC contract v1.1. "Mark highlight" (F9) and "Save last 30s"
 * (F10) are UI-only for now — no contract command covers them yet.
 */
export default function BarView({
  onSession,
  onLog,
}: {
  onSession: (videoPath: string, durationSec: number, sessionId: string) => void;
  onLog: (line: string) => void;
}) {
  const [st, setSt] = useState<"idle" | "count" | "rec">("idle");
  const [n, setN] = useState(3);
  const [sec, setSec] = useState(0);
  const [pause, setPause] = useState(false);
  const [menu, setMenu] = useState(false);
  const [sources, setSources] = useState<Array<{ id: string; label: string }>>([]);
  const [src, setSrc] = useState(0);
  const [gm, setGm] = useState(true);
  const [mic, setMic] = useState(true);
  const [cam, setCam] = useState(false);
  const [rb, setRb] = useState(true);
  const [mk, setMk] = useState(0);

  useEffect(() => {
    listSources()
      .then((s) => {
        const opts = [
          ...s.screens.map((x) => ({ id: x.id, label: x.name })),
          ...s.windows.map((x) => ({ id: x.id, label: x.title })),
        ];
        setSources(opts);
      })
      .catch((e) => onLog(`list_sources failed: ${String(e)}`));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // countdown → start recording via IPC
  useEffect(() => {
    if (st !== "count") return;
    if (n === 0) {
      const sid = sources[src]?.id ?? "screen-0";
      startRecording(sid)
        .then(({ session_id }) => {
          onLog(`Recording started (${session_id}) on ${sid}.`);
          (window as unknown as { __fc_session: string }).__fc_session =
            session_id;
          setSt("rec");
          setSec(0);
          setMk(0);
        })
        .catch((e) => {
          onLog(`start_recording failed: ${String(e)}`);
          setSt("idle");
        });
      return;
    }
    const i = setTimeout(() => setN(n - 1), 800);
    return () => clearTimeout(i);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [st, n]);

  // elapsed via contract event
  useEffect(() => {
    if (st !== "rec") return;
    return onRecordingTick((t) => setSec(t.elapsed_sec));
  }, [st]);

  const mm = (s: number) =>
    String(Math.floor(s / 60)).padStart(2, "0") +
    ":" +
    String(s % 60).padStart(2, "0");

  const doPause = async () => {
    try {
      const { paused_at_sec } = await pauseRecording();
      setPause(true);
      onLog(`Paused at ${paused_at_sec}s.`);
    } catch (e) {
      onLog(`pause failed: ${String(e)}`);
    }
  };
  const doResume = async () => {
    try {
      await resumeRecording();
      setPause(false);
      onLog("Resumed.");
    } catch (e) {
      onLog(`resume failed: ${String(e)}`);
    }
  };

  const doStop = async () => {
    try {
      const { video_path, duration_sec } = await stopRecording();
      const sid = (window as unknown as { __fc_session: string }).__fc_session;
      setSt("idle");
      setPause(false);
      onLog(`Recording stopped — ${duration_sec}s.`);
      // analyze, then hand the session to the editor
      try {
        const { markers } = await analyzeMoments(sid);
        onLog(`Analyzed: ${markers.length} markers.`);
        (window as unknown as { __fc_markers: unknown }).__fc_markers = markers;
      } catch (e) {
        onLog(`analyze_moments failed: ${String(e)}`);
      }
      onSession(video_path, duration_sec, sid);
    } catch (e) {
      onLog(`stop_recording failed: ${String(e)}`);
    }
  };

  // F9 = mark highlight, F10 = save last 30s (UI-only, no contract command yet)
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (st !== "rec") return;
      if (e.key === "F9") {
        e.preventDefault();
        setMk((m) => m + 1);
        onLog(`Highlight marked @ ${mm(sec)} (F9).`);
      } else if (e.key === "F10") {
        e.preventDefault();
        onLog(`Save last 30s @ ${mm(sec)} (F10) — replay buffer ${rb ? "on" : "off"}.`);
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [st, sec, rb]);

  const S = sources.length > 0 ? sources : [{ id: "screen-0", label: "Display 1" }];

  return (
    <div className="main">
      <div className="desk">
        <div className="win">
          <Scene />
        </div>
        <div className="win win2">
          <b style={{ color: "var(--tx)" }}>Desktop</b>
          <br />
          This is how the bar floats above your game. It stays out of the
          recording.
        </div>
        {st === "count" && (
          <div className="cd">
            <span>{n}</span>
          </div>
        )}
        <div className="bar">
          {menu && st === "idle" && (
            <div className="menu">
              {S.map((s, i) => (
                <button
                  key={s.id}
                  className={i === src ? "on" : ""}
                  onClick={() => {
                    setSrc(i);
                    setMenu(false);
                  }}
                >
                  <b>{s.label}</b>
                  <small>{i === 0 ? "Default" : "Capture target"}</small>
                </button>
              ))}
            </div>
          )}
          {st === "idle" && (
            <>
              <button
                className="chip"
                style={{ color: "var(--tx)" }}
                onClick={() => setMenu(!menu)}
                aria-expanded={menu}
              >
                {S[src]?.label ?? "Source"} ▾
              </button>
              <span className="vr" />
              <button className={"chip" + (gm ? " on" : "")} onClick={() => setGm(!gm)}>
                Game audio
              </button>
              <button className={"chip" + (mic ? " on" : "")} onClick={() => setMic(!mic)}>
                Mic
              </button>
              <button className={"chip" + (cam ? " on" : "")} onClick={() => setCam(!cam)}>
                Webcam
              </button>
              <span className="vr" />
              <button className={"chip" + (rb ? " on" : "")} onClick={() => setRb(!rb)}>
                Replay 30s
              </button>
              <button className="chip" style={{ color: "var(--tx)" }}>
                1440p · 144
              </button>
              <button
                className="bullet"
                style={{ marginLeft: 8 }}
                aria-label="Start recording"
                onClick={() => {
                  setN(3);
                  setSt("count");
                }}
              >
                <i />
              </button>
            </>
          )}
          {st === "count" && (
            <>
              <span className="chip" style={{ color: "var(--tx)" }}>
                Recording {S[src]?.label} in {n}
              </span>
              <button className="chip" onClick={() => setSt("idle")}>
                Cancel
              </button>
            </>
          )}
          {st === "rec" && (
            <>
              <span
                className={"chip" + (pause ? "" : " live")}
                style={{ color: "var(--tx)", minWidth: 104 }}
              >
                <span className="dot" />
                {pause ? "Paused " : "REC "}
                {mm(sec)}
              </span>
              <button className="chip" onClick={() => (pause ? doResume() : doPause())}>
                {pause ? "Resume" : "Pause"}
              </button>
              <button className="chip" onClick={() => setMk(mk + 1)}>
                Mark highlight{mk ? " (" + mk + ")" : ""}
              </button>
              <button className="chip" disabled={!rb} style={{ opacity: rb ? 1 : 0.4 }}>
                Save last 30s
              </button>
              <span className="vr" />
              <button
                className="chip"
                onClick={() => {
                  setSt("idle");
                  setPause(false);
                }}
              >
                Discard
              </button>
              <button
                className="bullet stop"
                style={{ marginLeft: 8 }}
                aria-label="Stop recording"
                onClick={doStop}
              >
                <i />
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

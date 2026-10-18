import { useState } from "react";
import RecordView from "./views/RecordView";
import EditorView from "./views/EditorView";
import BarView from "./views/BarView";
import type { Marker } from "./ipc/tauriApi";

/**
 * App shell — top tabs switch the three mockup views.
 * In the real Tauri app the floating bar is a separate window
 * (see WINDOWS_SPEC.md); here tabs simulate it for the demo.
 */
type V = "rec" | "ed" | "bar";

export default function App() {
  const [v, setV] = useState<V>("bar");
  const [videoPath, setVideoPath] = useState("");
  const [duration, setDuration] = useState(0);
  const [markers, setMarkers] = useState<Marker[]>([]);
  const [log, setLog] = useState<string[]>(["FlickCut — mockup port."]);
  const addLog = (line: string) => setLog((l) => [...l.slice(-49), line]);

  const onSession = (
    vpath: string,
    dur: number,
    _sid: string
  ) => {
    setVideoPath(vpath);
    setDuration(dur);
    const ms = (window as unknown as { __fc_markers?: Marker[] }).__fc_markers;
    setMarkers(Array.isArray(ms) ? ms : []);
    setV("ed");
  };

  return (
    <div className="app">
      <header className="top">
        <div className="brand">
          <img src="logo-transparent.png" alt="" />
          <span className="wm">FlickCut</span>
        </div>
        <nav>
          <button className={"tab" + (v === "rec" ? " on" : "")} onClick={() => setV("rec")}>
            Record
          </button>
          <button className={"tab" + (v === "ed" ? " on" : "")} onClick={() => setV("ed")}>
            Editor
          </button>
          <button className={"tab" + (v === "bar" ? " on" : "")} onClick={() => setV("bar")}>
            Floating bar
          </button>
        </nav>
        <span className="sp" />
        <span className="proj">
          {videoPath ? videoPath.split("/").pop() : "clutch-round-12.flick"}
        </span>
        <button className="btn c" onClick={() => setV("ed")}>
          Export
        </button>
      </header>
      {v === "rec" ? (
        <RecordView onSession={onSession} onLog={addLog} />
      ) : v === "bar" ? (
        <BarView onSession={onSession} onLog={addLog} />
      ) : (
        <EditorView
          markers={markers}
          videoPath={videoPath}
          duration={duration || 90}
          onLog={addLog}
        />
      )}
      {/* event log (demo aid) */}
      <div
        title="IPC log"
        style={{
          position: "fixed",
          right: 10,
          bottom: 10,
          width: 300,
          maxHeight: 150,
          overflow: "auto",
          background: "rgba(11,13,16,0.9)",
          border: "1px solid var(--line)",
          fontSize: 11,
          padding: 8,
          zIndex: 60,
          display: log.length > 1 ? "block" : "none",
        }}
      >
        {log.slice(-8).map((l, i) => (
          <div key={i} style={{ color: "var(--mut)" }}>
            {l}
          </div>
        ))}
      </div>
    </div>
  );
}

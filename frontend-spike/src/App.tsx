import { useEffect, useState } from "react";
import FloatingBar from "./components/FloatingBar";
import HeaderBar from "./components/HeaderBar";
import LibraryDock from "./components/LibraryDock";
import PreviewPane from "./components/PreviewPane";
import SettingsDock from "./components/SettingsDock";
import Timeline from "./components/Timeline";
import {
  analyzeMoments,
  importVideo,
  isRunningInTauri,
  listSources,
  onInputEvent,
  onRecordingTick,
  onRenderProgress,
  pauseRecording,
  renderVideo,
  resumeRecording,
  startRecording,
  stopRecording,
  type EffectType,
  type Marker,
  type Region,
} from "./ipc/tauriApi";
import { formatTime, regionsFromMarkers } from "./mockData";

type View = "bar" | "editor";

export default function App() {
  const [view, setView] = useState<View>("bar");
  const [markers, setMarkers] = useState<Marker[]>([]);
  const [regions, setRegions] = useState<Region[]>([]);
  const [selectedRegion, setSelectedRegion] = useState<number | null>(null);
  const [sourceOptions, setSourceOptions] = useState<
    Array<{ id: string; label: string }>
  >([]);
  const [sourceId, setSourceId] = useState<string>("");
  const [sessionId, setSessionId] = useState<string>("");
  const [videoPath, setVideoPath] = useState<string>("");
  const [duration, setDuration] = useState(0);
  const [recording, setRecording] = useState(false);
  const [paused, setPaused] = useState(false);
  const [micOn, setMicOn] = useState(true);
  const [elapsed, setElapsed] = useState(0);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState(0);
  const [log, setLog] = useState<string[]>([
    `FlickCut — contract v1, ${isRunningInTauri() ? "Tauri shell" : "mock backend"}.`,
  ]);

  const addLog = (line: string) => setLog((l) => [...l.slice(-49), line]);

  useEffect(() => {
    listSources()
      .then((s) => {
        const opts = [
          ...s.screens.map((x) => ({ id: x.id, label: `🖥 ${x.name}` })),
          ...s.windows.map((x) => ({ id: x.id, label: `🪟 ${x.title}` })),
        ];
        setSourceOptions(opts);
        if (opts.length > 0) setSourceId(opts[0].id);
        addLog(`Sources: ${opts.map((o) => o.label).join(", ")}.`);
      })
      .catch((e) => addLog(`list_sources failed: ${String(e)}`));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!recording) return;
    const offTick = onRecordingTick((t) => setElapsed(t.elapsed_sec));
    const offInput = onInputEvent((e) =>
      addLog(`input: ${e.source}/${e.action} @ ${(e.t_ms / 1000).toFixed(1)}s`)
    );
    return () => {
      offTick();
      offInput();
    };
  }, [recording]);

  useEffect(() => {
    return onRenderProgress((p) => {
      setProgress(p.percent);
      if (p.percent >= 100) setBusy(false);
    });
  }, []);

  const region = selectedRegion != null ? regions[selectedRegion] : null;

  const patchRegion = (index: number, patch: Partial<Region>) =>
    setRegions((rs) => rs.map((r, i) => (i === index ? { ...r, ...patch } : r)));

  const finishSession = async (sid: string, vpath: string, dur: number) => {
    setVideoPath(vpath);
    setDuration(dur);
    try {
      const { markers: ms } = await analyzeMoments(sid);
      setMarkers(ms);
      const rs = regionsFromMarkers(ms, dur);
      setRegions(rs);
      setSelectedRegion(rs.length > 0 ? 0 : null);
      addLog(`Analyzed: ${ms.length} markers, ${rs.length} regions suggested.`);
    } catch (e) {
      addLog(`analyze_moments failed: ${String(e)}`);
    }
  };

  const handleRecord = async () => {
    if (busy) return;
    try {
      const { session_id } = await startRecording(sourceId);
      setSessionId(session_id);
      setRecording(true);
      setPaused(false);
      setElapsed(0);
      setMarkers([]);
      setRegions([]);
      setSelectedRegion(null);
      addLog(`Recording started (${session_id}) on ${sourceId}.`);
    } catch (e) {
      addLog(`start_recording failed: ${String(e)}`);
    }
  };

  const handleStop = async () => {
    try {
      const { video_path, duration_sec } = await stopRecording();
      const sid = sessionId;
      setRecording(false);
      setPaused(false);
      setSessionId("");
      addLog(`Recording stopped — ${duration_sec}s saved.`);
      await finishSession(sid, video_path, duration_sec);
      // Recordly flow: stopping opens the editor
      setView("editor");
    } catch (e) {
      addLog(`stop_recording failed: ${String(e)}`);
    }
  };

  const handlePause = async () => {
    try {
      await pauseRecording();
      setPaused(true);
      addLog("Recording paused.");
    } catch (e) {
      addLog(`pause failed: ${String(e)}`);
    }
  };

  const handleResume = async () => {
    try {
      await resumeRecording();
      setPaused(false);
      addLog("Recording resumed.");
    } catch (e) {
      addLog(`resume failed: ${String(e)}`);
    }
  };

  const runRender = async (label: string) => {
    if (!videoPath) {
      addLog("Nothing to render — record or import first.");
      return;
    }
    if (regions.length === 0) {
      addLog("No regions — they are suggested automatically after recording.");
      return;
    }
    setBusy(true);
    setProgress(0);
    addLog(label);
    try {
      const { output_path } = await renderVideo(videoPath, regions);
      addLog(`Done: ${output_path}`);
    } catch (e) {
      addLog(`render_video failed: ${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  /* -- floating-bar view (the launch UI, Recordly-style) ------------- */
  if (view === "bar") {
    return (
      <div
        style={{
          height: "100vh",
          overflow: "hidden",
          position: "relative",
          background:
            "radial-gradient(1200px 700px at 50% 20%, #141b28 0%, #0A0C10 70%)",
        }}
      >
        {/* fake desktop backdrop so the floating bar reads as floating */}
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 12,
            color: "#6d717b",
            userSelect: "none",
          }}
        >
          <img
            src="logo-transparent.png"
            alt=""
            style={{ height: 72, opacity: 0.35 }}
          />
          <div style={{ fontSize: 15, letterSpacing: 1 }}>
            {recording
              ? "Recording… use the bar below to pause or stop"
              : "FlickCut is running — hit record when the action starts"}
          </div>
          {!recording && videoPath && (
            <button className="btn" onClick={() => setView("editor")}>
              Open editor
            </button>
          )}
        </div>
        <FloatingBar
          recording={recording}
          paused={paused}
          elapsed={formatTime(elapsed)}
          sources={sourceOptions}
          sourceId={sourceId}
          micOn={micOn}
          onSourceChange={setSourceId}
          onMicToggle={() => setMicOn((m) => !m)}
          onRecord={handleRecord}
          onPause={handlePause}
          onResume={handleResume}
          onStop={handleStop}
          onOpenEditor={() => setView("editor")}
        />
      </div>
    );
  }

  /* -- editor view (opens after stop, Recordly flow) ------------------ */
  return (
    <div
      style={{
        height: "100vh",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      <HeaderBar
        elapsed={formatTime(elapsed)}
        recording={recording}
        version="0.3.0-tauri"
        onMinimize={() => setView("bar")}
      />
      <div
        style={{ flex: 1, display: "flex", gap: 10, padding: 12, minHeight: 0 }}
      >
        <LibraryDock
          markers={markers}
          regions={regions}
          selectedRegion={selectedRegion}
          onSelectRegion={setSelectedRegion}
          onDeleteMarker={(i) => {
            setMarkers((ms) => ms.filter((_, j) => j !== i));
            addLog("Marker deleted.");
          }}
          onClearMarkers={() => {
            setMarkers([]);
            addLog("All markers cleared.");
          }}
        />
        <div
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            gap: 8,
            minWidth: 0,
          }}
        >
          <PreviewPane recording={false} />
          <div
            style={{
              fontSize: 11,
              color: "#6d717b",
              textAlign: "center",
            }}
          >
            {videoPath ? `Editing ${videoPath}` : "Record from the bar to begin"}
          </div>
        </div>
        <SettingsDock
          region={region}
          busy={busy}
          progress={progress}
          log={log}
          onEffectChange={(e: EffectType) => {
            if (selectedRegion != null) patchRegion(selectedRegion, { effect: e });
          }}
          onIntensityChange={(v) => {
            if (selectedRegion != null)
              patchRegion(selectedRegion, { intensity: v });
          }}
          onBoundsChange={(start, end) => {
            if (selectedRegion != null && end > start && start >= 0)
              patchRegion(selectedRegion, { start, end });
          }}
          onAddRegion={() => {
            const c = elapsed > 0 ? elapsed : duration / 2 || 30;
            const nr: Region = {
              start: Math.max(0, c - 0.6),
              end: Math.min(duration || 120, c + 0.6),
              effect: "slowmo",
              intensity: 70,
            };
            setRegions((rs) => {
              const next = [...rs, nr].sort((a, b) => a.start - b.start);
              setSelectedRegion(next.indexOf(nr));
              return next;
            });
            addLog(`Region added: ${nr.start.toFixed(1)}s–${nr.end.toFixed(1)}s.`);
          }}
          onDeleteRegion={() => {
            if (selectedRegion == null) return;
            setRegions((rs) => rs.filter((_, i) => i !== selectedRegion));
            setSelectedRegion(null);
            addLog("Region deleted.");
          }}
          onToggleRecording={() => {
            // in the editor, recording controls live on the bar
            setView("bar");
            addLog("Use the floating bar to record.");
          }}
          onImport={async () => {
            try {
              const { session_id: sid } = await importVideo("/mock/import.mp4");
              addLog(`Import: session ${sid}.`);
              await finishSession(sid, "/mock/import.mp4", 124.5);
            } catch (e) {
              addLog(`import_video failed: ${String(e)}`);
            }
          }}
          onAutoEdit={() => runRender("Auto-Edit: rendering regions…")}
          onExport={() => runRender("Export: rendering highlights…")}
        />
      </div>
      <Timeline
        markers={markers}
        regions={regions}
        duration={duration || 124.5}
        selectedRegion={selectedRegion}
        playhead={elapsed}
        onSelectRegion={setSelectedRegion}
      />
    </div>
  );
}

import { useEffect, useState } from "react";
import HeaderBar from "./components/HeaderBar";
import LibraryDock from "./components/LibraryDock";
import PreviewPane from "./components/PreviewPane";
import SettingsDock from "./components/SettingsDock";
import Timeline from "./components/Timeline";
import {
  detectMoments,
  exportHighlights,
  isRunningInTauri,
  listSources,
  onMomentDetected,
  onRenderProgress,
  renderPreview,
  startRecording,
  stopRecording,
  type CaptureSource,
  type EffectType,
  type Moment,
  type Region,
} from "./ipc/tauriApi";
import {
  DURATION,
  formatTime,
  regionsFromMoments,
} from "./mockData";

let nextId = 100;

export default function App() {
  const [moments, setMoments] = useState<Moment[]>([]);
  const [regions, setRegions] = useState<Region[]>([]);
  const [selectedRegionId, setSelectedRegionId] = useState<string | null>(null);
  const [sources, setSources] = useState<CaptureSource[]>([]);
  const [sourceId, setSourceId] = useState<string>("");
  const [videoPath, setVideoPath] = useState<string>("");
  const [recording, setRecording] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [playhead, setPlayhead] = useState(0);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState(0);
  const [log, setLog] = useState<string[]>([
    `FlickCut editor — ${isRunningInTauri() ? "Tauri shell" : "mock backend"}.`,
  ]);

  const addLog = (line: string) => setLog((l) => [...l.slice(-49), line]);

  // boot: capture sources
  useEffect(() => {
    listSources()
      .then((ss) => {
        setSources(ss);
        if (ss.length > 0) setSourceId(ss[0].id);
        addLog(`Sources: ${ss.map((s) => s.name).join(", ")}.`);
      })
      .catch((e) => addLog(`listSources failed: ${String(e)}`));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // live moments while recording
  useEffect(() => {
    if (!recording) return;
    return onMomentDetected((m) => {
      setMoments((ms) => [...ms, m]);
      addLog(`Moment: ${m.type} @ ${m.timestamp.toFixed(1)}s.`);
    });
  }, [recording]);

  // render progress events
  useEffect(() => {
    return onRenderProgress((p) => {
      setProgress(p.progress);
      if (p.progress >= 100) setBusy(false);
    });
  }, []);

  // elapsed timer (mock clock; the shell will drive this via events later)
  useEffect(() => {
    if (!recording) return;
    const iv = window.setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => window.clearInterval(iv);
  }, [recording]);

  const selectedRegion =
    regions.find((r) => r.id === selectedRegionId) ?? null;

  const patchRegion = (id: string, patch: Partial<Region>) =>
    setRegions((rs) => rs.map((r) => (r.id === id ? { ...r, ...patch } : r)));

  const toggleRecording = async () => {
    if (busy) return;
    if (recording) {
      try {
        const summary = await stopRecording();
        setRecording(false);
        setVideoPath(summary.video_path);
        setMoments(summary.moments);
        setRegions(regionsFromMoments(summary.moments, summary.duration_s));
        setPlayhead(0);
        addLog(
          `Recording stopped — ${summary.duration_s}s, ` +
            `${summary.moments.length} moments.`
        );
      } catch (e) {
        addLog(`stopRecording failed: ${String(e)}`);
      }
      return;
    }
    try {
      const { session_id } = await startRecording(sourceId);
      setRecording(true);
      setElapsed(0);
      setMoments([]);
      setRegions([]);
      setSelectedRegionId(null);
      addLog(`Recording started (${session_id}) — capturing ${sourceId}.`);
    } catch (e) {
      addLog(`startRecording failed: ${String(e)}`);
    }
  };

  const runJob = async (
    label: string,
    fn: () => Promise<{ preview_path?: string; output_path?: string }>
  ) => {
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
      const out = await fn();
      addLog(`Done: ${out.preview_path ?? out.output_path} (mock).`);
    } catch (e) {
      addLog(`Failed: ${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

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
      />
      <div
        style={{ flex: 1, display: "flex", gap: 10, padding: 12, minHeight: 0 }}
      >
        <LibraryDock
          moments={moments}
          regions={regions}
          selectedRegionId={selectedRegionId}
          onSelectRegion={setSelectedRegionId}
          onDeleteMoment={(id) => {
            setMoments((ms) => ms.filter((m) => m.id !== id));
            addLog("Moment deleted.");
          }}
          onClearMoments={() => {
            setMoments([]);
            addLog("All moments cleared.");
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
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <span style={{ fontSize: 12, color: "#a1a8b6" }}>Source</span>
            <select
              className="input"
              style={{ maxWidth: 280 }}
              value={sourceId}
              onChange={(e) => setSourceId(e.target.value)}
              disabled={recording}
            >
              {sources.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </div>
          <PreviewPane recording={recording} />
        </div>
        <SettingsDock
          region={selectedRegion}
          recording={recording}
          busy={busy}
          progress={progress}
          log={log}
          onEffectChange={(e: EffectType) => {
            if (selectedRegionId) patchRegion(selectedRegionId, { effect: e });
          }}
          onIntensityChange={(v) => {
            if (selectedRegionId)
              patchRegion(selectedRegionId, { intensity: v });
          }}
          onBoundsChange={(start, end) => {
            if (selectedRegionId && end > start && start >= 0)
              patchRegion(selectedRegionId, { start, end });
          }}
          onAddRegion={() => {
            const id = `r${nextId++}`;
            const c = playhead > 0 ? playhead : DURATION / 2;
            const nr: Region = {
              id,
              start: Math.max(0, c - 0.6),
              end: Math.min(DURATION, c + 0.6),
              effect: "slowmo",
              intensity: 70,
            };
            setRegions((rs) => [...rs, nr].sort((a, b) => a.start - b.start));
            setSelectedRegionId(id);
            addLog(
              `Region added: ${nr.start.toFixed(1)}s–${nr.end.toFixed(1)}s.`
            );
          }}
          onDeleteRegion={() => {
            if (!selectedRegionId) return;
            setRegions((rs) => rs.filter((r) => r.id !== selectedRegionId));
            setSelectedRegionId(null);
            addLog("Region deleted.");
          }}
          onToggleRecording={toggleRecording}
          onImport={async () => {
            try {
              const ms = await detectMoments("/mock/import.mp4");
              setMoments(ms);
              setRegions(regionsFromMoments(ms, DURATION));
              setVideoPath("/mock/import.mp4");
              addLog(`Import: ${ms.length} moments detected.`);
            } catch (e) {
              addLog(`detectMoments failed: ${String(e)}`);
            }
          }}
          onAutoEdit={() =>
            runJob("Auto-Edit: rendering regions…", () =>
              renderPreview(videoPath, regions)
            )
          }
          onExport={() =>
            runJob("Export: rendering highlights…", () =>
              exportHighlights(videoPath, regions, "/mock/highlights.mp4")
            )
          }
        />
      </div>
      <Timeline
        moments={moments}
        regions={regions}
        selectedRegionId={selectedRegionId}
        playhead={playhead}
        onSelectRegion={setSelectedRegionId}
      />
    </div>
  );
}

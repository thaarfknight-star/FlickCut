import { useEffect, useRef, useState } from "react";
import HeaderBar from "./components/HeaderBar";
import LibraryDock from "./components/LibraryDock";
import PreviewPane from "./components/PreviewPane";
import SettingsDock from "./components/SettingsDock";
import Timeline from "./components/Timeline";
import {
  DURATION,
  MOCK_MOMENTS,
  MOCK_REGIONS,
  formatTime,
  type EffectType,
  type Region,
} from "./mockData";

let nextId = 100;

export default function App() {
  const [moments, setMoments] = useState(MOCK_MOMENTS);
  const [regions, setRegions] = useState<Region[]>(MOCK_REGIONS);
  const [selectedRegionId, setSelectedRegionId] = useState<string | null>(
    "r1"
  );
  const [recording, setRecording] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [playhead] = useState(32);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState(0);
  const [log, setLog] = useState<string[]>([
    "FlickCut editor spike — mock session loaded.",
    `${MOCK_MOMENTS.length} moments, ${MOCK_REGIONS.length} suggested regions.`,
  ]);
  const timer = useRef<number | null>(null);

  const addLog = (line: string) =>
    setLog((l) => [...l.slice(-49), line]);

  // recording timer
  useEffect(() => {
    if (recording) {
      timer.current = window.setInterval(() => setElapsed((e) => e + 1), 1000);
    } else if (timer.current) {
      window.clearInterval(timer.current);
      timer.current = null;
    }
    return () => {
      if (timer.current) window.clearInterval(timer.current);
    };
  }, [recording]);

  const selectedRegion =
    regions.find((r) => r.id === selectedRegionId) ?? null;

  const patchRegion = (id: string, patch: Partial<Region>) =>
    setRegions((rs) => rs.map((r) => (r.id === id ? { ...r, ...patch } : r)));

  const toggleRecording = () => {
    if (recording) {
      setRecording(false);
      addLog(`Recording stopped at ${formatTime(elapsed)} — session saved.`);
    } else {
      setRecording(true);
      setElapsed(0);
      addLog("Recording started — capturing screen + input telemetry.");
    }
  };

  const fakeProgress = (label: string, done: string) => {
    setBusy(true);
    setProgress(0);
    const iv = window.setInterval(() => {
      setProgress((p) => {
        if (p >= 100) {
          window.clearInterval(iv);
          setBusy(false);
          addLog(done);
          return 100;
        }
        return p + 10;
      });
    }, 120);
    addLog(label);
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
        version="0.3.0-spike"
      />
      <div style={{ flex: 1, display: "flex", gap: 10, padding: 12, minHeight: 0 }}>
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
        <PreviewPane recording={recording} />
        <SettingsDock
          region={selectedRegion}
          recording={recording}
          busy={busy}
          progress={progress}
          log={log}
          onEffectChange={(e: EffectType) => {
            if (selectedRegionId)
              patchRegion(selectedRegionId, { effect: e });
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
            addLog(`Region added: ${nr.start.toFixed(1)}s–${nr.end.toFixed(1)}s.`);
          }}
          onDeleteRegion={() => {
            if (!selectedRegionId) return;
            setRegions((rs) => rs.filter((r) => r.id !== selectedRegionId));
            setSelectedRegionId(null);
            addLog("Region deleted.");
          }}
          onToggleRecording={toggleRecording}
          onImport={() => addLog("Import video — mock: 5 moments detected.")}
          onAutoEdit={() =>
            fakeProgress(
              "Auto-Edit: rendering regions…",
              "Auto-Edit done — preview ready (mock)."
            )
          }
          onExport={() =>
            fakeProgress(
              "Export: rendering highlights…",
              "Export done: highlights.mp4 (mock)."
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

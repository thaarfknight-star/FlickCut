import { useEffect, useRef, useState } from "react";
import HeaderBar from "./components/HeaderBar";
import LibraryDock from "./components/LibraryDock";
import PreviewPane from "./components/PreviewPane";
import SettingsDock from "./components/SettingsDock";
import Timeline from "./components/Timeline";
import {
  DURATION,
  formatTime,
  regionsFromMarkers,
} from "./mockData";
import type { EffectType, Marker, Region } from "./ipc/types";

// Seed data matching the IPC contract shapes (docs/ipc-contract-v1.md).
// TAgent4's lane wires these to the real backend via ./ipc/tauriApi.
const SEED_MARKERS: Marker[] = [
  { timestamp: 12.4, type: "kill", intensity: 92, duration: 1.2, x: 0.62, y: 0.44 },
  { timestamp: 34.8, type: "hit", intensity: 58, duration: 0.6, x: 0.41, y: 0.52 },
  { timestamp: 61.2, type: "kill", intensity: 87, duration: 1.2, x: 0.55, y: 0.38 },
  { timestamp: 88.9, type: "exciting", intensity: 74, duration: 2.0, x: 0.5, y: 0.5 },
];

export default function App() {
  const [markers, setMarkers] = useState<Marker[]>(SEED_MARKERS);
  const [regions, setRegions] = useState<Region[]>(() =>
    regionsFromMarkers(SEED_MARKERS, DURATION)
  );
  const [selectedRegion, setSelectedRegion] = useState<number | null>(0);
  const [recording, setRecording] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [playhead] = useState(32);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState(0);
  const [log, setLog] = useState<string[]>([
    "FlickCut editor — mock session loaded.",
    `${SEED_MARKERS.length} markers, ${regionsFromMarkers(SEED_MARKERS, DURATION).length} suggested regions.`,
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

  const selected: Region | null =
    selectedRegion !== null ? (regions[selectedRegion] ?? null) : null;

  const patchRegion = (index: number, patch: Partial<Region>) =>
    setRegions((rs) =>
      rs.map((r, i) => (i === index ? { ...r, ...patch } : r))
    );

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
        version="0.1.0"
      />
      <div style={{ flex: 1, display: "flex", gap: 10, padding: 12, minHeight: 0 }}>
        <LibraryDock
          markers={markers}
          regions={regions}
          selectedRegion={selectedRegion}
          onSelectRegion={setSelectedRegion}
          onDeleteMarker={(index) => {
            setMarkers((ms) => ms.filter((_, i) => i !== index));
            addLog("Marker deleted.");
          }}
          onClearMarkers={() => {
            setMarkers([]);
            addLog("All markers cleared.");
          }}
        />
        <PreviewPane recording={recording} />
        <SettingsDock
          region={selected}
          recording={recording}
          busy={busy}
          progress={progress}
          log={log}
          onEffectChange={(e: EffectType) => {
            if (selectedRegion !== null)
              patchRegion(selectedRegion, { effect: e });
          }}
          onIntensityChange={(v) => {
            if (selectedRegion !== null)
              patchRegion(selectedRegion, { intensity: v });
          }}
          onBoundsChange={(start, end) => {
            if (selectedRegion !== null && end > start && start >= 0)
              patchRegion(selectedRegion, { start, end });
          }}
          onAddRegion={() => {
            const c = playhead > 0 ? playhead : DURATION / 2;
            const nr: Region = {
              start: Math.max(0, c - 0.6),
              end: Math.min(DURATION, c + 0.6),
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
            if (selectedRegion === null) return;
            setRegions((rs) => rs.filter((_, i) => i !== selectedRegion));
            setSelectedRegion(null);
            addLog("Region deleted.");
          }}
          onToggleRecording={toggleRecording}
          onImport={() => addLog("Import video — mock: 5 markers detected.")}
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
        markers={markers}
        regions={regions}
        duration={DURATION}
        selectedRegion={selectedRegion}
        playhead={playhead}
        onSelectRegion={setSelectedRegion}
      />
    </div>
  );
}

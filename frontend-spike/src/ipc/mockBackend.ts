/**
 * Mock backend — implements the IPC contract with timers and fake data.
 * Used when the app runs outside Tauri (plain browser / `vite dev`).
 * The real shell replaces this file's role via `invoke`/`listen`.
 */
import { COMMANDS, EVENTS } from "./contract";
import type {
  CaptureSource,
  Moment,
  RecordingSummary,
  RenderProgress,
} from "./types";

type Handler = (payload: unknown) => void;

const listeners = new Map<string, Set<Handler>>();

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((h) => {
    try {
      h(payload);
    } catch {
      /* listener errors never break the backend */
    }
  });
}

export function mockListen(
  event: string,
  handler: Handler
): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler);
  return () => listeners.get(event)?.delete(handler);
}

let sessionCounter = 0;
let recording = false;
let recElapsed = 0;
let recTimer: number | null = null;
let momentTimer: number | null = null;

const MOCK_SOURCES: CaptureSource[] = [
  { id: "screen-0", name: "Main display", kind: "screen" },
  { id: "window-1", name: "Game window (mock)", kind: "window" },
];

const MOCK_MOMENT_FEED: Array<Omit<Moment, "id">> = [
  { timestamp: 12.4, type: "kill", intensity: 95, x: 0.62, y: 0.41 },
  { timestamp: 34.8, type: "hit", intensity: 60, x: 0.51, y: 0.48 },
  { timestamp: 47.2, type: "kill", intensity: 88, x: 0.58, y: 0.39 },
  { timestamp: 71.9, type: "exciting", intensity: 74, x: 0.5, y: 0.5 },
  { timestamp: 102.3, type: "kill", intensity: 91, x: 0.66, y: 0.44 },
];

function fakeRender(jobId: string, done: () => void) {
  let p = 0;
  const iv = window.setInterval(() => {
    p = Math.min(100, p + 10);
    const payload: RenderProgress = { job_id: jobId, progress: p };
    emit(EVENTS.RENDER_PROGRESS, payload);
    if (p >= 100) {
      window.clearInterval(iv);
      done();
    }
  }, 120);
}

/** Dispatch a contract command to the mock backend. */
export async function mockInvoke<T>(
  command: string,
  args: Record<string, unknown> = {}
): Promise<T> {
  // small async delay so the UI behaves like real IPC
  await new Promise((r) => setTimeout(r, 30));

  switch (command) {
    case COMMANDS.LIST_SOURCES:
      return MOCK_SOURCES as unknown as T;

    case COMMANDS.START_RECORDING: {
      sessionCounter += 1;
      recording = true;
      recElapsed = 0;
      const feed = [...MOCK_MOMENT_FEED];
      let n = 0;
      recTimer = window.setInterval(() => {
        recElapsed += 1;
      }, 1000);
      // drip-feed one mock moment every ~4s while "recording"
      momentTimer = window.setInterval(() => {
        const m = feed.shift();
        if (!m) return;
        n += 1;
        emit(EVENTS.MOMENT_DETECTED, {
          ...m,
          id: `live-${sessionCounter}-${n}`,
          timestamp: recElapsed + 0.5,
        } as Moment);
      }, 4000);
      return { session_id: `sess-${sessionCounter}` } as unknown as T;
    }

    case COMMANDS.STOP_RECORDING: {
      recording = false;
      if (recTimer) window.clearInterval(recTimer);
      if (momentTimer) window.clearInterval(momentTimer);
      const summary: RecordingSummary = {
        session_id: `sess-${sessionCounter}`,
        video_path: "/mock/session.mp4",
        duration_s: Math.max(recElapsed, 1),
        moments: MOCK_MOMENT_FEED.map((m, i) => ({
          ...m,
          id: `m${i + 1}`,
        })),
      };
      return summary as unknown as T;
    }

    case COMMANDS.DETECT_MOMENTS: {
      void args["video_path"];
      return MOCK_MOMENT_FEED.map((m, i) => ({
        ...m,
        id: `imp-${i + 1}`,
      })) as unknown as T;
    }

    case COMMANDS.RENDER_PREVIEW: {
      const jobId = `preview-${Date.now()}`;
      return new Promise<T>((resolve) => {
        fakeRender(jobId, () =>
          resolve({ preview_path: "/mock/preview.mp4" } as unknown as T)
        );
      });
    }

    case COMMANDS.EXPORT_HIGHLIGHTS: {
      const jobId = `export-${Date.now()}`;
      const out = String(args["output_path"] || "/mock/highlights.mp4");
      return new Promise<T>((resolve) => {
        fakeRender(jobId, () =>
          resolve({ output_path: out } as unknown as T)
        );
      });
    }

    default:
      throw new Error(`mockBackend: unknown command "${command}"`);
  }
}

export function isMockRecording(): boolean {
  return recording;
}

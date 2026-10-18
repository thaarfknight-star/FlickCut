/**
 * Mock backend — implements docs/ipc-contract-v1.md with timers and
 * fake data. Used when running outside Tauri (plain browser).
 */
import { COMMANDS, EVENTS } from "./contract";
import type {
  InputEvent,
  Marker,
  RecordingTick,
  RenderProgress,
  SourceList,
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

export function mockListen(event: string, handler: Handler): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler);
  return () => listeners.get(event)?.delete(handler);
}

let sessionCounter = 0;
let activeSession: string | null = null;
let recElapsed = 0;
let tickTimer: number | null = null;
let inputTimer: number | null = null;

const MOCK_SOURCES: SourceList = {
  screens: [{ id: "screen-0", name: "Main display" }],
  windows: [{ id: "window-1", title: "Game window (mock)" }],
};

const MOCK_MARKERS: Marker[] = [
  { timestamp: 12.4, type: "kill", intensity: 95, duration: 1.2, x: 0.62, y: 0.41 },
  { timestamp: 34.8, type: "hit", intensity: 60, duration: 0.6, x: 0.51, y: 0.48 },
  { timestamp: 47.2, type: "kill", intensity: 88, duration: 1.2, x: 0.58, y: 0.39 },
  { timestamp: 71.9, type: "exciting", intensity: 74, duration: 1.0, x: 0.5, y: 0.5 },
  { timestamp: 102.3, type: "kill", intensity: 91, duration: 1.2, x: 0.66, y: 0.44 },
];

const MOCK_INPUTS: Array<Omit<InputEvent, "t_ms">> = [
  { source: "mouse", action: "click_left", detail: {} },
  { source: "mouse", action: "click_left", detail: {} },
  { source: "keyboard", action: "key_r", detail: {} },
];

function stopTimers() {
  if (tickTimer) window.clearInterval(tickTimer);
  if (inputTimer) window.clearInterval(inputTimer);
  tickTimer = inputTimer = null;
}

/** Dispatch a contract command to the mock backend. */
export async function mockInvoke<T>(
  command: string,
  args: Record<string, unknown> = {}
): Promise<T> {
  await new Promise((r) => setTimeout(r, 30));

  switch (command) {
    case COMMANDS.LIST_SOURCES:
      return MOCK_SOURCES as unknown as T;

    case COMMANDS.START_RECORDING: {
      sessionCounter += 1;
      activeSession = `sess-${sessionCounter}`;
      recElapsed = 0;
      tickTimer = window.setInterval(() => {
        recElapsed += 1;
        emit(EVENTS.RECORDING_TICK, {
          elapsed_sec: recElapsed,
        } as RecordingTick);
      }, 1000);
      // drip-feed mock input events while recording
      let i = 0;
      inputTimer = window.setInterval(() => {
        const tpl = MOCK_INPUTS[i++ % MOCK_INPUTS.length];
        emit(EVENTS.INPUT_EVENT, {
          ...tpl,
          t_ms: recElapsed * 1000,
        } as InputEvent);
      }, 2500);
      void args["source_id"];
      return { session_id: activeSession } as unknown as T;
    }

    case COMMANDS.STOP_RECORDING: {
      stopTimers();
      const dur = Math.max(recElapsed, 1);
      activeSession = null;
      return {
        video_path: "/mock/session.mp4",
        duration_sec: dur,
      } as unknown as T;
    }

    case COMMANDS.ANALYZE_MOMENTS: {
      void args["session_id"];
      // scale markers into the recorded duration for realism
      return { markers: MOCK_MARKERS } as unknown as T;
    }

    case COMMANDS.RENDER_VIDEO: {
      void args["video_path"];
      void args["regions"];
      return new Promise<T>((resolve) => {
        let p = 0;
        const iv = window.setInterval(() => {
          p = Math.min(100, p + 10);
          emit(EVENTS.RENDER_PROGRESS, { percent: p } as RenderProgress);
          if (p >= 100) {
            window.clearInterval(iv);
            resolve({ output_path: "/mock/highlights.mp4" } as unknown as T);
          }
        }, 120);
      });
    }

    case COMMANDS.IMPORT_VIDEO: {
      void args["path"];
      sessionCounter += 1;
      activeSession = `sess-${sessionCounter}`;
      return { session_id: activeSession } as unknown as T;
    }

    default:
      throw new Error(`mockBackend: unknown command "${command}"`);
  }
}

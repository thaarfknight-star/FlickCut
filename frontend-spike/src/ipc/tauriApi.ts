/**
 * tauriApi — the ONLY module the UI talks to.
 *
 * Implements docs/ipc-contract-v1.md. Inside the Tauri shell
 * (`window.__TAURI__` present) every function delegates to the real
 * `invoke`/`listen`; otherwise it falls back to the mock backend so the
 * editor runs standalone in a plain browser (`vite dev`).
 *
 * Components must import from here — never `invoke`/`listen` directly.
 */
import { COMMANDS, EVENTS } from "./contract";
import { mockInvoke, mockListen } from "./mockBackend";
import type {
  InputEvent,
  Marker,
  RecordingTick,
  Region,
  RenderProgress,
  SourceList,
} from "./types";

export type {
  InputEvent,
  Marker,
  RecordingTick,
  Region,
  RenderProgress,
  SourceList,
};
export type { MomentType, EffectType } from "./types";

declare global {
  interface Window {
    __TAURI__?: {
      core: {
        invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
      };
      event: {
        listen: <T>(
          event: string,
          handler: (e: { payload: T }) => void
        ) => Promise<() => void>;
      };
    };
  }
}

const inTauri = typeof window !== "undefined" && !!window.__TAURI__;

async function invoke<T>(
  command: string,
  args: Record<string, unknown> = {}
): Promise<T> {
  if (inTauri) {
    return window.__TAURI__!.core.invoke<T>(command, args);
  }
  return mockInvoke<T>(command, args);
}

function listen<T>(event: string, handler: (payload: T) => void): () => void {
  if (inTauri) {
    let unsub: (() => void) | null = null;
    window
      .__TAURI__!.event.listen<T>(event, (e) => handler(e.payload))
      .then((u) => {
        unsub = u;
      });
    return () => unsub?.();
  }
  return mockListen(event, handler as (p: unknown) => void);
}

/* -- commands ---------------------------------------------------- */

export const listSources = (): Promise<SourceList> =>
  invoke<SourceList>(COMMANDS.LIST_SOURCES);

export const startRecording = (
  sourceId: string
): Promise<{ session_id: string }> =>
  invoke(COMMANDS.START_RECORDING, { source_id: sourceId });

export const stopRecording = (): Promise<{
  video_path: string;
  duration_sec: number;
}> => invoke(COMMANDS.STOP_RECORDING);

export const analyzeMoments = (
  sessionId: string
): Promise<{ markers: Marker[] }> =>
  invoke(COMMANDS.ANALYZE_MOMENTS, { session_id: sessionId });

export const renderVideo = (
  videoPath: string,
  regions: Region[]
): Promise<{ output_path: string }> =>
  invoke(COMMANDS.RENDER_VIDEO, { video_path: videoPath, regions });

export const importVideo = (path: string): Promise<{ session_id: string }> =>
  invoke(COMMANDS.IMPORT_VIDEO, { path });

/**
 * Pause/resume are NOT in contract v1 (frozen). Works in mock mode;
 * in the real shell this throws until DEVagent adds the commands (v1.1).
 */
export const pauseRecording = (): Promise<void> =>
  inTauri
    ? Promise.reject(
        new Error("pause_recording is not in IPC contract v1")
      )
    : mockInvoke<void>("pause_recording_mock");

export const resumeRecording = (): Promise<void> =>
  inTauri
    ? Promise.reject(
        new Error("resume_recording is not in IPC contract v1")
      )
    : mockInvoke<void>("resume_recording_mock");

/* -- events ------------------------------------------------------ */

export const onRecordingTick = (
  handler: (t: RecordingTick) => void
): (() => void) => listen<RecordingTick>(EVENTS.RECORDING_TICK, handler);

export const onRenderProgress = (
  handler: (p: RenderProgress) => void
): (() => void) => listen<RenderProgress>(EVENTS.RENDER_PROGRESS, handler);

export const onInputEvent = (
  handler: (e: InputEvent) => void
): (() => void) => listen<InputEvent>(EVENTS.INPUT_EVENT, handler);

export const isRunningInTauri = (): boolean => inTauri;

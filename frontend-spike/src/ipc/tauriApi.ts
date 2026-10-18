/**
 * tauriApi — the ONLY module the UI talks to.
 *
 * When running inside the Tauri shell (`window.__TAURI__` present) every
 * function below delegates to the real `invoke`/`listen`. Otherwise it
 * falls back to the mock backend, so the editor runs standalone in a
 * plain browser (`vite dev`).
 *
 * Components must import from here — never from `mockBackend`, never
 * `invoke`/`listen` directly. Connecting TAgent5's shell = no component
 * changes.
 */
import { COMMANDS, EVENTS } from "./contract";
import { mockInvoke, mockListen } from "./mockBackend";
import type {
  CaptureSource,
  Moment,
  RecordingSummary,
  Region,
  RenderProgress,
} from "./types";

export type { CaptureSource, Moment, RecordingSummary, Region, RenderProgress };
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

const inTauri =
  typeof window !== "undefined" && !!window.__TAURI__;

async function invoke<T>(
  command: string,
  args: Record<string, unknown> = {}
): Promise<T> {
  if (inTauri) {
    return window.__TAURI__!.core.invoke<T>(command, args);
  }
  return mockInvoke<T>(command, args);
}

function listen<T>(
  event: string,
  handler: (payload: T) => void
): () => void {
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

export const listSources = (): Promise<CaptureSource[]> =>
  invoke<CaptureSource[]>(COMMANDS.LIST_SOURCES);

export const startRecording = (
  sourceId: string
): Promise<{ session_id: string }> =>
  invoke(COMMANDS.START_RECORDING, { source_id: sourceId });

export const stopRecording = (): Promise<RecordingSummary> =>
  invoke<RecordingSummary>(COMMANDS.STOP_RECORDING);

export const detectMoments = (videoPath: string): Promise<Moment[]> =>
  invoke<Moment[]>(COMMANDS.DETECT_MOMENTS, { video_path: videoPath });

export const renderPreview = (
  videoPath: string,
  regions: Region[]
): Promise<{ preview_path: string }> =>
  invoke(COMMANDS.RENDER_PREVIEW, { video_path: videoPath, regions });

export const exportHighlights = (
  videoPath: string,
  regions: Region[],
  outputPath: string
): Promise<{ output_path: string }> =>
  invoke(COMMANDS.EXPORT_HIGHLIGHTS, {
    video_path: videoPath,
    regions,
    output_path: outputPath,
  });

/* -- events ------------------------------------------------------ */

export const onMomentDetected = (
  handler: (m: Moment) => void
): (() => void) => listen<Moment>(EVENTS.MOMENT_DETECTED, handler);

export const onRenderProgress = (
  handler: (p: RenderProgress) => void
): (() => void) => listen<RenderProgress>(EVENTS.RENDER_PROGRESS, handler);

export const isRunningInTauri = (): boolean => inTauri;

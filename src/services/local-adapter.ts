import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import type { PosServices } from "./ports";
import type { AppHealth } from "./types";

export type InvokeFn = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createLocalServices(invoke: InvokeFn = tauriInvoke): PosServices {
  return {
    settings: {
      getHealth: () => invoke<AppHealth>("get_app_health"),
    },
  };
}

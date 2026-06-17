import type { AppHealth } from "./types";

export interface SettingsService {
  getHealth(): Promise<AppHealth>;
}

export interface PosServices {
  settings: SettingsService;
}

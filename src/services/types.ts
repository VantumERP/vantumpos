export type BackendKind = "local";

export interface AppHealth {
  backend: BackendKind;
  appVersion: string;
  databasePath: string;
  migrated: boolean;
}

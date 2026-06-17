import type { PosServices } from "./ports";

export function createMockServices(): PosServices {
  return {
    settings: {
      async getHealth() {
        return {
          backend: "local",
          appVersion: "test",
          databasePath: "mock://vantumpos.sqlite3",
          migrated: true,
        };
      },
    },
  };
}

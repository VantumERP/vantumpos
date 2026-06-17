import { describe, expect, it, vi } from "vitest";

import { createLocalServices } from "./local-adapter";
import { createMockServices } from "./mock-adapter";

describe("local service adapter", () => {
  it("calls the Tauri health command through the injected invoker", async () => {
    const invoke = vi.fn().mockResolvedValue({
      backend: "local",
      appVersion: "0.1.0",
      databasePath: "C:/Users/test/AppData/Roaming/vantumpos/vantumpos.sqlite3",
      migrated: true,
    });

    const services = createLocalServices(invoke);
    const health = await services.settings.getHealth();

    expect(invoke).toHaveBeenCalledWith("get_app_health");
    expect(health.backend).toBe("local");
    expect(health.migrated).toBe(true);
  });
});

describe("mock service adapter", () => {
  it("returns deterministic health for UI tests", async () => {
    const services = createMockServices();
    const health = await services.settings.getHealth();

    expect(health).toEqual({
      backend: "local",
      appVersion: "test",
      databasePath: "mock://vantumpos.sqlite3",
      migrated: true,
    });
  });
});

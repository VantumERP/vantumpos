import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AppVersion } from "./AppVersion";
import type { SettingsService } from "@/services/ports";

function settingsWith(appVersion: string): SettingsService {
  return {
    getHealth: vi.fn(async () => ({
      backend: "local",
      appVersion,
      databasePath: "mock",
      migrated: true,
    })),
  } as unknown as SettingsService;
}

describe("AppVersion", () => {
  it("shows the app version from health", async () => {
    render(<AppVersion settings={settingsWith("0.1.0")} />);
    expect(await screen.findByText("Verzija 0.1.0")).toBeInTheDocument();
  });
});

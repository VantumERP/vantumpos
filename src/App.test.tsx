import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AppShell } from "./app/AppShell";
import { createMockServices } from "./services/mock-adapter";
import type { PosServices } from "./services/ports";

describe("AppShell", () => {
  it("renders the POS navigation and backend status", async () => {
    render(<AppShell services={createMockServices()} />);

    expect(screen.getByRole("heading", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Artikli" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Lager" })).toBeInTheDocument();
    expect(await screen.findByText("Lokalna baza spremna")).toBeInTheDocument();
  });

  it("renders a stable fallback when backend health fails", async () => {
    const services: PosServices = {
      settings: {
        getHealth: () => Promise.reject(new Error("boom")),
      },
    };

    render(<AppShell services={services} />);

    expect(await screen.findByText("Backend nije dostupan.")).toBeInTheDocument();
    expect(screen.queryByText("boom")).not.toBeInTheDocument();
  });
});

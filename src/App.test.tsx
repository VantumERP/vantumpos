import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AppShell } from "./app/AppShell";
import { createMockServices } from "./services/mock-adapter";

describe("AppShell", () => {
  it("renders the POS navigation and backend status", async () => {
    render(<AppShell services={createMockServices()} />);

    expect(screen.getByRole("heading", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Artikli" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Lager" })).toBeInTheDocument();
    expect(await screen.findByText("Lokalna baza spremna")).toBeInTheDocument();
  });
});

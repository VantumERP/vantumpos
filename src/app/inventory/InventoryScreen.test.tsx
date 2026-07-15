import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { InventoryScreen } from "./InventoryScreen";
import { createMockServices } from "@/services/mock-adapter";

describe("InventoryScreen", () => {
  it("submits product, quantity, and reason for the inventory movement", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const receiveStock = vi.spyOn(services.inventory, "receiveStock");

    render(<InventoryScreen services={services} />);

    await user.click(
      await screen.findByRole("button", { name: "Prijem robe za Mleko 1 l" }),
    );

    const quantity = await screen.findByLabelText("Količina");
    await user.clear(quantity);
    await user.type(quantity, "2");
    await user.type(screen.getByLabelText("Razlog"), "Dostava");
    await user.click(screen.getByRole("button", { name: "Sačuvaj prijem" }));

    await waitFor(() =>
      expect(receiveStock).toHaveBeenCalledWith(
        expect.objectContaining({ productId: 1, quantityMilli: 2000 }),
      ),
    );
  });

  it("renders the backend error and still submits the write-off request", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const writeOffStock = vi.spyOn(services.inventory, "writeOffStock");

    render(<InventoryScreen services={services} />);

    await user.click(
      await screen.findByRole("button", { name: "Otpis za Mleko 1 l" }),
    );

    const quantity = await screen.findByLabelText("Količina");
    await user.clear(quantity);
    await user.type(quantity, "5");
    await user.type(screen.getByLabelText("Razlog"), "Lom");
    await user.click(screen.getByRole("button", { name: "Sačuvaj otpis" }));

    expect(await screen.findByText("Nema dovoljno zaliha.")).toBeInTheDocument();
    expect(writeOffStock).toHaveBeenCalledWith(
      expect.objectContaining({ quantityMilli: 5000 }),
    );
  });
});

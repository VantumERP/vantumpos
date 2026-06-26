import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReceiptsScreen } from "./ReceiptsScreen";
import { createMockServices } from "@/services/mock-adapter";

describe("ReceiptsScreen", () => {
  it("threads the session user id into the void call", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const voidReceipt = vi.spyOn(services.receipts, "voidReceipt");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Storniraj racun" }));
    await user.type(screen.getByLabelText("Razlog"), "Greska kasira");
    await user.click(screen.getByRole("button", { name: "Potvrdi storniranje" }));

    await waitFor(() =>
      expect(voidReceipt).toHaveBeenCalledWith({
        receiptId: 1,
        userId: 9,
        reason: "Greska kasira",
      }),
    );
  });

  it("threads the session user id into the return call", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));

    const quantity = await screen.findByLabelText("Kolicina za Kafa 200 g");
    await user.clear(quantity);
    await user.type(quantity, "1");
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sacuvaj povrat" }));

    await waitFor(() =>
      expect(returnItems).toHaveBeenCalledWith(
        expect.objectContaining({
          receiptId: 1,
          userId: 9,
          items: [{ saleItemId: 1, quantityMilli: 1000 }],
        }),
      ),
    );
  });

  it("blocks the return and skips the service when no quantity is entered", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sacuvaj povrat" }));

    expect(
      await screen.findByText("Unesite kolicinu za bar jedan artikal."),
    ).toBeInTheDocument();
    expect(returnItems).not.toHaveBeenCalled();
  });
});

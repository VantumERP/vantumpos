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

  // ZoT čl. 34: the marking duty is the proizvođač's/uvoznik's (st. 2), but
  // čl. 68 st. 1 tač. 9 punishes the trgovac who *sells* goods without a
  // deklaracija. The goods receipt is the last moment the shop can refuse the
  // pallet — so it warns there, and only warns (§3 req 25).
  describe("deklaracija na prijemu (ZoT čl. 34)", () => {
    const advisory =
      "U sistemu nisu evidentirani podaci sa deklaracije. Ako roba fizički " +
      "nosi ispravnu deklaraciju, prekršaja nema — unesite podatke sa " +
      "deklaracije ili evidentirajte proveru deklaracije (olakšavajuća " +
      "okolnost, čl. 69a).";

    // `legal::declaration_missing` verbatim, with `penalty: null` — the fixture
    // names one čl. 34 state (roba bez deklaracije, čl. 68) and never welds it
    // to the neuredna-deklaracija state (čl. 67), and it carries no figure of
    // its own: every amount comes from src-tauri/src/legal.rs.
    const missingNotice = {
      summary:
        "Prodaja robe bez deklaracije. Deklaraciju obezbeđuje proizvođač, " +
        "odnosno uvoznik, ali za prodaju takve robe odgovara trgovac.",
      penalty: null,
      citation: "Zakon o trgovini, čl. 34 st. 1–2, čl. 68 st. 1 tač. 9.",
      isLegalDuty: true,
    };

    function receiptWithWarning() {
      return {
        productId: 1,
        movementId: 10,
        movementType: "receive" as const,
        quantityMilli: 2000,
        previousQuantityMilli: 3000,
        newQuantityMilli: 5000,
        createdAt: "2026-06-18T12:00:00Z",
        declarationWarnings: [
          {
            productId: 1,
            productName: "Mleko 1 l",
            missingFields: ["manufacturerName", "countryOfOrigin"],
            advisory,
            notice: missingNotice,
          },
        ],
      };
    }

    async function receiveMleko(user: ReturnType<typeof userEvent.setup>) {
      await user.click(
        await screen.findByRole("button", { name: "Prijem robe za Mleko 1 l" }),
      );
      const quantity = await screen.findByLabelText("Količina");
      await user.clear(quantity);
      await user.type(quantity, "2");
      await user.click(screen.getByRole("button", { name: "Sačuvaj prijem" }));
    }

    it("says the receipt is written before it says anything is missing", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      vi.spyOn(services.inventory, "receiveStock").mockResolvedValue(
        receiptWithWarning(),
      );

      render(<InventoryScreen services={services} />);
      await receiveMleko(user);

      expect(
        await screen.findByText("Prijem je upisan — nedostaju podaci deklaracije"),
      ).toBeInTheDocument();
    });

    // §3 req 26 [ACCURACY]: the qualifier travels *with* the figure. The notice
    // alone reads as a proven offence; the advisory alone hides the exposure.
    it("renders the advisory together with the notice, never the notice alone", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      vi.spyOn(services.inventory, "receiveStock").mockResolvedValue(
        receiptWithWarning(),
      );

      render(<InventoryScreen services={services} />);
      await receiveMleko(user);

      expect(await screen.findByText(advisory)).toBeInTheDocument();
      expect(screen.getByText(missingNotice.summary)).toBeInTheDocument();
      // No tier resolved, so no figure is invented — the panel says where the
      // legal form is entered instead.
      expect(
        screen.getByText(
          "Unesite pravnu formu u Podešavanja → Profil da bi kazna bila prikazana.",
        ),
      ).toBeInTheDocument();
      expect(
        screen.getByText(
          "Zakon o trgovini, čl. 34 st. 1–2, čl. 68 st. 1 tač. 9.",
        ),
      ).toBeInTheDocument();
      expect(
        screen.getByText("Nedostaje: Poslovno ime proizvođača, Zemlja proizvodnje"),
      ).toBeInTheDocument();
    });

    // The timestamped check is the čl. 69a tač. 4 mitigation record — the one
    // action the operator can take at the pallet without retyping the label.
    it("records the deklaracija check from the warning panel", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      vi.spyOn(services.inventory, "receiveStock").mockResolvedValue(
        receiptWithWarning(),
      );
      const markDeclarationChecked = vi.spyOn(
        services.inventory,
        "markDeclarationChecked",
      );

      render(<InventoryScreen services={services} />);
      await receiveMleko(user);

      await user.click(
        await screen.findByRole("button", {
          name: "Evidentiraj proveru deklaracije",
        }),
      );

      await waitFor(() => expect(markDeclarationChecked).toHaveBeenCalledWith(1));
    });

    // „Mleko 1 l" in the demo catalogue has no proizvođač, so the warning path
    // is the *default* here — the empty-warning receipt has to be arranged
    // explicitly, or this test asserts nothing about closing.
    it("closes as usual when the receipt carries no warning", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      vi.spyOn(services.inventory, "receiveStock").mockResolvedValue({
        ...receiptWithWarning(),
        declarationWarnings: [],
      });

      render(<InventoryScreen services={services} />);
      await receiveMleko(user);

      await waitFor(() =>
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
      );
      expect(
        screen.queryByText("Prijem je upisan — nedostaju podaci deklaracije"),
      ).not.toBeInTheDocument();
    });
  });
});

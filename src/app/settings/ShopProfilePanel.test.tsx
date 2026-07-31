import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ShopProfilePanel } from "./ShopProfilePanel";
import type { ShopProfile } from "@/services/types";

const unset: ShopProfile = {
  pravnaForma: null,
  pdvObveznik: null,
  distanceSelling: null,
  lpfrInPremises: null,
  esirElements: [],
};

describe("ShopProfilePanel", () => {
  it("preselects no legal form and says why it matters", async () => {
    render(<ShopProfilePanel profile={unset} onSave={vi.fn()} />);

    const preduzetnik = screen.getByRole("radio", { name: /preduzetnik/i });
    const pravnoLice = screen.getByRole("radio", { name: /pravno lice/i });
    expect(preduzetnik).not.toBeChecked();
    expect(pravnoLice).not.toBeChecked();
    expect(
      screen.getByText(/iznosi kazni se ne prikazuju dok se ne unese pravna forma/i),
    ).toBeInTheDocument();
  });

  it("frames the ESIR row as an internal note, never as an evidencija", () => {
    render(<ShopProfilePanel profile={unset} onSave={vi.fn()} />);

    expect(screen.getByText(/interna beleška o proveri/i)).toBeInTheDocument();
    expect(
      screen.getByText(/nijedan propis ne zahteva da radnja čuva ove podatke/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/evidencija ESIR/i)).not.toBeInTheDocument();
  });

  it("leaves prodaja na daljinu unanswered and asks for the answer", () => {
    render(<ShopProfilePanel profile={unset} onSave={vi.fn()} />);

    const da = screen.getByRole("radio", { name: /prodaja na daljinu: da/i });
    const ne = screen.getByRole("radio", { name: /prodaja na daljinu: ne/i });
    expect(da).not.toBeChecked();
    expect(ne).not.toBeChecked();
    expect(screen.getByText(/nije odgovoreno/i)).toBeInTheDocument();
  });

  it("saves the selected profile", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<ShopProfilePanel profile={unset} onSave={onSave} />);

    await userEvent.click(screen.getByRole("radio", { name: /preduzetnik/i }));
    await userEvent.click(screen.getByRole("radio", { name: /prodaja na daljinu: da/i }));
    await userEvent.click(screen.getByRole("button", { name: /sačuvaj profil/i }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({ pravnaForma: "preduzetnik", distanceSelling: true }),
      ),
    );
  });

  it("prompts for a re-check when the last ESIR check is more than a year old", () => {
    const thirteenMonthsAgo = new Date(
      Date.now() - 400 * 24 * 60 * 60 * 1000,
    ).toISOString();
    const stale: ShopProfile = {
      ...unset,
      esirElements: [
        {
          naziv: "VantumESIR",
          verzija: "2.1.4",
          ib: "123456",
          tip: "ESIR",
          checkedOn: thirteenMonthsAgo,
        },
      ],
    };

    render(<ShopProfilePanel profile={stale} onSave={vi.fn()} />);

    expect(
      screen.getByText(/provera je starija od godinu dana/i),
    ).toBeInTheDocument();
  });
});

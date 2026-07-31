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

  it("leaves prodaja na daljinu unanswered and asks for the answer", async () => {
    render(<ShopProfilePanel profile={unset} onSave={vi.fn()} />);

    const da = screen.getByRole("radio", { name: /prodaja na daljinu: da/i });
    const ne = screen.getByRole("radio", { name: /prodaja na daljinu: ne/i });
    expect(da).not.toBeChecked();
    expect(ne).not.toBeChecked();
    expect(screen.getByText(/nije odgovoreno/i)).toBeInTheDocument();

    expect(
      screen.getByText(/odgovorite na pitanje o prodaji na daljinu/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/ćutanje se ne računa/i)).toBeInTheDocument();

    await userEvent.click(ne);

    expect(
      screen.queryByText(/odgovorite na pitanje o prodaji na daljinu/i),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/ćutanje se ne računa/i)).not.toBeInTheDocument();
  });

  it("saves an untouched profile with every question still null", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<ShopProfilePanel profile={unset} onSave={onSave} />);

    await userEvent.click(screen.getByRole("button", { name: /sačuvaj profil/i }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          pravnaForma: null,
          pdvObveznik: null,
          distanceSelling: null,
          lpfrInPremises: null,
        }),
      ),
    );
  });

  it("returns prodaja na daljinu to unanswered instead of storing ne", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<ShopProfilePanel profile={unset} onSave={onSave} />);

    await userEvent.click(
      screen.getByRole("radio", { name: /prodaja na daljinu: ne/i }),
    );
    await userEvent.click(
      screen.getByRole("radio", { name: /prodaja na daljinu: nije odgovoreno/i }),
    );
    await userEvent.click(screen.getByRole("button", { name: /sačuvaj profil/i }));

    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].distanceSelling).toBeNull();
    expect(onSave.mock.calls[0][0].distanceSelling).not.toBe(false);
  });

  it("opens the PURS registry through the injected opener, not a target=_blank anchor", async () => {
    const onOpenRegistry = vi.fn().mockResolvedValue(undefined);
    render(
      <ShopProfilePanel
        profile={unset}
        onSave={vi.fn()}
        onOpenRegistry={onOpenRegistry}
      />,
    );

    await userEvent.click(
      screen.getByRole("button", { name: /registar odobrenih elemenata efu/i }),
    );

    expect(onOpenRegistry).toHaveBeenCalledWith(
      "https://www.purs.gov.rs/sr/eFiskalizacija/registar-odobrenih-elemenata-efu.html",
    );
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

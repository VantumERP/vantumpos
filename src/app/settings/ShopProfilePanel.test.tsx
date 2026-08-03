import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ShopProfilePanel } from "./ShopProfilePanel";
import type { LegalNotice, ShopProfile } from "@/services/types";

const unset: ShopProfile = {
  pravnaForma: null,
  pdvObveznik: null,
  distanceSelling: null,
  lpfrInPremises: null,
  lpfrCarveOutInternetOnly: null,
  lpfrCarveOutOwnUsedAssets: null,
  esirElements: [],
};

/**
 * What `settings_lpfr_notice` returns for a shop whose stored legal form is
 * *preduzetnik*. The figure is quoted here only because a test has to assert
 * the panel renders the backend's string verbatim — the panel itself never
 * derives one, and `src-tauri/src/legal.rs` remains the only place a fine
 * figure is decided.
 */
const preduzetnikLpfrNotice: LegalNotice = {
  summary:
    "U svakom poslovnom prostoru i poslovnoj prostoriji mora da radi najmanje " +
    "jedan lokalni procesor fiskalnih računa (L-PFR).",
  penalty: "Prekršaj: novčana kazna od 50.000 do 500.000 dinara (čl. 15 st. 3).",
  citation: "Zakon o fiskalizaciji, čl. 6 st. 4; prekršaj: čl. 15 st. 1 tač. 4.",
  isLegalDuty: true,
};

/** The same notice for a shop that has not yet said what it is. */
const unsetLpfrNotice: LegalNotice = {
  ...preduzetnikLpfrNotice,
  penalty: null,
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

  describe("lokalni PFR (ZF čl. 6 st. 4)", () => {
    it("answers 'ne' with the čl. 6 st. 4 duty instead of silence", async () => {
      render(
        <ShopProfilePanel
          profile={unset}
          onSave={vi.fn()}
          lpfrNotice={unsetLpfrNotice}
        />,
      );

      expect(
        screen.queryByText(/mora da radi najmanje jedan lokalni procesor/i),
      ).not.toBeInTheDocument();

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );

      expect(
        screen.getByText(/mora da radi najmanje jedan lokalni procesor/i),
      ).toBeInTheDocument();
      expect(
        screen.getByText(/čl. 6 st. 4; prekršaj: čl. 15 st. 1 tač. 4/i),
      ).toBeInTheDocument();
    });

    it("keeps the duty standing while a carve-out is merely unanswered", async () => {
      render(
        <ShopProfilePanel
          profile={unset}
          onSave={vi.fn()}
          lpfrNotice={unsetLpfrNotice}
        />,
      );

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );

      // Both carve-outs are still „bez odgovora“. Silence is not an excuse.
      expect(
        screen.getByText(/mora da radi najmanje jedan lokalni procesor/i),
      ).toBeInTheDocument();
    });

    it("drops the notice once either čl. 6 st. 4 carve-out applies", async () => {
      const { unmount } = render(
        <ShopProfilePanel
          profile={unset}
          onSave={vi.fn()}
          lpfrNotice={unsetLpfrNotice}
        />,
      );

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );
      await userEvent.click(
        screen.getByRole("radio", { name: /prodaja isključivo preko interneta: da/i }),
      );

      expect(
        screen.queryByText(/mora da radi najmanje jedan lokalni procesor/i),
      ).not.toBeInTheDocument();

      unmount();

      render(
        <ShopProfilePanel
          profile={unset}
          onSave={vi.fn()}
          lpfrNotice={unsetLpfrNotice}
        />,
      );

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );
      await userEvent.click(
        screen.getByRole("radio", {
          name: /prodaja sopstvenih korišćenih sredstava: da/i,
        }),
      );

      expect(
        screen.queryByText(/mora da radi najmanje jedan lokalni procesor/i),
      ).not.toBeInTheDocument();
    });

    it("quotes the tier the backend resolved, and no figure while the form is unset", async () => {
      const { unmount } = render(
        <ShopProfilePanel
          profile={unset}
          onSave={vi.fn()}
          lpfrNotice={unsetLpfrNotice}
        />,
      );

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );

      expect(screen.queryByText(/50\.000 do 500\.000/)).not.toBeInTheDocument();
      expect(screen.queryByText(/2\.000\.000/)).not.toBeInTheDocument();
      expect(screen.getByText(/da bi iznos kazne bio prikazan/i)).toBeInTheDocument();

      unmount();

      render(
        <ShopProfilePanel
          profile={{ ...unset, pravnaForma: "preduzetnik" }}
          onSave={vi.fn()}
          lpfrNotice={preduzetnikLpfrNotice}
        />,
      );

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );

      expect(screen.getByText(/50\.000 do 500\.000/)).toBeInTheDocument();
      expect(screen.queryByText(/2\.000\.000/)).not.toBeInTheDocument();
    });

    it("withholds the stored figure while an unsaved legal form contradicts it", async () => {
      render(
        <ShopProfilePanel
          profile={{ ...unset, pravnaForma: "preduzetnik" }}
          onSave={vi.fn()}
          lpfrNotice={preduzetnikLpfrNotice}
        />,
      );

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );
      await userEvent.click(screen.getByRole("radio", { name: /^pravno lice/i }));

      // The notice was resolved for the *stored* preduzetnik tier; the radio now
      // says pravno lice. Showing the old figure would quote the wrong tier.
      expect(screen.queryByText(/50\.000 do 500\.000/)).not.toBeInTheDocument();
      expect(screen.getByText(/da bi iznos kazne bio prikazan/i)).toBeInTheDocument();
    });

    it("saves both carve-out answers", async () => {
      const onSave = vi.fn().mockResolvedValue(undefined);
      render(<ShopProfilePanel profile={unset} onSave={onSave} />);

      await userEvent.click(
        screen.getByRole("radio", { name: /lokalni pfr: ne/i }),
      );
      await userEvent.click(
        screen.getByRole("radio", { name: /prodaja isključivo preko interneta: ne/i }),
      );
      await userEvent.click(
        screen.getByRole("radio", {
          name: /prodaja sopstvenih korišćenih sredstava: da/i,
        }),
      );
      await userEvent.click(
        screen.getByRole("button", { name: /sačuvaj profil/i }),
      );

      await waitFor(() =>
        expect(onSave).toHaveBeenCalledWith(
          expect.objectContaining({
            lpfrInPremises: false,
            lpfrCarveOutInternetOnly: false,
            lpfrCarveOutOwnUsedAssets: true,
          }),
        ),
      );
    });
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

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SettingsScreen } from "./SettingsScreen";
import { Toaster } from "@/components/ui/sonner";
import { createMockServices } from "@/services/mock-adapter";
import type { BackupJob, BackupStatus } from "@/services/types";

function renderSettings(services = createMockServices()) {
  render(
    <>
      <SettingsScreen
        services={services}
        usersPanel={<div>Korisnici panel</div>}
      />
      <Toaster />
    </>,
  );
  return services;
}

describe("SettingsScreen", () => {
  it("renders distinct content for each settings tab", async () => {
    const user = userEvent.setup();
    renderSettings();

    expect(await screen.findByLabelText("Naziv radnje")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "PDV" }));
    expect(
      await screen.findByRole("heading", { name: "PDV stope" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Računi" }));
    expect(screen.getByLabelText("Prefiks računa")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Profil" }));
    expect(
      await screen.findByRole("heading", { name: "Profil radnje" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Korisnici" }));
    expect(screen.getByText("Korisnici panel")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Backup" }));
    expect(
      await screen.findByRole("heading", { name: "Status backupa" }),
    ).toBeInTheDocument();
  });

  it("opens the PURS registry through the opener service", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const openExternalUrl = vi
      .spyOn(services.print, "openExternalUrl")
      .mockResolvedValue(undefined);
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Profil" }));
    await user.click(
      await screen.findByRole("button", {
        name: /registar odobrenih elemenata efu/i,
      }),
    );

    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://www.purs.gov.rs/sr/eFiskalizacija/registar-odobrenih-elemenata-efu.html",
    );
  });

  it("saves company settings and shows a success toast", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.clear(await screen.findByLabelText("Naziv radnje"));
    await user.type(screen.getByLabelText("Naziv radnje"), "Vantum Market");
    await user.click(screen.getByRole("button", { name: "Sačuvaj radnju" }));

    expect(
      await screen.findByText("Podešavanja radnje su sačuvana."),
    ).toBeInTheDocument();
  });

  it("renders a VAT validation error without closing the dialog", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Nova PDV stopa" }));
    await user.click(screen.getByRole("button", { name: "Sačuvaj PDV stopu" }));

    expect(
      await screen.findByText("Naziv PDV stope je obavezan."),
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "PDV stopa" })).toBeInTheDocument();
  });

  it("renders the stale warning and a failed backup job", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const failedJob: BackupJob = {
      id: 99,
      backupType: "automatic",
      path: "D:/backups/vantumpos-automatic-99.sqlite3",
      status: "failed",
      errorMessage: "Disk pun.",
      fileSizeBytes: null,
      createdAt: "2026-06-25 10:00:00",
      completedAt: null,
    };
    const staleStatus: BackupStatus = {
      backupFolder: "D:/backups",
      automaticBackupEnabled: true,
      stale: true,
      encryptionConfigured: true,
      lastSuccessfulBackup: null,
      lastFailedBackup: failedJob,
    };
    services.backup.getBackupStatus = async () => staleStatus;
    services.backup.listBackupJobs = async () => [failedJob];

    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Backup" }));

    expect(await screen.findByText("Backup nije napravljen")).toBeInTheDocument();

    const historyRow = screen.getByText(failedJob.path).closest("tr");
    expect(historyRow).not.toBeNull();
    expect(
      within(historyRow as HTMLElement).getByText("Neuspešan"),
    ).toBeInTheDocument();
  });

  it("requires the exact confirmation text before restore", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Backup" }));
    await user.type(
      screen.getByLabelText("Putanja backup fajla"),
      "D:/backup.sqlite3",
    );
    await user.click(screen.getByRole("button", { name: "Vrati backup" }));

    expect(
      screen.getByRole("heading", { name: "Potvrdite restore" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Potvrdi restore" })).toBeDisabled();

    await user.type(screen.getByLabelText("Potvrda"), "VRATI PODATKE");
    expect(screen.getByRole("button", { name: "Potvrdi restore" })).toBeEnabled();
  });

  it("warns when backups are unencrypted and lets an admin set a passphrase", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const setBackupPassphrase = vi.fn().mockResolvedValue(undefined);
    services.backup.setBackupPassphrase = setBackupPassphrase;

    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Backup" }));

    expect(
      await screen.findByText(/Rezervne kopije nisu šifrovane/i),
    ).toBeInTheDocument();

    await user.type(
      screen.getByLabelText(/Lozinka za šifrovanje/i),
      "tajna-lozinka",
    );
    await user.click(screen.getByRole("button", { name: /Postavi lozinku/i }));

    expect(setBackupPassphrase).toHaveBeenCalledWith("tajna-lozinka");
  });

  it("shows the 10-year retention warning in the reset dialog", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Backup" }));
    await user.click(
      await screen.findByRole("button", { name: "Obriši probne podatke" }),
    );

    expect(
      await screen.findByText(/do 10 godina/i),
    ).toBeInTheDocument();
  });

  it("discloses that the reset clears the deklaracija checks on the catalog", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Backup" }));

    // The catalog row survives the reset but its declaration_checked_at stamp
    // does not, so the card must not imply the whole catalog is untouched.
    expect(
      await screen.findByText(/deklaracija proverena se poništavaju/i),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/proveru ponovite pri prvom prijemu robe/i),
    ).toBeInTheDocument();
  });

  it("discloses that the reset deletes the kalkulacija book", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Backup" }));
    await user.click(
      await screen.findByRole("button", { name: "Obriši probne podatke" }),
    );

    // `reset_trading_data` runs DELETE FROM kalkulacije next to the KEP wipe.
    // A kalkulacija is an isprava numbered per book_year, so its deletion is
    // not an implementation detail of „obriši probne račune“ — the owner is
    // losing a numbered book and has to be told so before confirming.
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toHaveTextContent(/knjiga kalkulacija/i);
    expect(dialog).toHaveTextContent(/numeriš[ue] po poslovnoj godini/i);
    expect(dialog).toHaveTextContent(/KEP/);
  });
});

describe("SettingsScreen deposit calendar", () => {
  it("counts Saturday by default and says the law never defined a radni dan", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Kalendar" }));

    expect(
      await screen.findByRole("heading", { name: "Rok za polog gotovine" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Subota je radni dan")).toBeChecked();
    expect(screen.getByText(/nije definisan/i)).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "Božić" })).toBeInTheDocument();
  });

  it("stores a Saturday change through the settings service", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const setSaturdayIsWorking = vi.spyOn(
      services.settings,
      "setSaturdayIsWorking",
    );
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kalendar" }));
    await user.click(await screen.findByLabelText("Subota je radni dan"));

    await waitFor(() =>
      expect(setSaturdayIsWorking).toHaveBeenCalledWith(false),
    );
    expect(
      await screen.findByText(/pomera kasnije/i),
      "excluding Saturdays moves every deadline later — the operator must be told",
    ).toBeInTheDocument();
  });

  it("adds and removes a non-working day", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveNonWorkingDay = vi.spyOn(services.settings, "saveNonWorkingDay");
    const deleteNonWorkingDay = vi.spyOn(
      services.settings,
      "deleteNonWorkingDay",
    );
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kalendar" }));
    await user.type(await screen.findByLabelText("Datum"), "2026-08-05");
    await user.type(screen.getByLabelText("Naziv"), "Slava radnje");
    await user.click(screen.getByRole("button", { name: "Dodaj neradni dan" }));

    await waitFor(() =>
      expect(saveNonWorkingDay).toHaveBeenCalledWith("2026-08-05", "Slava radnje"),
    );

    const addedRow = await screen.findByRole("row", { name: /Slava radnje/ });
    await user.click(within(addedRow).getByRole("button", { name: "Ukloni" }));

    await waitFor(() =>
      expect(deleteNonWorkingDay).toHaveBeenCalledWith("2026-08-05"),
    );
  });

  it("cites the full title, the gazette and the kazna articles", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Kalendar" }));

    const citation = await screen.findByText(
      /Zakon o obavljanju plaćanja pravnih lica, preduzetnika i fizičkih lica koja ne obavljaju delatnost/i,
    );
    expect(citation).toHaveTextContent(/Sl\. glasnik RS, br\. 68\/2015/);
    expect(citation).toHaveTextContent(/čl\. 3 st\. 1/);
    expect(citation).toHaveTextContent(/kazne čl\. 7 st\. 1 tač\. 2\) i st\. 3/);
    expect(citation).toHaveTextContent(/Poreska uprava/);
    // Never the short form, and no fine figure outside legal.rs.
    expect(screen.queryByText(/\bZOP\b/)).not.toBeInTheDocument();
    expect(screen.queryByText(/\d{2}\.000/)).not.toBeInTheDocument();
  });

  it("never claims a prepared list while the holiday table is empty", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    services.settings.getCashDepositCalendar = async () => ({
      saturdayIsWorking: true,
      days: [],
      horizonYear: 2027,
    });
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kalendar" }));

    expect(
      await screen.findByText(/Lista neradnih dana je prazna/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/pripremljena zaključno sa/i),
    ).not.toBeInTheDocument();
  });

  it("frames the holiday table as this shop's list, not a statutory one", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Kalendar" }));

    expect(
      await screen.findByText(/proverite listu za svaku godinu/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/blagajnički maksimum/i)).not.toBeInTheDocument();
  });
});

/**
 * The NBS rate surface is the control the till points a cashier at when the
 * AML čl. 46 st. 1 check cannot run. If it does not exist, `load_eur_rate`
 * stays `None` forever, every AML column on `sales` stays NULL, and the till's
 * own error copy names a control that is not there — so these tests are about
 * the control existing and round-tripping, not only about wording.
 */
describe("SettingsScreen EUR rate", () => {
  it("shows the cached rate, its date and its source", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));

    expect(
      await screen.findByRole("heading", {
        name: "Kurs evra za proveru gotovine",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText(/117,23 RSD za 1 EUR/)).toBeInTheDocument();
    expect(screen.getByText(/18\.06\.2026/)).toBeInTheDocument();
    expect(screen.getByText(/Izvor: NBS/)).toBeInTheDocument();
  });

  it("says plainly that a failed refresh does not block selling", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));

    expect(
      await screen.findByText(/ne blokira prodaju/i),
      "an unreachable NBS may never read as something the cashier must clear first",
    ).toBeInTheDocument();
  });

  it("round-trips a manual rate and clears staleness for that date", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const setManualEurRate = vi.spyOn(services.settings, "setManualEurRate");
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));
    await user.clear(await screen.findByLabelText("Kurs (RSD za 1 EUR)"));
    await user.type(screen.getByLabelText("Kurs (RSD za 1 EUR)"), "119,50");
    await user.clear(screen.getByLabelText("Datum kursa"));
    await user.type(screen.getByLabelText("Datum kursa"), "2026-06-18");
    await user.click(screen.getByRole("button", { name: "Sačuvaj ručni kurs" }));

    // Para per 1 EUR, never a float.
    await waitFor(() =>
      expect(setManualEurRate).toHaveBeenCalledWith(11_950, "2026-06-18"),
    );

    expect(await screen.findByText(/119,50 RSD za 1 EUR/)).toBeInTheDocument();
    expect(screen.getByText(/Izvor: ručno/)).toBeInTheDocument();
    expect(
      screen.queryByText("Kurs nije od današnjeg dana"),
      "a rate entered for today is not stale",
    ).not.toBeInTheDocument();
  });

  it("warns when the stored rate is not today's", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));
    await user.clear(await screen.findByLabelText("Kurs (RSD za 1 EUR)"));
    await user.type(screen.getByLabelText("Kurs (RSD za 1 EUR)"), "119,50");
    await user.clear(screen.getByLabelText("Datum kursa"));
    await user.type(screen.getByLabelText("Datum kursa"), "2026-06-01");
    await user.click(screen.getByRole("button", { name: "Sačuvaj ručni kurs" }));

    const warning = await screen.findByText("Kurs nije od današnjeg dana");
    const alert = warning.closest('[data-slot="alert"]');
    expect(alert).toHaveTextContent(/nosi datum 01\.06\.2026/);
    // The day the verdict was judged against, never an implied „sada".
    expect(alert).toHaveTextContent(/izvršena za 18\.06\.2026/);
  });

  it("refreshes from the NBS through the settings service", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const refreshEurRate = vi.spyOn(services.settings, "refreshEurRate");
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));
    await user.click(
      await screen.findByRole("button", { name: "Osveži kurs sa NBS-a" }),
    );

    await waitFor(() => expect(refreshEurRate).toHaveBeenCalled());
  });

  it("names the absent rate as the reason the check cannot run, and does not call it a breach", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    services.settings.getEurRate = async () => ({
      rate: null,
      isStale: true,
      checkedFor: "2026-06-18",
    });
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));

    expect(await screen.findByText("Kurs nije poznat")).toBeInTheDocument();
    expect(
      screen.getByText(/provera limita gotovine ne može da se izvrši/i),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/prodaja nije blokirana/i),
      "an unknown rate is an unrun check, never a breach",
    ).toBeInTheDocument();
  });

  it("rejects a mistyped rate without pretending it was saved", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    renderSettings(services);

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));
    await user.clear(await screen.findByLabelText("Kurs (RSD za 1 EUR)"));
    // A tenfold typo: it would multiply the AML dinar threshold by ten.
    await user.type(screen.getByLabelText("Kurs (RSD za 1 EUR)"), "1172,30");
    await user.clear(screen.getByLabelText("Datum kursa"));
    await user.type(screen.getByLabelText("Datum kursa"), "2026-06-18");
    await user.click(screen.getByRole("button", { name: "Sačuvaj ručni kurs" }));

    expect(
      await screen.findByText("Kurs mora biti između 50 i 500 dinara za 1 evro."),
    ).toBeInTheDocument();
    expect(screen.getByText(/117,23 RSD za 1 EUR/)).toBeInTheDocument();
  });

  it("says the law names neither the rate nor the conversion day", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));

    const citation = await screen.findByText(
      /Zakon o sprečavanju pranja novca i finansiranja terorizma/i,
    );
    expect(citation).toHaveTextContent(/čl\. 46 st\. 1/);
    expect(
      screen.getByText(/ne imenuje ni kurs ni dan preračuna/i),
      "the srednji-kurs-on-transaction-date rule is inferred from čl. 8 st. 1 tač. 2, not verbatim in čl. 46",
    ).toBeInTheDocument();
    // No fine figure may live outside legal.rs.
    expect(screen.queryByText(/dinara kazn/i)).not.toBeInTheDocument();
  });

  /**
   * Verified-rules §3 req 5: the one-year aggregation limb is a real duty that
   * binds the shop, the software cannot compute it (no customers table, no
   * buyer tag, no rolling 365-day total), and the gap must therefore be
   * **stated in writing** to the shop owner rather than left to be discovered.
   */
  it("states in writing that it cannot aggregate a buyer's cash over one year", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(await screen.findByRole("tab", { name: "Kurs" }));

    const disclosure = (
      await screen.findByText("Program ne sabira uplate istog kupca")
    ).closest('[data-slot="alert"]');

    // The ban reaches „jednu ili više međusobno povezanih gotovinskih
    // transakcija ili jedan ili više ugovora u periodu od godinu dana“.
    expect(disclosure).toHaveTextContent(
      /više međusobno povezanih gotovinskih transakcija/i,
    );
    expect(disclosure).toHaveTextContent(/ugovora u periodu od godinu dana/i);
    // What the software actually does — a limit, never advertised as a feature.
    expect(disclosure).toHaveTextContent(
      /proverava isključivo pojedinačnu prodaju/i,
    );
    expect(disclosure).toHaveTextContent(/ne sabira ranije uplate istog kupca/i);
    // And that the duty binds the shop regardless.
    expect(disclosure).toHaveTextContent(
      /važi za radnju i onda kada je program ne proverava/i,
    );
    expect(disclosure).toHaveTextContent(/46/);
    // Still no fine figure outside legal.rs.
    expect(disclosure).not.toHaveTextContent(/kazn/i);
  });
});

describe("SettingsScreen VAT rates", () => {
  it("edits an existing VAT rate and threads its id to saveTaxRate", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Uredi PDV 20" }));

    const dialog = await screen.findByRole("dialog", { name: "PDV stopa" });
    const nameInput = within(dialog).getByLabelText("Naziv");
    expect(nameInput).toHaveValue("PDV 20");

    await user.clear(nameInput);
    await user.type(nameInput, "PDV 20 standard");
    await user.click(within(dialog).getByRole("button", { name: "Sačuvaj PDV stopu" }));

    await waitFor(() =>
      expect(saveTaxRate).toHaveBeenCalledWith({
        id: 1,
        name: "PDV 20 standard",
        rateBasisPoints: 2000,
        active: true,
      }),
    );
    expect(await screen.findByText("PDV 20 standard")).toBeInTheDocument();
  });

  it("deactivates a VAT rate instead of deleting it", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Deaktiviraj PDV 20" }));

    await waitFor(() =>
      expect(saveTaxRate).toHaveBeenCalledWith({
        id: 1,
        name: "PDV 20",
        rateBasisPoints: 2000,
        active: false,
      }),
    );

    const row = (await screen.findByText("PDV 20")).closest("tr");
    expect(row).not.toBeNull();
    expect(within(row as HTMLElement).getByText("Neaktivna")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Deaktiviraj PDV 20" }),
    ).not.toBeInTheDocument();
  });

  it("blocks saving a VAT rate with an empty name", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Uredi PDV 20" }));

    const dialog = await screen.findByRole("dialog", { name: "PDV stopa" });
    await user.clear(within(dialog).getByLabelText("Naziv"));
    await user.click(within(dialog).getByRole("button", { name: "Sačuvaj PDV stopu" }));

    expect(
      await within(dialog).findByText("Naziv PDV stope je obavezan."),
    ).toBeInTheDocument();
    expect(saveTaxRate).not.toHaveBeenCalled();
  });
});

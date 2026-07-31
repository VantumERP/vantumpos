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

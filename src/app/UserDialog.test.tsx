import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { UserDialog, UsersScreen } from "./AppShell";
import type { PosServices } from "@/services/ports";
import type { EmployeeProfile, SaveUserRequest, UserAccount } from "@/services/types";

const kasirka: UserAccount = {
  id: 7,
  username: "jelena",
  displayName: "Jelena Đurić",
  role: "cashier",
  active: true,
  createdAt: "2026-01-05T08:00:00Z",
  updatedAt: "2026-01-05T08:00:00Z",
  lastLoginAt: null,
};

const admin: UserAccount = {
  ...kasirka,
  id: 1,
  username: "admin",
  displayName: "Administrator",
  role: "admin",
};

const prazanProfil: EmployeeProfile = {
  datumRodjenja: null,
  datumRodjenjaNajmladjegDeteta: null,
  samohraniRoditelj: null,
  deteTezakInvalid: null,
  trudnocaIliDojenje: null,
  trudnocaIliDojenjeOd: null,
  radiUPreraspodeli: false,
  ugovorenoRadnoVremeMinutaNedeljno: null,
  zanimanjeSifra: null,
  kvalifikacijaSifra: null,
  saglasnostPrekovremeniOd: null,
};

function renderDialog(overrides: Partial<Parameters<typeof UserDialog>[0]> = {}) {
  const onSave = vi.fn<(request: SaveUserRequest) => Promise<void>>(async () => {});

  render(
    <UserDialog
      open
      user={null}
      profile={null}
      onOpenChange={() => {}}
      onSave={onSave}
      {...overrides}
    />,
  );

  return { onSave };
}

function dialog() {
  return screen.getByRole("dialog");
}

describe("UserDialog — profil zaposlenog", () => {
  it("uz svako polje zaštite navodi član koji ga uvodi", () => {
    renderDialog();

    const forma = dialog();
    expect(screen.getByLabelText(/Datum rođenja$/)).toBeInTheDocument();
    expect(
      screen.getByLabelText(/Datum rođenja najmlađeg deteta/),
    ).toBeInTheDocument();
    expect(screen.getByLabelText(/Samohrani roditelj/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Trudnoća ili dojenje/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Preraspodela radnog vremena/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Ugovoreno radno vreme/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Šifra zanimanja/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Šifra kvalifikacije/)).toBeInTheDocument();

    for (const clan of [
      /ZoR čl\. 87/,
      /ZoR čl\. 88 st\. 1/,
      /ZoR čl\. 90/,
      /ZoR čl\. 91 st\. 1/,
      /ZoR čl\. 91 st\. 2/,
      /ZoR čl\. 51 st\. 1/,
      /ZoR čl\. 58/,
      /ZEOR čl\. 44 st\. 2/,
    ]) {
      // More than one field may rest on the same article — čl. 91 st. 2 carries
      // both legs, and both ZEOR codes come from the same duty.
      expect(within(forma).queryAllByText(clan).length).toBeGreaterThan(0);
    }
  });

  it("nosi drugu alternativu čl. 91 st. 2, koja nema starosnu granicu", () => {
    renderDialog();

    expect(screen.getByLabelText(/težak invalid/i)).toBeInTheDocument();
    expect(
      within(dialog()).getByText(/nema starosne granice/i),
    ).toBeInTheDocument();
  });

  it("uz drugu alternativu čl. 91 st. 2 imenuje uslov „Samohrani roditelj: Da“", () => {
    renderDialog();

    // Subjekt čl. 91 st. 2 je samohrani roditelj u obe alternative, pa i provera
    // u worktime.rs traži samohranog roditelja. Bez te zavisnosti u tekstu
    // operater koji upiše samo „Dete je težak invalid: Da“ ostaje bez ijedne
    // provere saglasnosti, a tekst mu je rekao da provera od tog trenutka važi.
    const polje = screen
      .getByLabelText(/težak invalid/i)
      .closest("[data-slot=field]");
    expect(polje).not.toBeNull();
    expect(
      within(polje as HTMLElement).getByText(/samohran/i),
    ).toBeInTheDocument();
  });

  it("sadržaj drži u sopstvenom skroleru, da dugme za čuvanje ostane dohvatljivo", () => {
    renderDialog();

    // jsdom ne računa raspored, pa se pravilo proverava na klasama popupa:
    // osnovni DialogContent je `fixed` i centriran, bez ograničenja visine i bez
    // skrolovanja. Sa sedamnaest polja profila DialogFooter („Sačuvaj
    // korisnika“) bi ispao van ekrana i profil čl. 87–91 se ne bi mogao sačuvati.
    const popup = dialog();
    expect(popup).toHaveAttribute("data-slot", "dialog-content");
    expect(popup.className).toMatch(/\bmax-h-\[\d+vh\]/);
    expect(popup.className).toContain("overflow-y-auto");
  });

  it("ne prikuplja saglasnost — evidentira da pisana saglasnost postoji i od kada", () => {
    renderDialog();

    const forma = dialog();
    expect(within(forma).queryAllByRole("checkbox")).toHaveLength(0);
    expect(
      within(forma).queryByText(
        /dajem saglasnost|saglasan sam|saglasna sam|prihvatam|pristajem/i,
      ),
    ).toBeNull();

    const saglasnost = screen.getByLabelText(/Pisana saglasnost za prekovremeni rad/);
    expect(saglasnost).toHaveAttribute("type", "date");
    expect(within(forma).getByText(/ne prikuplja/i)).toBeInTheDocument();
  });

  it("ne prikazuje saglasnost za preraspodelu iz čl. 57 st. 4", () => {
    renderDialog();

    const forma = dialog();
    expect(within(forma).queryByText(/čl\. 57 st\. 4/)).toBeNull();
    expect(
      within(forma).queryByText(/saglasnost[^.]*preraspodel|preraspodel[^.]*saglasnost/i),
    ).toBeNull();
    expect(
      screen.queryByLabelText(/saglasnost[^.]*preraspodel/i),
    ).toBeNull();
  });

  it("trudnoću i dojenje vodi kao oznaku i datum, bez ijednog polja za slobodan tekst", () => {
    renderDialog();

    const forma = dialog();
    expect(forma.querySelectorAll("textarea")).toHaveLength(0);

    const oznaka = screen.getByLabelText(/Trudnoća ili dojenje/);
    expect(oznaka.tagName).toBe("SELECT");
    expect(screen.getByLabelText(/Nalaz važi od/)).toHaveAttribute("type", "date");
    expect(
      within(forma).getByText(/dijagnoza i medicinska dokumentacija se ne unose/i),
    ).toBeInTheDocument();
  });

  it("povlačenjem oznake čl. 90 briše i datum nalaza, pa se profil može sačuvati", async () => {
    const user = userEvent.setup();
    const { onSave } = renderDialog({
      user: kasirka,
      profile: {
        ...prazanProfil,
        trudnocaIliDojenje: true,
        trudnocaIliDojenjeOd: "2026-05-04",
      },
    });

    const datum = screen.getByLabelText(/Nalaz važi od/);
    expect(datum).toHaveValue("2026-05-04");
    expect(datum).not.toBeDisabled();

    // Prestanak evidencije po čl. 90 je obična radnja: oznaka ide na „Ne“.
    // Ako datum ostane, čuvanje pada na pravilu uparivanja u users.rs i oznaka
    // se ne može povući — a to je polje koje se najpre mora moći povući.
    fireEvent.change(screen.getByLabelText(/Trudnoća ili dojenje/), {
      target: { value: "ne" },
    });

    expect(datum).toHaveValue("");
    expect(datum).toBeDisabled();

    await user.click(screen.getByRole("button", { name: /Sačuvaj korisnika/ }));

    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].profile).toEqual({
      ...prazanProfil,
      trudnocaIliDojenje: false,
      trudnocaIliDojenjeOd: null,
    });
  });

  it("prikazuje sačuvani profil zaposlenog", () => {
    renderDialog({
      user: kasirka,
      profile: {
        ...prazanProfil,
        datumRodjenja: "1994-04-11",
        samohraniRoditelj: true,
        deteTezakInvalid: true,
        radiUPreraspodeli: true,
        ugovorenoRadnoVremeMinutaNedeljno: 2400,
        zanimanjeSifra: "5223.02",
        saglasnostPrekovremeniOd: "2026-06-01",
      },
    });

    expect(screen.getByLabelText(/Datum rođenja$/)).toHaveValue("1994-04-11");
    expect(screen.getByLabelText(/Samohrani roditelj/)).toHaveValue("da");
    expect(screen.getByLabelText(/težak invalid/i)).toHaveValue("da");
    expect(screen.getByLabelText(/Preraspodela radnog vremena/)).toHaveValue("da");
    expect(screen.getByLabelText(/Ugovoreno radno vreme/)).toHaveValue(2400);
    expect(screen.getByLabelText(/Šifra zanimanja/)).toHaveValue("5223.02");
    expect(
      screen.getByLabelText(/Pisana saglasnost za prekovremeni rad/),
    ).toHaveValue("2026-06-01");
  });

  it("šalje profil zajedno sa nalogom", async () => {
    const user = userEvent.setup();
    const { onSave } = renderDialog({ user: kasirka, profile: prazanProfil });

    fireEvent.change(screen.getByLabelText(/Datum rođenja$/), {
      target: { value: "1994-04-11" },
    });
    fireEvent.change(screen.getByLabelText(/Samohrani roditelj/), {
      target: { value: "da" },
    });
    fireEvent.change(screen.getByLabelText(/Trudnoća ili dojenje/), {
      target: { value: "da" },
    });
    fireEvent.change(screen.getByLabelText(/Nalaz važi od/), {
      target: { value: "2026-05-04" },
    });
    fireEvent.change(screen.getByLabelText(/Ugovoreno radno vreme/), {
      target: { value: "2400" },
    });
    fireEvent.change(screen.getByLabelText(/Šifra zanimanja/), {
      target: { value: "5223.02" },
    });

    await user.click(screen.getByRole("button", { name: /Sačuvaj korisnika/ }));

    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].profile).toEqual({
      ...prazanProfil,
      datumRodjenja: "1994-04-11",
      samohraniRoditelj: true,
      trudnocaIliDojenje: true,
      trudnocaIliDojenjeOd: "2026-05-04",
      ugovorenoRadnoVremeMinutaNedeljno: 2400,
      zanimanjeSifra: "5223.02",
    });
  });
});

describe("UsersScreen — otvaranje profila", () => {
  function services(profile: EmployeeProfile) {
    return {
      users: {
        listUsers: vi.fn().mockResolvedValue([kasirka]),
        createUser: vi.fn(),
        updateUser: vi.fn(),
        deactivateUser: vi.fn(),
        getEmployeeProfile: vi.fn().mockResolvedValue(profile),
      },
    } as unknown as PosServices;
  }

  it("učitava profil zaposlenog tek kad se otvori izmena naloga", async () => {
    const user = userEvent.setup();
    const posServices = services({
      ...prazanProfil,
      datumRodjenja: "1994-04-11",
      zanimanjeSifra: "5223.02",
    });

    render(<UsersScreen services={posServices} currentUser={admin} />);

    await screen.findByText("Jelena Đurić");
    expect(posServices.users.getEmployeeProfile).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Uredi" }));

    await waitFor(() =>
      expect(posServices.users.getEmployeeProfile).toHaveBeenCalledWith(7),
    );
    await waitFor(() =>
      expect(screen.getByLabelText(/Datum rođenja$/)).toHaveValue("1994-04-11"),
    );
    expect(screen.getByLabelText(/Šifra zanimanja/)).toHaveValue("5223.02");
  });
});

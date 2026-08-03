import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { PopisModule, sledeciKorak } from "./PopisModule";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";

type User = ReturnType<typeof userEvent.setup>;

interface OtvoriOpcije {
  potvrdiUskladjivanje?: boolean;
  clan?: { ime: string; rukujeImovinom: boolean };
  vrsta?: "godisnji" | "nivelacioni";
}

/** Fills the „novi popis“ form and submits it. */
async function otvoriPopis(user: User, opcije: OtvoriOpcije = {}) {
  const { potvrdiUskladjivanje = true, clan, vrsta } = opcije;

  await user.click(await screen.findByRole("button", { name: /novi popis/i }));
  if (vrsta) {
    await user.selectOptions(screen.getByLabelText(/vrsta popisa/i), vrsta);
  }
  await user.clear(screen.getByLabelText(/prodajno mesto/i));
  await user.type(screen.getByLabelText(/prodajno mesto/i), "Butik Centar");
  await user.clear(screen.getByLabelText(/datum popisa/i));
  await user.type(screen.getByLabelText(/datum popisa/i), "2026-12-31");

  if (clan) {
    await user.type(screen.getByLabelText(/ime člana komisije/i), clan.ime);
    if (clan.rukujeImovinom) {
      await user.click(
        screen.getByRole("checkbox", { name: /rukuje imovinom/i }),
      );
    }
    await user.click(screen.getByRole("button", { name: /dodaj člana/i }));
  }

  if (potvrdiUskladjivanje) {
    await user.click(
      screen.getByRole("checkbox", { name: /usklađiv/i }),
    );
  }

  await user.click(screen.getByRole("button", { name: /^otvori popis$/i }));
}

function services(): PosServices {
  return createMockServices();
}

describe("navigation", () => {
  it("exposes Popis as an admin-only item beside the other ledgers", () => {
    const item = navigationItems.find((candidate) => candidate.id === "popis");

    expect(item).toMatchObject({ label: "Popis", adminOnly: true });
  });
});

describe("PopisModule — the ZoRač čl. 20 st. 3 gate (req. 39)", () => {
  /**
   * The ordering is legislated, not recommended: the ledgers are reconciled and
   * only then is the popis taken. The confirmation is therefore a statement the
   * shop makes, with the article on it — not a checkbox labelled „potvrđujem“.
   */
  it("names the reconciliation the shop is confirming", async () => {
    render(<PopisModule services={services()} />);

    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /novi popis/i }));

    expect(
      screen.getByText(/glavne knjige sa dnevnikom/i),
    ).toBeInTheDocument();
    // Twice on this screen: on the confirmation itself, and in the nivelacija
    // napomena that says the app opens no popis on the shop's behalf.
    expect(screen.getAllByText(/ZoRač čl\. 20 st\. 3/).length).toBeGreaterThan(0);
  });

  it("surfaces the refusal when the popis is opened without it", async () => {
    const user = userEvent.setup();
    const svc = services();
    render(<PopisModule services={svc} />);

    await otvoriPopis(user, { potvrdiUskladjivanje: false });

    expect(
      await screen.findByText(/Popis se ne može otvoriti/i),
    ).toBeInTheDocument();
    expect(await svc.popis.list()).toHaveLength(0);
  });

  it("opens the popis once the reconciliation is confirmed", async () => {
    const user = userEvent.setup();
    const svc = services();
    render(<PopisModule services={svc} />);

    await otvoriPopis(user);

    await waitFor(async () => expect(await svc.popis.list()).toHaveLength(1));
    // Once in the list row and once on the opened popis.
    expect(await screen.findAllByText(/priprema popisa/i)).toHaveLength(2);
  });
});

describe("PopisModule — the komisija (req. 40)", () => {
  /**
   * PoP čl. 5 st. 1 excludes the person who handles the goods, and čl. 6 st. 2
   * applies the commission provisions *shodno* to a single person — but whether
   * the exclusion follows him is unresolved (§6 R-5). So the app warns and the
   * popis goes on: refusing would enforce a duty nobody has settled, in a 2–3
   * person boutique where everyone handles stock.
   */
  it("warns but does not block on a goods-handling commission member", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: true },
    });

    expect(await screen.findByText(/PoP čl\. 5 st\. 1/)).toBeInTheDocument();
    expect(screen.getByText(/Popis nije zaustavljen/i)).toBeInTheDocument();
    // Not blocked: the next statutory step is still on offer.
    expect(
      screen.getByRole("button", { name: /započni brojanje/i }),
    ).toBeEnabled();
  });

  it("raises no such warning for a member who does not handle the goods", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });

    expect(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/PoP čl\. 5 st\. 1/)).not.toBeInTheDocument();
  });
});

describe("PopisModule — the nivelacija scope (req. 33)", () => {
  /**
   * Neither ZoRač čl. 21 nor PoP čl. 3 scopes the count. Narrowing it to the
   * repriced articles is borrowed by analogy from a regime this customer cannot
   * use, so it is offered as a preporuka with its reasoning visible — and it
   * must never read as „the law says count only these“.
   */
  it("labels the nivelacija scope narrowing a preporuka, never an obaveza", async () => {
    const svc = services();
    const pregled = await svc.popis.nivelacijaPregled();
    render(<PopisModule services={svc} />);

    const obim = await screen.findByRole("group", { name: /obim popisa/i });

    // `pravniStatus` is the one string that says the narrowing is not a duty,
    // so it must travel with the option wherever the option is rendered — read
    // off the service rather than retyped here, or this is a third copy of the
    // copy the whole requirement is about.
    for (const opcija of pregled.obavestenje.obuhvat) {
      expect(within(obim).getByText(opcija.pravniStatus)).toBeInTheDocument();
      expect(within(obim).getByText(opcija.obrazlozenje)).toBeInTheDocument();
    }

    expect(within(obim).getAllByText(/preporuka/i).length).toBeGreaterThan(0);
    expect(
      within(obim).getAllByText(/nije zakonska obaveza/i).length,
    ).toBeGreaterThan(0);
  });

  /**
   * The mechanical guard the backend copy carries too: no duty word may sit in
   * the scope block. „obaveza“ is deliberately absent from the list — the copy
   * has to be able to DENY one.
   */
  it("puts no duty word on the narrowing", async () => {
    render(<PopisModule services={services()} />);

    const obim = await screen.findByRole("group", { name: /obim popisa/i });
    const tekst = obim.textContent ?? "";

    for (const rec of [
      "morate",
      "dužni ste",
      "dužan je",
      "obavezno",
      "obavezan je",
      "nalaže",
    ]) {
      expect(tekst.toLowerCase()).not.toContain(rec);
    }
  });

  /**
   * The duty itself is the other half and it must be stated: a price change
   * raises a popis (ZoRač čl. 21, PoP čl. 3), the app opens none on the shop's
   * behalf, and the report says which price moves it follows.
   */
  it("states the čl. 21 duty, its rok and what the report is built from", async () => {
    render(<PopisModule services={services()} />);

    // The duty and its authority — one sentence and the citation beside it.
    expect(
      (await screen.findAllByText(/ZoRač čl\. 21, PoP čl\. 3/)).length,
    ).toBeGreaterThan(0);
    expect(screen.getByText(/30 dana po izvršenom popisu/i)).toBeInTheDocument();
    expect(
      screen.getByText(/ne otvara popis umesto vas/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/Ne prate se/i)).toBeInTheDocument();
  });
});

describe("sledeciKorak — one arrow per state", () => {
  /**
   * The five arrows of `crate::popis::advance`, and nothing after the
   * knjiženje. A table that offered two steps from one state, or any step from
   * `posted`, would put a button on screen the state machine refuses by name.
   */
  it("maps each state onto exactly the step the bylaw allows next", () => {
    expect(sledeciKorak("draft")).toMatchObject({
      label: "Započni brojanje",
      potpis: false,
    });
    expect(sledeciKorak("counting")).toMatchObject({
      label: "Potpiši stvarno stanje",
      pravniOsnov: "PoP čl. 8 st. 5",
      potpis: true,
    });
    expect(sledeciKorak("counted_signed")).toMatchObject({
      label: "Obračunaj razlike",
      potpis: false,
    });
    expect(sledeciKorak("computed")).toMatchObject({
      label: "Potpiši obračunate liste",
      pravniOsnov: "PoP čl. 9 st. 3",
      potpis: true,
    });
    expect(sledeciKorak("computed_signed")).toMatchObject({
      label: "Proknjiži popis",
      pravniOsnov: "PoP čl. 14 st. 3",
      potpis: false,
    });
    expect(sledeciKorak("posted")).toBeNull();
  });

  /**
   * Req. 31 — and it is exactly the two čl. 8 st. 5 and čl. 9 st. 3 steps that
   * are signings on paper. A step wrongly marked `potpis` would ask for signers
   * where the bylaw asks for none; one wrongly unmarked would take the potpis
   * without ever telling the shop to print and sign.
   */
  it("marks the two statutory signature events and only those", () => {
    const potpisni = (
      [
        "draft",
        "counting",
        "counted_signed",
        "computed",
        "computed_signed",
      ] as const
    ).filter((status) => sledeciKorak(status)?.potpis === true);

    expect(potpisni).toEqual(["counting", "computed"]);
  });
});

describe("PopisModule — the statutory sequence", () => {
  /**
   * One arrow per state and no others. A „Proknjiži“ button on a draft popis
   * would be an action the state machine refuses by name — an affordance that
   * exists only to be rejected.
   */
  it("offers only the next statutory step, and offers none once posted", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });

    await screen.findByRole("button", { name: /započni brojanje/i });
    expect(
      screen.queryByRole("button", { name: /proknjiži/i }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /započni brojanje/i }));
    await screen.findByRole("button", { name: /potpiši stvarno stanje/i });
    expect(
      screen.queryByRole("button", { name: /započni brojanje/i }),
    ).not.toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: /potpiši stvarno stanje/i }),
    );
    await screen.findByRole("button", { name: /obračunaj razlike/i });

    await user.click(screen.getByRole("button", { name: /obračunaj razlike/i }));
    await screen.findByRole("button", { name: /potpiši obračunate liste/i });

    await user.click(
      screen.getByRole("button", { name: /potpiši obračunate liste/i }),
    );
    await screen.findByRole("button", { name: /proknjiži popis/i });

    await user.click(screen.getByRole("button", { name: /proknjiži popis/i }));

    expect(
      (await screen.findAllByText(/proknjižen popis/i)).length,
    ).toBeGreaterThan(0);
    expect(
      screen.queryByRole("button", { name: /proknjiži popis/i }),
    ).not.toBeInTheDocument();
    // Req. 41 / ZoRač čl. 8 st. 4 — the way back in is a new document. Said
    // twice: once by the module, once by the locked count sheet.
    expect(screen.getAllByText(/novim popisom/i)).toHaveLength(2);
  });

  /**
   * Req. 31. PoP čl. 9 st. 3 says *„uz štampanje“* expressly, and a purely
   * electronic signature is an unverified deviation (§6 R-6). Clicking here
   * records that the members signed the printed liste; it is not itself a
   * signature, and the copy must not let the shop believe otherwise.
   */
  it("says the potpis is recorded, not made, and that the liste are printed", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user);
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    expect(await screen.findByText(/odštampajte/i)).toBeInTheDocument();
    expect(screen.getByText(/PoP čl\. 9 st\. 3/)).toBeInTheDocument();
    expect(
      screen.getByText(/ne zamenjuje potpis na papiru/i),
    ).toBeInTheDocument();
  });
});

describe("PopisModule — the čl. 9 st. 2 shortcut (req. 34)", () => {
  /**
   * The reference is checked against a posted in-year popis and NOT against the
   * čl. 14 st. 2 odluka o usvajanju, because nothing in the schema records that
   * decision. A field that implied both were verified would let a shop rely on
   * a check the app does not make.
   */
  it("says which limb of čl. 9 st. 2 is checked and which is not", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await user.click(await screen.findByRole("button", { name: /novi popis/i }));

    expect(screen.getByText(/PoP čl\. 9 st\. 2/)).toBeInTheDocument();
    expect(screen.getByText(/ne proverava/i)).toBeInTheDocument();
  });
});

describe("PopisModule — the count sheet and the izveštaj", () => {
  it("mounts the blind count sheet and no izveštaj composition during counting", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user);
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    // The count sheet is there and blind (req. 29).
    expect(
      await screen.findByRole("button", { name: /sačuvaj stavku/i }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/knjigovodstvena količina/i)).toBeNull();
    // And the izveštaj is not composable yet (čl. 8 st. 5).
    expect(
      screen.queryByRole("button", { name: /sastavi izveštaj/i }),
    ).not.toBeInTheDocument();
  });

  /**
   * PoP čl. 2 st. 6 — a signed copy of the konsignaciona lista is owed to the
   * owner within ten days of the count. It is derived from the liste, it
   * carries the rok, and it says plainly that the app does not deliver it.
   */
  it("raises the čl. 2 st. 6 reminder once tuđa roba is on a lista", async () => {
    const user = userEvent.setup();
    const svc = services();
    render(<PopisModule services={svc} />);

    await otvoriPopis(user);
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    await user.selectOptions(
      screen.getByLabelText(/popisna lista/i),
      "konsignacija",
    );
    await user.type(screen.getByLabelText(/^naziv/i), "Haljina — komision");
    await user.clear(screen.getByLabelText(/stvarna količina/i));
    await user.type(screen.getByLabelText(/stvarna količina/i), "2");
    await user.click(screen.getByRole("button", { name: /sačuvaj stavku/i }));

    expect(await screen.findByText(/PoP čl\. 2 st\. 6/)).toBeInTheDocument();
    expect(screen.getByText(/2027-01-10/)).toBeInTheDocument();
    expect(screen.getByText(/ne dostavlja/i)).toBeInTheDocument();
  });
});

describe("PopisModule — the req. 36 declaration, taken in time", () => {
  /**
   * The gate refuses a *declared* category whose lista is empty. Asked at the
   * izveštaj it can only ever fire after the čl. 8 st. 5 potpis, and by then
   * `counted_signed` refuses every line write and `computed` refuses a new
   * stavka by name — so the only remedy left is a whole new popis. Asked during
   * the count, the remedy is one stavka: this walks exactly that.
   */
  it("surfaces the empty declared lista while the shop can still fill it", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user);
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    await user.click(
      await screen.findByRole("checkbox", { name: /gotovina po apoenima/i }),
    );

    expect(
      await screen.findByText(/popisne liste nisu potpune/i),
    ).toBeInTheDocument();
    // The door is still open — this is the whole point of asking here.
    expect(
      screen.getByRole("button", { name: /potpiši stvarno stanje/i }),
    ).toBeEnabled();

    await user.selectOptions(screen.getByLabelText(/popisna lista/i), "gotovina");
    await user.type(screen.getByLabelText(/^naziv/i), "Novčanica 1.000");
    await user.clear(screen.getByLabelText(/stvarna količina/i));
    await user.type(screen.getByLabelText(/stvarna količina/i), "5");
    await user.click(screen.getByRole("button", { name: /sačuvaj stavku/i }));

    await waitFor(() =>
      expect(screen.queryByText(/popisne liste nisu potpune/i)).toBeNull(),
    );
    expect(
      screen.getByText(/sve prijavljene kategorije imaju bar jednu stavku/i),
    ).toBeInTheDocument();
  });

  /**
   * The declaration taken during the count is the one the izveštaj is judged
   * against — one answer, carried forward, not two independent ones that can
   * disagree about what the shop said it had.
   */
  it("carries the count-phase declaration through to the izveštaj", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    await user.click(
      await screen.findByRole("checkbox", { name: /gotovina po apoenima/i }),
    );
    await user.selectOptions(screen.getByLabelText(/popisna lista/i), "gotovina");
    await user.type(screen.getByLabelText(/^naziv/i), "Novčanica 1.000");
    await user.clear(screen.getByLabelText(/stvarna količina/i));
    await user.type(screen.getByLabelText(/stvarna količina/i), "5");
    await user.click(screen.getByRole("button", { name: /sačuvaj stavku/i }));

    await user.click(
      await screen.findByRole("button", { name: /potpiši stvarno stanje/i }),
    );
    await user.click(
      await screen.findByRole("button", { name: /obračunaj razlike/i }),
    );

    expect(
      await screen.findByText(
        /prijavljene kategorije uz popisne liste: gotovina po apoenima/i,
      ),
    ).toBeInTheDocument();
  });

  /**
   * All six liste stand on the sheet from the first moment, each with the
   * article that requires it. Without them the five posebne liste of čl. 10
   * st. 3, čl. 10 st. 4, čl. 11 st. 1, čl. 12 st. 2 and čl. 2 st. 5 are
   * invisible until somebody guesses they exist.
   */
  it("shows all six liste with their article during the count", async () => {
    const user = userEvent.setup();
    const svc = services();
    render(<PopisModule services={svc} />);

    await otvoriPopis(user);
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    const session = (await svc.popis.list())[0];
    const pregled = await svc.popis.get(session.id);
    expect(pregled.liste).toHaveLength(6);

    for (const lista of pregled.liste) {
      expect(
        await screen.findByRole("checkbox", { name: lista.naziv }),
      ).toBeInTheDocument();
      expect(screen.getAllByText(lista.pravniOsnov).length).toBeGreaterThan(0);
    }
  });
});

describe("PopisModule — the čl. 8 st. 4 handover list on a nivelacija popis", () => {
  /**
   * Req. 33 / PoP čl. 8 st. 4 — „листе са номенклатурним бројевима, називима,
   * врсти и јединицама мере“. Four fields and no količina among them: the list
   * is read before anything is counted, so a stanje beside each article would
   * hand the book quantities over at the very start of the count (req. 29).
   */
  it("hands over the four čl. 8 st. 4 fields and no quantity", async () => {
    const user = userEvent.setup();
    const svc = services();
    render(<PopisModule services={svc} />);

    await otvoriPopis(user, { vrsta: "nivelacioni" });

    const blok = await screen.findByRole("group", {
      name: /artikli za popis po nivelaciji/i,
    });

    // Exactly four columns and not one more — an „očekivano“ or „stanje“ column
    // added here is the earliest čl. 8 st. 5 leak there is.
    expect(
      within(blok)
        .getAllByRole("columnheader")
        .map((zaglavlje) => zaglavlje.textContent),
    ).toEqual(["Nomenklaturni broj", "Naziv", "Vrsta", "Jedinica mere"]);

    const obuhvat = await svc.popis.nivelacijaObuhvat(1, null);
    for (const artikal of obuhvat.artikli) {
      expect(within(blok).getByText(artikal.naziv)).toBeInTheDocument();
    }
    // The seeded perpetual stanje of „Mleko 1 l“ — 3 kom — reaches no cell.
    expect(within(blok).queryByText("3 kom")).toBeNull();
  });

  /**
   * The scope is a control and not a label. Design §3 offers the narrowing as a
   * default with its reasoning visible and §7 t. 8 forbids calling it required
   * — a block that printed both options and let the shop pick neither would
   * make „obim slobodno proširite“ an instruction with nothing behind it.
   */
  it("lets the shop widen the scope and asks the backend for the wider one", async () => {
    const user = userEvent.setup();
    const svc = services();
    const spy = vi.spyOn(svc.popis, "nivelacijaObuhvat");
    render(<PopisModule services={svc} />);

    await otvoriPopis(user, { vrsta: "nivelacioni" });

    const izbor = await screen.findByLabelText(/obim za ovu listu/i);
    // The backend applied its own default and said which one it applied.
    await waitFor(() => expect(izbor).toHaveValue("samo_nivelisani"));
    expect(spy).toHaveBeenCalledWith(1, null);

    await user.selectOptions(izbor, "ceo_objekat");

    await waitFor(() => expect(spy).toHaveBeenCalledWith(1, "ceo_objekat"));
    expect(izbor).toHaveValue("ceo_objekat");
  });

  /** A godišnji popis has no nivelacija scope, and the backend refuses one. */
  it("offers the handover list only on a nivelacija popis", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user);
    await screen.findByRole("button", { name: /započni brojanje/i });

    expect(
      screen.queryByRole("group", { name: /artikli za popis po nivelaciji/i }),
    ).toBeNull();
  });
});

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { PopisModule, sledeciKorak } from "./PopisModule";
import { navigationItems } from "@/app/navigation";
import { Toaster } from "@/components/ui/sonner";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { PopisStatus } from "@/services/types";

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

/**
 * Walks an already-opened popis forward through the module's own buttons and
 * stops in `cilj`. Only the arrows `sledeciKorak` offers are ever clicked, so a
 * fixture can never reach a state the state machine refuses to produce — which
 * matters most for `computed`, the first state in which book quantities exist
 * at all.
 */
async function dovediDo(user: User, cilj: PopisStatus) {
  const putanja: [PopisStatus, RegExp][] = [
    ["counting", /započni brojanje/i],
    ["counted_signed", /potpiši stvarno stanje/i],
    ["computed", /obračunaj razlike/i],
    ["computed_signed", /potpiši obračunate liste/i],
    ["posted", /proknjiži popis/i],
  ];
  const ciljIndex = putanja.findIndex(([status]) => status === cilj);

  // The module offers exactly one arrow, so the arrow on screen is where the
  // popis already stands. A helper that always started at „Započni brojanje“
  // could not walk a popis it had itself already moved.
  let pocetak = 0;
  await waitFor(() => {
    pocetak = putanja.findIndex(
      ([, dugme]) => screen.queryByRole("button", { name: dugme }) !== null,
    );
    expect(pocetak).toBeGreaterThanOrEqual(0);
  });

  for (let korak = pocetak; korak <= ciljIndex; korak += 1) {
    await user.click(
      await screen.findByRole("button", { name: putanja[korak][1] }),
    );
  }
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
   *
   * **And it must cite the article of the potpis actually being taken.** There
   * are two, and they are not the same act: the `counting` step is the čl. 8
   * st. 5 potpis on the counted liste — the one that releases the book data —
   * while čl. 9 st. 3's *„uz štampanje“* governs the obračunate liste signed
   * after it. A paragraph that printed „PoP čl. 9 st. 3“ under both left the
   * operator reading two different provisions for one act, one of which does not
   * govern it. So the article is carried on `Korak` beside the label and both
   * phases are asserted; the same `pravniOsnov` the step header already shows.
   */
  it("cites the article of the potpis being taken, in both phases", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    // Phase A — čl. 8 st. 5, and čl. 9 st. 3 must not appear on it at all.
    const fazaA = await screen.findByText(/ne zamenjuje potpis na papiru/i);
    expect(fazaA).toHaveTextContent(/PoP čl\. 8 st\. 5/);
    expect(fazaA).not.toHaveTextContent(/čl\. 9 st\. 3/);
    expect(await screen.findByText(/odštampajte/i)).toBeInTheDocument();

    // Phase B — the čl. 9 st. 3 potpis on the obračunate liste.
    await user.click(
      screen.getByRole("button", { name: /potpiši stvarno stanje/i }),
    );
    await user.click(
      await screen.findByRole("button", { name: /obračunaj razlike/i }),
    );
    await screen.findByRole("button", { name: /potpiši obračunate liste/i });

    const fazaB = screen.getByText(/ne zamenjuje potpis na papiru/i);
    expect(fazaB).toHaveTextContent(/PoP čl\. 9 st\. 3/);
    expect(fazaB).not.toHaveTextContent(/čl\. 8 st\. 5/);
  });

  /**
   * A string must not state the opposite of what the program does, and that runs
   * in both directions. This paragraph used to end „program ih ne štampa i ne
   * izvozi“, which was true until `popis_export_lista` shipped and false the
   * moment it did: the backend now renders both popisne liste and writes them to
   * `exports/`. Flipping the sentence the other way would be the same defect
   * again — this panel has no button on it, so a sentence announcing an export
   * would send the operator hunting for one. So the paragraph makes no claim
   * about the program at all: it states the operator's duty and what recording
   * the potpis here does and does not do.
   *
   * The izveštaj carries the opposite sentence and it is still true — čl. 13
   * st. 1's izveštaj is composed on screen and nothing exports it. That claim is
   * about a different document and `IzvestajPanel.test.tsx` pins it separately;
   * this assertion is scoped to this paragraph so the two cannot be confused.
   */
  it("does not deny the print and the export the backend now has", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await user.click(
      await screen.findByRole("button", { name: /započni brojanje/i }),
    );

    const fazaA = await screen.findByText(/ne zamenjuje potpis na papiru/i);
    expect(fazaA).not.toHaveTextContent(/ne štampa/i);
    expect(fazaA).not.toHaveTextContent(/ne izvozi/i);

    await user.click(
      screen.getByRole("button", { name: /potpiši stvarno stanje/i }),
    );
    await user.click(
      await screen.findByRole("button", { name: /obračunaj razlike/i }),
    );
    await screen.findByRole("button", { name: /potpiši obračunate liste/i });

    const fazaB = screen.getByText(/ne zamenjuje potpis na papiru/i);
    expect(fazaB).not.toHaveTextContent(/ne štampa/i);
    expect(fazaB).not.toHaveTextContent(/ne izvozi/i);
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

describe("PopisModule — the printed popisne liste (reqs. 31/32)", () => {
  /**
   * The copy beside the button has to say which of the two documents the click
   * produces, because they are not variants of one sheet: the čl. 8 st. 5 one
   * carries the counted state and nothing the books know, and the čl. 9 st. 3
   * one carries the obračun. An operator who printed the wrong one and handed
   * it to the komisija would breach čl. 8 st. 5 with a document this program
   * generated for him.
   */
  it("offers the čl. 8 st. 5 sheet during the count and says it carries no book data", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await dovediDo(user, "counting");

    const blok = await screen.findByRole("group", {
      name: /popisne liste za štampu/i,
    });
    expect(
      within(blok).getByRole("button", { name: /štampaj popisne liste/i }),
    ).toBeInTheDocument();
    expect(
      within(blok).getByText(/bez knjigovodstvenih količina/i),
    ).toBeInTheDocument();
  });

  /**
   * The two descriptions must not be one interchangeable string. A single
   * sentence covering both phases would either name čl. 8 st. 5 over a sheet
   * carrying razlike or name čl. 9 st. 3 over one that carries none — and the
   * article is the only thing on screen that tells the operator which document
   * he is about to put in front of the komisija.
   */
  it("names čl. 9 st. 3 on the computed sheet and čl. 8 st. 5 on the counted one", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await dovediDo(user, "counting");

    const brojanje = await screen.findByText(/štampa se popisna lista/i);
    expect(brojanje).toHaveTextContent(/PoP čl\. 8 st\. 5/);
    expect(brojanje).not.toHaveTextContent(/čl\. 9 st\. 3/);

    await dovediDo(user, "computed");

    const obracun = await screen.findByText(/štampaju se obračunate/i);
    expect(obracun).toHaveTextContent(/PoP čl\. 9 st\. 3/);
    expect(obracun).not.toHaveTextContent(/čl\. 8 st\. 5/);
    // And the counting sentence is gone, not merely joined by a second one.
    expect(screen.queryByText(/štampa se popisna lista/i)).toBeNull();
  });

  /** SW-8 export-then-open: the written file is what goes to the OS handler. */
  it("exports through the backend and hands the written file to the print service", async () => {
    const user = userEvent.setup();
    const svc = services();
    const izvoz = vi.spyOn(svc.popis, "exportLista");
    const otvori = vi
      .spyOn(svc.print, "openForPrint")
      .mockResolvedValue(undefined);

    render(
      <>
        <PopisModule services={svc} />
        <Toaster />
      </>,
    );

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await dovediDo(user, "counting");
    await user.click(
      screen.getByRole("button", { name: /štampaj popisne liste/i }),
    );

    await waitFor(() => expect(izvoz).toHaveBeenCalledTimes(1));
    const exported = await izvoz.mock.results[0].value;
    await waitFor(() => expect(otvori).toHaveBeenCalledWith(exported.path));
  });

  /**
   * The load-bearing one. `popis_export_lista` derives the phase from
   * `book_quantities_released(status, fazaAPotpisana)` and refuses „b“ before
   * the čl. 8 st. 5 potpis by name — but the frontend must never be the surface
   * that asks for it. Omitted means „print what this popis has“; „a“ can only
   * ever narrow, and over-withholding breaches nothing. „b“ would be this
   * screen instructing the backend to release book quantities onto paper, and
   * no state of this module may send it.
   */
  it("never asks the backend for the čl. 9 st. 3 sheet", async () => {
    const user = userEvent.setup();
    const svc = services();
    const izvoz = vi.spyOn(svc.popis, "exportLista");
    vi.spyOn(svc.print, "openForPrint").mockResolvedValue(undefined);

    render(<PopisModule services={svc} />);
    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });

    for (const status of [
      "counting",
      "counted_signed",
      "computed",
      "computed_signed",
      "posted",
    ] as const) {
      await dovediDo(user, status);
      for (const dugme of await screen.findAllByRole("button", {
        name: /^štampaj (popisne liste|potpisane liste)/i,
      })) {
        await user.click(dugme);
      }
    }

    expect(izvoz).toHaveBeenCalled();
    for (const [, faza] of izvoz.mock.calls) {
      expect(faza).not.toBe("b");
    }
  });

  /**
   * PoP čl. 2 st. 6 gives the owner of tuđa roba ten days for a primerak of the
   * **signed** posebna popisna lista, and the signed document is the counted
   * state. `popis_export_lista` keeps an explicit Faza A printable in every
   * state for exactly that reason, so the reprint has to be reachable once the
   * obračun has opened — otherwise the module states a rok it gives the shop no
   * way to meet.
   */
  it("keeps the signed čl. 8 st. 5 sheet printable once the obračun has opened", async () => {
    const user = userEvent.setup();
    const svc = services();
    const izvoz = vi.spyOn(svc.popis, "exportLista");
    vi.spyOn(svc.print, "openForPrint").mockResolvedValue(undefined);

    render(<PopisModule services={svc} />);
    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await dovediDo(user, "computed");

    await user.click(
      await screen.findByRole("button", { name: /štampaj potpisane liste/i }),
    );

    await waitFor(() => expect(izvoz).toHaveBeenCalledWith(1, "a"));
  });

  /**
   * A failed export must be said out loud. The alternative is a button that
   * looks as though it worked and a document that was never written — the shop
   * finds out at the moment it needs the signed sheet.
   */
  it("surfaces a failed export instead of silently opening nothing", async () => {
    const user = userEvent.setup();
    const svc = services();
    vi.spyOn(svc.popis, "exportLista").mockRejectedValue(
      new Error("Disk je pun."),
    );
    const otvori = vi
      .spyOn(svc.print, "openForPrint")
      .mockResolvedValue(undefined);

    render(
      <>
        <PopisModule services={svc} />
        <Toaster />
      </>,
    );

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await dovediDo(user, "counting");
    await user.click(
      screen.getByRole("button", { name: /štampaj popisne liste/i }),
    );

    expect(await screen.findByText(/Disk je pun\./)).toBeInTheDocument();
    expect(otvori).not.toHaveBeenCalled();
  });
});

describe("PopisModule — the odluka and the plan rada (req. 35)", () => {
  /**
   * The plan's own instruction, and the class of defect this project keeps
   * catching: an approval nobody performed is a worse record than a missing
   * one. Opening a popis approves nothing, rendering the plan approves nothing,
   * and the screen says so in the statute's own terms rather than leaving the
   * field blank and silent.
   */
  it("never auto-approves the plan rada", async () => {
    const user = userEvent.setup();
    const svc = services();
    const odobri = vi.spyOn(svc.popis, "odobriPlan");
    render(<PopisModule services={svc} />);

    await otvoriPopis(user);

    const blok = await screen.findByRole("group", {
      name: /odluka o popisu i plan rada/i,
    });
    expect(within(blok).getByText(/plan rada nije odobren/i)).toHaveTextContent(
      /PoP čl\. 8 st\. 2/,
    );
    expect(odobri).not.toHaveBeenCalled();
  });

  /**
   * For a preduzetnik the lice iz čl. 4 st. 2 is the owner personally (čl. 4
   * st. 2 → ZoRač čl. 43 st. 3), so the registered obveznik is the useful
   * suggestion — and only a suggestion. The backend deliberately refuses to
   * substitute it (`popis_plan_rada_bez_odobravaoca`), because a server-side
   * default is an auto-approval wearing a default's clothes; the default
   * belongs here, where a person sees it and can correct it before submitting.
   */
  it("defaults the approver to the registered obveznik and leaves it editable", async () => {
    const user = userEvent.setup();
    const svc = services();
    const podesavanja = await svc.settings.getCompanySettings();
    render(<PopisModule services={svc} />);

    await otvoriPopis(user);

    const polje = await screen.findByLabelText(/lica koje odobrava plan rada/i);
    await waitFor(() => expect(polje).toHaveValue(podesavanja.shopName));

    await user.clear(polje);
    await user.type(polje, "Amina Hodžić");
    expect(polje).toHaveValue("Amina Hodžić");
  });

  it("records the approval only when the operator submits it", async () => {
    const user = userEvent.setup();
    const svc = services();
    const odobri = vi.spyOn(svc.popis, "odobriPlan");
    render(<PopisModule services={svc} />);

    await otvoriPopis(user);

    const polje = await screen.findByLabelText(/lica koje odobrava plan rada/i);
    await user.clear(polje);
    await user.type(polje, "Amina Hodžić");
    expect(odobri).not.toHaveBeenCalled();

    await user.click(
      screen.getByRole("button", { name: /evidentiraj odobrenje/i }),
    );

    await waitFor(() => expect(odobri).toHaveBeenCalledWith(1, "Amina Hodžić"));
    expect(
      await screen.findByText(/plan rada je odobren/i),
    ).toHaveTextContent(/Amina Hodžić/);
  });

  /** The backend's own sentence, not a second wording invented here. */
  it("surfaces the refusal of a blank approver", async () => {
    const user = userEvent.setup();
    const svc = services();
    render(<PopisModule services={svc} />);

    await otvoriPopis(user);

    const polje = await screen.findByLabelText(/lica koje odobrava plan rada/i);
    await user.clear(polje);
    await user.click(
      screen.getByRole("button", { name: /evidentiraj odobrenje/i }),
    );

    expect(
      await screen.findByText(/Odobrenje bez imena se ne evidentira/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/plan rada nije odobren/i)).toBeInTheDocument();
  });

  it("prints the odluka and the plan rada through the export-then-open path", async () => {
    const user = userEvent.setup();
    const svc = services();
    const odluka = vi.spyOn(svc.popis, "exportOdluka");
    const plan = vi.spyOn(svc.popis, "exportPlanRada");
    const otvori = vi
      .spyOn(svc.print, "openForPrint")
      .mockResolvedValue(undefined);

    render(
      <>
        <PopisModule services={svc} />
        <Toaster />
      </>,
    );

    await otvoriPopis(user);

    await user.click(
      await screen.findByRole("button", { name: /štampaj odluku o popisu/i }),
    );
    await waitFor(() => expect(odluka).toHaveBeenCalledWith(1));

    await user.click(screen.getByRole("button", { name: /štampaj plan rada/i }));
    await waitFor(() => expect(plan).toHaveBeenCalledWith(1));

    await waitFor(() => expect(otvori).toHaveBeenCalledTimes(2));
  });

  /**
   * Req. 41 / čl. 14 st. 3 — `odobri_plan_rada` runs `posting_lock` and refuses
   * a posted popis. The module offers no action the state machine would refuse,
   * so the control goes; the documents stay printable, because a reprint of a
   * posted popis changes nothing about it.
   */
  it("offers no approval control on a proknjižen popis", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user, {
      clan: { ime: "Amina Hodžić", rukujeImovinom: false },
    });
    await dovediDo(user, "posted");

    expect(
      screen.queryByRole("button", { name: /evidentiraj odobrenje/i }),
    ).toBeNull();
    expect(
      screen.queryByLabelText(/lica koje odobrava plan rada/i),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: /štampaj plan rada/i }),
    ).toBeEnabled();
  });

  /**
   * `odluka_doneta_at` exists since v21 and **no verb in this build writes it**
   * — exporting a draft odluka is not *donošenje odluke*. The generated
   * document says so in its own header cell; the screen that offers the button
   * has to say the same thing, or the operator reads a dated decision into an
   * undated one.
   */
  it("says the datum donošenja of the odluka is not recorded here", async () => {
    const user = userEvent.setup();
    render(<PopisModule services={services()} />);

    await otvoriPopis(user);

    const blok = await screen.findByRole("group", {
      name: /odluka o popisu i plan rada/i,
    });
    expect(
      within(blok).getByText(/datum donošenja odluke/i),
    ).toHaveTextContent(/ne evidentira/i);
  });
});

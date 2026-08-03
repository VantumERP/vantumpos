import { LockIcon } from "lucide-react";
import { useState } from "react";
import type { FormEvent } from "react";

import { formatQuantity, parseQuantityInput } from "@/app/format";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type {
  PopisLineInput,
  PopisLineView,
  PopisLista,
  PopisSessionView,
} from "@/services/types";

/**
 * The popisna lista — the count sheet the commission writes on.
 *
 * **The čl. 8 st. 5 blind count is decided by one field and it is not
 * `status`.** `session.knjigovodstvoDostupno` is the backend's own answer to
 * `book_quantities_released(status, fazaAPotpisana)`, and this component asks
 * nothing else: `status` is a claim any UPDATE can make, a session can be born
 * in `counted_signed` with no potpis behind it, and the status limb of that
 * predicate is module-private backend-side precisely so nothing can reach it.
 *
 * **Hiding a column is not the compliance.** Req. 29 is enforced at the query
 * layer — while the count is blind the response carries no book quantity from
 * `popis_lines`, from `inventory_balances` or from `inventory_movements`, so
 * there is nothing here to hide. What this component owes the article is the
 * other half: it must not put a book figure, an „očekivano“ column or a razlika
 * on the screen from any source of its own, and it must not offer an input for
 * one. Both are asserted by test against a payload that carries the figures
 * anyway, so a sheet that keyed off „does the row have a number“ fails.
 *
 * **Every edit affordance mirrors a write the backend actually accepts.** Draft
 * and counting take a whole stavka; `computed` takes the obračun over stavke
 * that were counted and nothing else; the two signed states and the posted one
 * take nothing. A button that always got refused would promise a write this
 * app does not have.
 */

/** What the price column is on this lista. Čl. 11 st. 1 makes it the apoen. */
function cenaNaziv(lista: PopisLista): string {
  return lista === "gotovina" ? "Apoen" : "Cena";
}

/** What the shop may do to the liste in this state, and why. */
type Rezim =
  | { kind: "otvoren" }
  | { kind: "obracun" }
  | { kind: "zatvoren"; razlog: string };

export function countSheetRezim(session: PopisSessionView): Rezim {
  switch (session.status) {
    case "draft":
    case "counting":
      return { kind: "otvoren" };
    case "computed":
      return { kind: "obracun" };
    case "counted_signed":
      return {
        kind: "zatvoren",
        razlog:
          "Stvarno stanje je potpisano (PoP čl. 8 st. 5) — popisne liste se " +
          "više ne menjaju. Obračun razlika otvara sledeći korak.",
      };
    case "computed_signed":
      return {
        kind: "zatvoren",
        razlog:
          "Obračunate popisne liste su potpisane (PoP čl. 9 st. 3) i više se " +
          "ne menjaju.",
      };
    case "posted":
      return {
        kind: "zatvoren",
        razlog:
          "Popis je proknjižen (PoP čl. 14 st. 3) i više se ne menja — " +
          "ispravka se sprovodi novim popisom (ZoRač čl. 8 st. 4).",
      };
  }
}

interface FormState {
  lineId: number | null;
  listaVrsta: PopisLista;
  sifra: string;
  naziv: string;
  vrsta: string;
  jedinicaMere: string;
  stvarnaKolicina: string;
  bliziOpis: string;
  knjigovodstvenaKolicina: string;
  cena: string;
}

const PRAZNA_FORMA: FormState = {
  lineId: null,
  listaVrsta: "roba",
  sifra: "",
  naziv: "",
  vrsta: "",
  jedinicaMere: "",
  stvarnaKolicina: "",
  bliziOpis: "",
  knjigovodstvenaKolicina: "",
  cena: "",
};

const LISTE: { vrsta: PopisLista; naziv: string }[] = [
  { vrsta: "roba", naziv: "Roba u objektu" },
  { vrsta: "ostecena", naziv: "Oštećena, zastarela i neupotrebljiva roba" },
  { vrsta: "van_objekta", naziv: "Roba van objekta" },
  { vrsta: "gotovina", naziv: "Gotovina po apoenima" },
  { vrsta: "potrazivanja", naziv: "Nedokumentovana potraživanja i obaveze" },
  { vrsta: "konsignacija", naziv: "Konsignaciona i druga tuđa roba" },
];

function listaNaziv(lista: PopisLista): string {
  return LISTE.find((candidate) => candidate.vrsta === lista)?.naziv ?? lista;
}

function formaZaLiniju(linija: PopisLineView): FormState {
  return {
    lineId: linija.id,
    listaVrsta: linija.listaVrsta,
    sifra: linija.sifra ?? "",
    naziv: linija.naziv,
    vrsta: linija.vrsta ?? "",
    jedinicaMere: linija.jedinicaMere ?? "",
    stvarnaKolicina: (linija.stvarnaKolicinaMilli / 1000).toString(),
    bliziOpis: linija.bliziOpis ?? "",
    knjigovodstvenaKolicina:
      linija.knjigovodstvenaKolicinaMilli === null
        ? ""
        : (linija.knjigovodstvenaKolicinaMilli / 1000).toString(),
    cena:
      linija.cenaMinor === null ? "" : (linija.cenaMinor / 100).toFixed(2),
  };
}

export function CountSheet({
  session,
  onSaveLine,
  busy = false,
}: {
  session: PopisSessionView;
  /**
   * Absent means read-only. Present does **not** mean editable: the state
   * decides, and in a closed state no affordance is rendered at all.
   */
  onSaveLine?: (
    lineId: number | null,
    input: PopisLineInput,
  ) => Promise<void>;
  busy?: boolean;
}) {
  const [form, setForm] = useState<FormState>(PRAZNA_FORMA);
  const [error, setError] = useState<string | undefined>();

  const rezim = countSheetRezim(session);
  const dostupno = session.knjigovodstvoDostupno;
  const editable = onSaveLine !== undefined && rezim.kind !== "zatvoren";
  const obracunSamo = rezim.kind === "obracun";
  // In the obračun only a counted stavka may be touched, so the form appears
  // when one is picked and never on its own.
  const formaVidljiva = editable && (!obracunSamo || form.lineId !== null);

  const grupe = LISTE.map((lista) => ({
    ...lista,
    linije: session.linije.filter(
      (linija) => linija.listaVrsta === lista.vrsta,
    ),
  })).filter((grupa) => grupa.linije.length > 0);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!onSaveLine) {
      return;
    }

    let stvarnaKolicinaMilli: number;
    try {
      stvarnaKolicinaMilli = parseQuantityInput(form.stvarnaKolicina);
    } catch {
      setError("Stvarna količina nije ispravna.");
      return;
    }

    let knjigovodstvenaKolicinaMilli: number | null = null;
    if (dostupno && form.knjigovodstvenaKolicina.trim() !== "") {
      try {
        knjigovodstvenaKolicinaMilli = parseQuantityInput(
          form.knjigovodstvenaKolicina,
        );
      } catch {
        setError("Knjigovodstvena količina nije ispravna.");
        return;
      }
    }

    let cenaMinor: number | null = null;
    if (form.cena.trim() !== "") {
      try {
        cenaMinor = parseRsdInput(form.cena);
      } catch {
        setError(`${cenaNaziv(form.listaVrsta)} nije ispravan iznos.`);
        return;
      }
    }

    if (form.naziv.trim() === "") {
      setError("Svaka stavka popisne liste mora da ima naziv (PoP čl. 8 st. 4).");
      return;
    }

    setError(undefined);
    try {
      await onSaveLine(form.lineId, {
        listaVrsta: form.listaVrsta,
        sifra: form.sifra.trim() === "" ? null : form.sifra.trim(),
        naziv: form.naziv.trim(),
        vrsta: form.vrsta.trim() === "" ? null : form.vrsta.trim(),
        jedinicaMere:
          form.jedinicaMere.trim() === "" ? null : form.jedinicaMere.trim(),
        stvarnaKolicinaMilli,
        bliziOpis: form.bliziOpis.trim() === "" ? null : form.bliziOpis.trim(),
        // Never invented and never defaulted to a figure: in Phase A this stays
        // null because čl. 8 st. 5 has released nothing, and a `0` here would
        // report a manjak of the whole stanje.
        knjigovodstvenaKolicinaMilli,
        cenaMinor,
      });
      setForm(obracunSamo ? form : PRAZNA_FORMA);
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : typeof cause === "object" && cause !== null && "message" in cause
            ? String((cause as { message: unknown }).message)
            : "Stavka nije sačuvana.",
      );
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {rezim.kind === "zatvoren" ? (
        <Alert>
          <LockIcon />
          <AlertTitle>Popisne liste su zaključane</AlertTitle>
          <AlertDescription>{rezim.razlog}</AlertDescription>
        </Alert>
      ) : null}

      {grupe.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Na popisnim listama još nema nijedne stavke.
        </p>
      ) : null}

      {grupe.map((grupa) => (
        <div key={grupa.vrsta} className="flex flex-col gap-2">
          <p className="text-sm font-medium">{grupa.naziv}</p>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Šifra</TableHead>
                <TableHead>Naziv</TableHead>
                <TableHead>Vrsta</TableHead>
                <TableHead>Jed. mere</TableHead>
                <TableHead className="text-right">Stvarna količina</TableHead>
                {/*
                  Req. 29. Not „render null as a dash“ — the columns do not
                  exist at all while čl. 8 st. 5 withholds the data, because a
                  column with a dash in it is a place a later change puts a
                  number back.
                */}
                {dostupno ? (
                  <TableHead className="text-right">
                    Knjigovodstvena količina
                  </TableHead>
                ) : null}
                {dostupno ? (
                  <TableHead className="text-right">Razlika</TableHead>
                ) : null}
                <TableHead>Bliži opis</TableHead>
                <TableHead className="text-right">
                  {cenaNaziv(grupa.vrsta)}
                </TableHead>
                {editable ? <TableHead /> : null}
              </TableRow>
            </TableHeader>
            <TableBody>
              {grupa.linije.map((linija) => (
                <TableRow key={linija.id}>
                  <TableCell>{linija.sifra ?? "—"}</TableCell>
                  <TableCell>{linija.naziv}</TableCell>
                  <TableCell>{linija.vrsta ?? "—"}</TableCell>
                  <TableCell>{linija.jedinicaMere ?? "—"}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatQuantity(
                      linija.stvarnaKolicinaMilli,
                      linija.jedinicaMere ?? "kom",
                    )}
                  </TableCell>
                  {dostupno ? (
                    <TableCell className="text-right tabular-nums">
                      {linija.knjigovodstvenaKolicinaMilli === null
                        ? "—"
                        : formatQuantity(
                            linija.knjigovodstvenaKolicinaMilli,
                            linija.jedinicaMere ?? "kom",
                          )}
                    </TableCell>
                  ) : null}
                  {dostupno ? (
                    <TableCell className="text-right tabular-nums">
                      {linija.razlikaMilli === null
                        ? "—"
                        : formatQuantity(
                            linija.razlikaMilli,
                            linija.jedinicaMere ?? "kom",
                          )}
                    </TableCell>
                  ) : null}
                  <TableCell>{linija.bliziOpis ?? "—"}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {linija.cenaMinor === null
                      ? "—"
                      : formatRsd(linija.cenaMinor)}
                  </TableCell>
                  {editable ? (
                    <TableCell className="text-right">
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setForm(formaZaLiniju(linija));
                          setError(undefined);
                        }}
                      >
                        Izmeni
                      </Button>
                    </TableCell>
                  ) : null}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      ))}

      {formaVidljiva ? (
        <form onSubmit={submit} className="rounded-md border p-3">
          <FieldGroup>
            <p className="text-sm font-medium">
              {form.lineId === null
                ? "Nova stavka"
                : `Izmena stavke: ${listaNaziv(form.listaVrsta)}`}
            </p>
            {error ? <FieldError>{error}</FieldError> : null}

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-lista">Popisna lista</FieldLabel>
              <NativeSelect
                id="popis-lista"
                value={form.listaVrsta}
                disabled={obracunSamo}
                onChange={(event) =>
                  setForm({
                    ...form,
                    listaVrsta: event.target.value as PopisLista,
                  })
                }
              >
                {LISTE.map((lista) => (
                  <NativeSelectOption key={lista.vrsta} value={lista.vrsta}>
                    {lista.naziv}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </Field>

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-sifra">Šifra</FieldLabel>
              <Input
                id="popis-sifra"
                value={form.sifra}
                disabled={obracunSamo}
                onChange={(event) =>
                  setForm({ ...form, sifra: event.target.value })
                }
              />
              <FieldDescription>
                Nomenklaturni broj artikla (PoP čl. 8 st. 4).
              </FieldDescription>
            </Field>

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-naziv">Naziv</FieldLabel>
              <Input
                id="popis-naziv"
                value={form.naziv}
                disabled={obracunSamo}
                onChange={(event) =>
                  setForm({ ...form, naziv: event.target.value })
                }
              />
            </Field>

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-vrsta">Vrsta</FieldLabel>
              <Input
                id="popis-vrsta"
                value={form.vrsta}
                disabled={obracunSamo}
                onChange={(event) =>
                  setForm({ ...form, vrsta: event.target.value })
                }
              />
            </Field>

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-jm">Jedinica mere</FieldLabel>
              <Input
                id="popis-jm"
                value={form.jedinicaMere}
                disabled={obracunSamo}
                onChange={(event) =>
                  setForm({ ...form, jedinicaMere: event.target.value })
                }
              />
            </Field>

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-kolicina">Stvarna količina</FieldLabel>
              <Input
                id="popis-kolicina"
                value={form.stvarnaKolicina}
                disabled={obracunSamo}
                onChange={(event) =>
                  setForm({ ...form, stvarnaKolicina: event.target.value })
                }
              />
              <FieldDescription>
                Rezultat brojanja (PoP čl. 9 st. 1 t. 1).
              </FieldDescription>
            </Field>

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-blizi-opis">Bliži opis</FieldLabel>
              <Input
                id="popis-blizi-opis"
                value={form.bliziOpis}
                disabled={obracunSamo}
                onChange={(event) =>
                  setForm({ ...form, bliziOpis: event.target.value })
                }
              />
            </Field>

            {/*
              Req. 29. The input does not exist before the potpis — an operator
              who could type a book quantity into a blind count would be typing
              into a write the backend refuses by name.
            */}
            {dostupno ? (
              <Field className="w-auto">
                <FieldLabel htmlFor="popis-knjigovodstvena">
                  Knjigovodstvena količina
                </FieldLabel>
                <Input
                  id="popis-knjigovodstvena"
                  value={form.knjigovodstvenaKolicina}
                  onChange={(event) =>
                    setForm({
                      ...form,
                      knjigovodstvenaKolicina: event.target.value,
                    })
                  }
                />
                <FieldDescription>
                  Popunjava se tek posle potpisa stvarnog stanja (PoP čl. 8
                  st. 5). Za gotovinu i nedokumentovana potraživanja unosi se
                  ručno — iza njih ne stoji lager.
                </FieldDescription>
              </Field>
            ) : null}

            <Field className="w-auto">
              <FieldLabel htmlFor="popis-cena">
                {cenaNaziv(form.listaVrsta)}
              </FieldLabel>
              <Input
                id="popis-cena"
                value={form.cena}
                disabled={obracunSamo && form.listaVrsta === "gotovina"}
                onChange={(event) =>
                  setForm({ ...form, cena: event.target.value })
                }
              />
              <FieldDescription>
                {form.listaVrsta === "gotovina"
                  ? "Apoen novčanice ili kovanice — deo prebrojanog stanja (PoP čl. 11 st. 1), a ne obračunska cena."
                  : "Obračunska cena (PoP čl. 9 st. 1 t. 5)."}
              </FieldDescription>
            </Field>

            <div className="flex gap-2">
              <Button type="submit" size="sm" disabled={busy}>
                Sačuvaj stavku
              </Button>
              {form.lineId === null ? null : (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    setForm(PRAZNA_FORMA);
                    setError(undefined);
                  }}
                >
                  Odustani
                </Button>
              )}
            </div>
          </FieldGroup>
        </form>
      ) : null}

      {editable && obracunSamo && form.lineId === null ? (
        <p className="text-sm text-muted-foreground">
          Izaberite stavku i unesite obračun. Nova stavka se ne dodaje posle
          potpisa stvarnog stanja — nju niko nije prebrojao (PoP čl. 8 st. 5).
        </p>
      ) : null}
    </div>
  );
}

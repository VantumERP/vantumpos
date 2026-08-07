import {
  AlertTriangleIcon,
  ClipboardListIcon,
  LockIcon,
  PlusIcon,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";

import { CountSheet } from "./CountSheet";
import { IzvestajPanel } from "./IzvestajPanel";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
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
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { PopisService, PosServices } from "@/services/ports";
import type {
  KomisijaClanInput,
  NivelacijaObuhvatId,
  NivelacijaObuhvatView,
  NivelacijaPregledView,
  PopisLineInput,
  PopisLista,
  PopisSessionView,
  PopisStatus,
  PopisSummary,
  PopisUloga,
  PopisVrsta,
  ProveraListiView,
} from "@/services/types";

/**
 * Popis — the two-phase blind count, its two statutory signature events and the
 * čl. 13 st. 1 izveštaj (SW-16).
 *
 * **What this module is careful never to do.**
 *
 * It never derives the čl. 8 st. 5 answer. `knjigovodstvoDostupno` comes back
 * with every session and the count sheet asks nothing else — the module does
 * not re-derive it from `status`, because `status` is a claim any UPDATE can
 * make and the status limb of that predicate is module-private backend-side for
 * exactly this reason.
 *
 * It never offers an action the state machine would refuse. One arrow per
 * state, none on a posted popis, and the way back in is stated (a new popis,
 * ZoRač čl. 8 st. 4) rather than left for the operator to hunt for.
 *
 * It never presents the nivelacija scope narrowing as a duty. Neither ZoRač
 * čl. 21 nor PoP čl. 3 scopes the count, and the narrowing is borrowed from a
 * regime this shop cannot use — so the two strings, the duty and the scope, are
 * kept apart and the scope block carries the backend's own `pravniStatus`.
 *
 * It never lets a click read as a signature. PoP čl. 9 st. 3 says *„uz
 * štampanje“*; what is recorded here is that the members signed the printed
 * liste, and a purely electronic signature is an unverified deviation (§6 R-6).
 */

const ULOGE: { uloga: PopisUloga; naziv: string }[] = [
  { uloga: "predsednik", naziv: "Predsednik komisije" },
  { uloga: "clan", naziv: "Član komisije" },
  { uloga: "jedno_lice", naziv: "Jedno lice (PoP čl. 6 st. 1)" },
];

const VRSTE: { vrsta: PopisVrsta; naziv: string }[] = [
  { vrsta: "godisnji", naziv: "Godišnji popis (ZoRač čl. 20 st. 2)" },
  { vrsta: "nivelacioni", naziv: "Popis po nivelaciji (ZoRač čl. 21)" },
];

/** The one statutory step a popis in this state may take, and what it is called. */
interface Korak {
  label: string;
  /** The provision the step discharges — printed beside the button. */
  pravniOsnov: string;
  /** Whether the step records a signing on paper (req. 31). */
  potpis: boolean;
  run: (popis: PopisService, id: number, potpisnici: string[]) => Promise<PopisSessionView>;
}

export function sledeciKorak(status: PopisStatus): Korak | null {
  switch (status) {
    case "draft":
      return {
        label: "Započni brojanje",
        pravniOsnov: "PoP čl. 9 st. 1 t. 1",
        potpis: false,
        run: (popis, id) => popis.startCount(id),
      };
    case "counting":
      return {
        label: "Potpiši stvarno stanje",
        pravniOsnov: "PoP čl. 8 st. 5",
        potpis: true,
        run: (popis, id, potpisnici) => popis.signPhaseA(id, potpisnici),
      };
    case "counted_signed":
      return {
        label: "Obračunaj razlike",
        pravniOsnov: "PoP čl. 9 st. 1 t. 3–6",
        potpis: false,
        run: (popis, id) => popis.compute(id),
      };
    case "computed":
      return {
        label: "Potpiši obračunate liste",
        pravniOsnov: "PoP čl. 9 st. 3",
        potpis: true,
        run: (popis, id, potpisnici) => popis.signPhaseB(id, potpisnici),
      };
    case "computed_signed":
      return {
        label: "Proknjiži popis",
        pravniOsnov: "PoP čl. 14 st. 3",
        potpis: false,
        run: (popis, id) => popis.post(id),
      };
    case "posted":
      return null;
  }
}

const STATUS_NAZIV: Record<PopisStatus, string> = {
  draft: "priprema popisa",
  counting: "brojanje",
  counted_signed: "stvarno stanje potpisano",
  computed: "obračun razlika",
  computed_signed: "obračunate liste potpisane",
  posted: "proknjižen popis",
};

function poruka(cause: unknown): string {
  if (cause instanceof Error) {
    return cause.message;
  }
  if (typeof cause === "object" && cause !== null && "message" in cause) {
    return String((cause as { message: unknown }).message);
  }
  return "Radnja nije izvršena.";
}

export function PopisModule({ services }: { services: PosServices }) {
  const popis = services.popis;
  const [summaries, setSummaries] = useState<PopisSummary[]>([]);
  const [selected, setSelected] = useState<PopisSessionView | null>(null);
  const [pregled, setPregled] = useState<NivelacijaPregledView | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);

  const refreshList = useCallback(async () => {
    setSummaries(await popis.list());
  }, [popis]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [lista, nivelacija] = await Promise.all([
          popis.list(),
          popis.nivelacijaPregled(),
        ]);
        if (!cancelled) {
          setSummaries(lista);
          setPregled(nivelacija);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(poruka(cause));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [popis]);

  async function otvori(request: Parameters<PopisService["open"]>[0]) {
    setBusy(true);
    setError(undefined);
    try {
      const session = await popis.open(request);
      setSelected(session);
      setFormOpen(false);
      await refreshList();
    } catch (cause) {
      setError(poruka(cause));
    } finally {
      setBusy(false);
    }
  }

  async function korak(potpisnici: string[]) {
    if (!selected) {
      return;
    }
    const next = sledeciKorak(selected.status);
    if (!next) {
      return;
    }

    setBusy(true);
    setError(undefined);
    try {
      setSelected(await next.run(popis, selected.id, potpisnici));
      await refreshList();
    } catch (cause) {
      setError(poruka(cause));
    } finally {
      setBusy(false);
    }
  }

  async function sacuvajStavku(
    lineId: number | null,
    input: PopisLineInput,
  ) {
    if (!selected) {
      return;
    }
    setSelected(await popis.saveLine(selected.id, lineId, input));
    await refreshList();
  }

  return (
    <div className="flex flex-col gap-6">
      {error ? (
        <Alert variant="destructive">
          <AlertTriangleIcon />
          <AlertTitle>Popis</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {pregled ? <NivelacijaPanel pregled={pregled} /> : null}

      <Separator />

      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm font-medium">Popisi</p>
          <Button
            type="button"
            size="sm"
            variant={formOpen ? "ghost" : "default"}
            onClick={() => setFormOpen((open) => !open)}
          >
            <PlusIcon />
            {formOpen ? "Zatvori formu" : "Novi popis"}
          </Button>
        </div>

        {formOpen ? <OpenPopisForm busy={busy} onSubmit={otvori} /> : null}

        {summaries.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <ClipboardListIcon />
              </EmptyMedia>
              <EmptyTitle>Nema evidentiranih popisa</EmptyTitle>
              <EmptyDescription>
                Popis se sprovodi na dan bilansa (ZoRač čl. 20 st. 2) i pri
                svakoj promeni prodajnih cena u maloprodajnom objektu (ZoRač
                čl. 21).
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Datum popisa</TableHead>
                <TableHead>Vrsta</TableHead>
                <TableHead>Prodajno mesto</TableHead>
                <TableHead>Stanje</TableHead>
                <TableHead className="text-right">Stavki</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {summaries.map((summary) => (
                <TableRow key={summary.id}>
                  <TableCell>{summary.datumPopisa}</TableCell>
                  <TableCell>
                    {VRSTE.find((v) => v.vrsta === summary.vrsta)?.naziv ??
                      summary.vrsta}
                  </TableCell>
                  <TableCell>{summary.prodajnoMesto}</TableCell>
                  <TableCell>
                    <Badge variant="outline">
                      {STATUS_NAZIV[summary.status]}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right tabular-nums">
                    {summary.brojLinija}
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      onClick={() => {
                        setError(undefined);
                        void popis
                          .get(summary.id)
                          .then(setSelected)
                          .catch((cause: unknown) => setError(poruka(cause)));
                      }}
                    >
                      Otvori
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>

      {selected ? (
        <>
          <Separator />
          {/*
            Keyed on the popis: the req. 36 declaration is an answer about ONE
            popis, and carrying it across when the shop opens another would
            declare categories nobody was asked about.
          */}
          <PopisDetail
            key={selected.id}
            services={services}
            session={selected}
            busy={busy}
            onKorak={korak}
            onSaveLine={sacuvajStavku}
          />
        </>
      ) : null}
    </div>
  );
}

/**
 * Req. 33 — the standing ZoRač čl. 21 report.
 *
 * The duty and the scope are two blocks on purpose. A paragraph carrying both
 * would read as „the law says count only these“, which is the one thing design
 * §7 t. 8 forbids; the scope block therefore carries only the backend's own
 * `pravniStatus` and `obrazlozenje`, and no duty word appears in it anywhere.
 */
function NivelacijaPanel({ pregled }: { pregled: NivelacijaPregledView }) {
  const { obavestenje } = pregled;

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm font-medium">Popis po nivelaciji (ZoRač čl. 21)</p>
      <p className="text-sm">{obavestenje.obaveza}</p>
      <p className="text-xs text-muted-foreground">
        {obavestenje.pravniOsnov}
      </p>
      <p className="text-sm">{obavestenje.rokObjasnjenje}</p>

      <div
        role="group"
        aria-label="Obim popisa po nivelaciji"
        className="flex flex-col gap-1 rounded-md border p-3"
      >
        <p className="text-sm font-medium">Obim popisa</p>
        {obavestenje.obuhvat.map((opcija) => (
          <p key={opcija.obuhvat} className="text-sm">
            <span className="font-medium">{opcija.naziv}</span>
            {opcija.podrazumevani ? " (predlog)" : ""} —{" "}
            <span>{opcija.pravniStatus}</span>.{" "}
            <span className="text-muted-foreground">
              {opcija.obrazlozenje}
            </span>
          </p>
        ))}
      </div>

      <p className="text-xs text-muted-foreground">{obavestenje.napomena}</p>
      <p className="text-xs text-muted-foreground">{pregled.izvor}</p>

      {pregled.obaveze.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Nema evidentiranih promena prodajnih cena bez popisa.
        </p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Datum promene cena</TableHead>
              <TableHead className="text-right">Artikala</TableHead>
              <TableHead>Popis</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {pregled.obaveze.map((obaveza) => (
              <TableRow key={obaveza.datum}>
                <TableCell>{obaveza.datum}</TableCell>
                <TableCell className="text-right tabular-nums">
                  {obaveza.brojArtikala}
                </TableCell>
                <TableCell>
                  {obaveza.popisUToku
                    ? `u toku — ${STATUS_NAZIV[obaveza.popisUToku.status]}`
                    : "nije otvoren"}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}

function OpenPopisForm({
  busy,
  onSubmit,
}: {
  busy: boolean;
  onSubmit: (request: Parameters<PopisService["open"]>[0]) => Promise<void>;
}) {
  const [vrsta, setVrsta] = useState<PopisVrsta>("godisnji");
  const [prodajnoMesto, setProdajnoMesto] = useState("");
  const [datumPopisa, setDatumPopisa] = useState("");
  const [periodFrom, setPeriodFrom] = useState("");
  const [periodTo, setPeriodTo] = useState("");
  const [odlukaRef, setOdlukaRef] = useState("");
  const [perpetualOdlukaRef, setPerpetualOdlukaRef] = useState("");
  const [planRada, setPlanRada] = useState("");
  const [uskladjivanje, setUskladjivanje] = useState(false);
  const [komisija, setKomisija] = useState<KomisijaClanInput[]>([]);
  const [ime, setIme] = useState("");
  const [uloga, setUloga] = useState<PopisUloga>("predsednik");
  const [rukujeImovinom, setRukujeImovinom] = useState(false);
  const [formError, setFormError] = useState<string | undefined>();

  function dodajClana() {
    if (ime.trim() === "") {
      setFormError("Član komisije se evidentira po imenu (PoP čl. 5 st. 1).");
      return;
    }
    setFormError(undefined);
    setKomisija([
      ...komisija,
      { ime: ime.trim(), uloga, rukujeImovinom },
    ]);
    setIme("");
    setRukujeImovinom(false);
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    // The čl. 20 st. 3 confirmation is NOT pre-checked here. The gate is the
    // backend's — it is the one that records the confirmation — so the refusal
    // the shop reads is the statute's own sentence and not a second wording.
    await onSubmit({
      vrsta,
      prodajnoMesto: prodajnoMesto.trim(),
      datumPopisa: datumPopisa.trim(),
      periodFrom: periodFrom.trim() === "" ? null : periodFrom.trim(),
      periodTo: periodTo.trim() === "" ? null : periodTo.trim(),
      planRadaJson: planRada.trim() === "" ? null : planRada.trim(),
      odlukaRef: odlukaRef.trim() === "" ? null : odlukaRef.trim(),
      perpetualOdlukaRef:
        perpetualOdlukaRef.trim() === "" ? null : perpetualOdlukaRef.trim(),
      uskladjivanjePotvrdjeno: uskladjivanje,
      komisija,
    });
  }

  return (
    <form onSubmit={submit} className="rounded-md border p-3">
      <FieldGroup>
        {formError ? <FieldError>{formError}</FieldError> : null}

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-vrsta-sesije">Vrsta popisa</FieldLabel>
          <NativeSelect
            id="popis-vrsta-sesije"
            value={vrsta}
            onChange={(event) =>
              setVrsta(event.target.value as PopisVrsta)
            }
          >
            {VRSTE.map((option) => (
              <NativeSelectOption key={option.vrsta} value={option.vrsta}>
                {option.naziv}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-mesto">Prodajno mesto</FieldLabel>
          <Input
            id="popis-mesto"
            value={prodajnoMesto}
            onChange={(event) => setProdajnoMesto(event.target.value)}
          />
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-datum">Datum popisa</FieldLabel>
          <Input
            id="popis-datum"
            type="date"
            value={datumPopisa}
            onChange={(event) => setDatumPopisa(event.target.value)}
          />
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-period-od">Period od</FieldLabel>
          <Input
            id="popis-period-od"
            type="date"
            value={periodFrom}
            onChange={(event) => setPeriodFrom(event.target.value)}
          />
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-period-do">Period do</FieldLabel>
          <Input
            id="popis-period-do"
            type="date"
            value={periodTo}
            onChange={(event) => setPeriodTo(event.target.value)}
          />
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-odluka">
            Odluka o popisu i obrazovanju komisije
          </FieldLabel>
          <Input
            id="popis-odluka"
            value={odlukaRef}
            onChange={(event) => setOdlukaRef(event.target.value)}
          />
          <FieldDescription>
            Broj ili oznaka odluke koju donosi preduzetnik lično (PoP čl. 4
            st. 2).
          </FieldDescription>
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-plan-rada">Plan rada komisije</FieldLabel>
          <Input
            id="popis-plan-rada"
            value={planRada}
            onChange={(event) => setPlanRada(event.target.value)}
          />
          <FieldDescription>PoP čl. 8 st. 1–2.</FieldDescription>
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-perpetual">
            Odluka o već izvršenom popisu u toku godine
          </FieldLabel>
          <Input
            id="popis-perpetual"
            value={perpetualOdlukaRef}
            onChange={(event) => setPerpetualOdlukaRef(event.target.value)}
          />
          <FieldDescription>
            PoP čl. 9 st. 2 — izuzetak od naturalnog popisa. Aplikacija
            proverava da u istoj godini postoji raniji, proknjižen popis; da
            li je izveštaj o tom popisu usvojen (PoP čl. 14 st. 2) ne
            proverava, jer tu odluku aplikacija ne evidentira.
          </FieldDescription>
        </Field>

        <Separator />

        <p className="text-sm font-medium">Komisija za popis</p>
        <p className="text-xs text-muted-foreground">
          PoP čl. 5 st. 1 — popisivač ne može da bude lice koje rukuje imovinom
          koja se popisuje. Za jedno lice iz čl. 6 st. 1 shodna primena tog
          pravila nije razjašnjena, pa aplikacija upozorava i ne zaustavlja
          popis.
        </p>

        {komisija.length > 0 ? (
          <ul className="flex flex-col gap-1 text-sm">
            {komisija.map((clan, index) => (
              <li key={`${clan.ime}-${index}`}>
                {clan.ime} —{" "}
                {ULOGE.find((option) => option.uloga === clan.uloga)?.naziv}
                {clan.rukujeImovinom ? " · rukuje imovinom" : ""}
              </li>
            ))}
          </ul>
        ) : null}

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-clan-ime">Ime člana komisije</FieldLabel>
          <Input
            id="popis-clan-ime"
            value={ime}
            onChange={(event) => setIme(event.target.value)}
          />
        </Field>

        <Field className="w-auto">
          <FieldLabel htmlFor="popis-clan-uloga">Uloga</FieldLabel>
          <NativeSelect
            id="popis-clan-uloga"
            value={uloga}
            onChange={(event) =>
              setUloga(event.target.value as PopisUloga)
            }
          >
            {ULOGE.map((option) => (
              <NativeSelectOption key={option.uloga} value={option.uloga}>
                {option.naziv}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>

        <label
          className="flex items-center gap-2 text-sm"
          htmlFor="popis-clan-rukuje"
        >
          <Checkbox
            id="popis-clan-rukuje"
            checked={rukujeImovinom}
            onCheckedChange={(checked) => setRukujeImovinom(checked === true)}
          />
          Rukuje imovinom koja se popisuje
        </label>

        <div>
          <Button type="button" size="sm" variant="outline" onClick={dodajClana}>
            Dodaj člana
          </Button>
        </div>

        <Separator />

        <label
          className="flex items-start gap-2 text-sm"
          htmlFor="popis-uskladjivanje"
        >
          <Checkbox
            id="popis-uskladjivanje"
            checked={uskladjivanje}
            onCheckedChange={(checked) => setUskladjivanje(checked === true)}
          />
          <span>
            Potvrđujem da je pre popisa izvršeno usklađivanje glavne knjige sa
            dnevnikom i pomoćnih knjiga sa glavnom knjigom.
            <span className="block text-xs text-muted-foreground">
              ZoRač čl. 20 st. 3 — redosled je propisan zakonom, a ne
              preporučen.
            </span>
          </span>
        </label>

        <div>
          <Button type="submit" size="sm" disabled={busy}>
            Otvori popis
          </Button>
        </div>
      </FieldGroup>
    </form>
  );
}

function PopisDetail({
  services,
  session,
  busy,
  onKorak,
  onSaveLine,
}: {
  services: PosServices;
  session: PopisSessionView;
  busy: boolean;
  onKorak: (potpisnici: string[]) => Promise<void>;
  onSaveLine: (lineId: number | null, input: PopisLineInput) => Promise<void>;
}) {
  const korak = sledeciKorak(session.status);
  const [potpisnici, setPotpisnici] = useState("");
  // Req. 36. The declaration is a request parameter and not a stored fact —
  // nothing in the schema records it — so it lives with the popis on screen and
  // travels with the izveštaj request, empty or not.
  const [prijavljene, setPrijavljene] = useState<PopisLista[]>([]);
  const [provera, setProvera] = useState<ProveraListiView | null>(null);

  const popis = services.popis;

  useEffect(() => {
    let cancelled = false;
    popis
      .proveraListi(session.id, prijavljene)
      .then((rezultat) => {
        if (!cancelled) {
          setProvera(rezultat);
        }
      })
      .catch(() => {
        // A readiness report that failed to load is not itself a refusal: the
        // one that stops the izveštaj is the generator's, by name.
        if (!cancelled) {
          setProvera(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [popis, session, prijavljene]);

  const imena =
    potpisnici.trim() === ""
      ? session.komisija.map((clan) => clan.ime)
      : potpisnici
          .split(",")
          .map((ime) => ime.trim())
          .filter((ime) => ime !== "");

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-sm font-medium">
          Popis {session.datumPopisa} · {session.prodajnoMesto}
        </p>
        <Badge variant="outline">{STATUS_NAZIV[session.status]}</Badge>
        {session.postedAt ? <Badge variant="secondary">proknjižen</Badge> : null}
      </div>

      {session.upozorenja.map((upozorenje) => (
        <Alert key={upozorenje}>
          <AlertTriangleIcon />
          <AlertDescription>{upozorenje}</AlertDescription>
        </Alert>
      ))}

      {session.konsignacijaRok ? (
        <p className="text-sm">
          Rok za dostavu potpisane konsignacione liste vlasniku:{" "}
          <span className="font-medium">{session.konsignacijaRok}</span>.
        </p>
      ) : null}

      {session.komisija.length > 0 ? (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Komisija</TableHead>
              <TableHead>Uloga</TableHead>
              <TableHead>Rukuje imovinom</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {session.komisija.map((clan) => (
              <TableRow key={clan.id}>
                <TableCell>{clan.ime}</TableCell>
                <TableCell>
                  {ULOGE.find((option) => option.uloga === clan.uloga)?.naziv ??
                    clan.uloga}
                </TableCell>
                <TableCell>{clan.rukujeImovinom ? "da" : "ne"}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : null}

      {session.potpisi.length > 0 ? (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Potpis</TableHead>
              <TableHead>Faza</TableHead>
              <TableHead>Vreme</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {session.potpisi.map((potpis) => (
              <TableRow key={potpis.id}>
                <TableCell>{potpis.potpisnik}</TableCell>
                <TableCell>
                  {potpis.faza === "a"
                    ? "stvarno stanje (čl. 8 st. 5)"
                    : "obračunate liste (čl. 9 st. 3)"}
                </TableCell>
                <TableCell>{potpis.potpisanoAt}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : null}

      {korak ? (
        <div className="flex flex-col gap-2 rounded-md border p-3">
          <div className="flex flex-wrap items-baseline gap-2">
            <p className="text-sm font-medium">Sledeći korak</p>
            <span className="text-xs text-muted-foreground">
              {korak.pravniOsnov}
            </span>
          </div>

          {korak.potpis ? (
            <>
              {/*
                Req. 31. PoP čl. 9 st. 3 authorises the computer for the obračun
                „uz štampanje“ — the potpis is on paper. This records that it
                happened; it is not itself a signature, and §6 R-6 leaves the
                purely electronic route unverified.

                The article is `korak.pravniOsnov`, never a literal. There are
                two potpisi and they discharge different provisions: čl. 8 st. 5
                on the counted liste, čl. 9 st. 3 on the obračunate ones. Printing
                čl. 9 st. 3 under both — which this paragraph used to do — showed
                the operator two articles for one act, and cited the „uz
                štampanje“ rule for a potpis it does not govern.

                The paragraph makes no claim about what the program can output.
                It used to end „program ih ne štampa i ne izvozi“, which was
                honest while it was true and stopped being true when
                `popis_export_lista` shipped — the backend renders both popisne
                liste and writes them to `exports/`. Announcing that export here
                would be the same defect facing the other way: this panel has no
                such button yet, and a sentence promising one sends the operator
                hunting. So the sentence states the duty, which is the shop's
                either way, and stops there. The izveštaj keeps its own „program
                ga ne štampa i ne izvozi“ — that is a different document and it
                is still true.
              */}
              <p className="text-sm">
                Odštampajte popisne liste i neka ih članovi komisije potpišu.
                Ovde se evidentira da je taj potpis dat: evidentiranje ne
                zamenjuje potpis na papiru {`(${korak.pravniOsnov}).`}
              </p>
              <Field className="w-auto">
                <FieldLabel htmlFor="popis-potpisnici">
                  Potpisnici (imena, razdvojena zarezom)
                </FieldLabel>
                <Input
                  id="popis-potpisnici"
                  value={potpisnici}
                  placeholder={session.komisija
                    .map((clan) => clan.ime)
                    .join(", ")}
                  onChange={(event) => setPotpisnici(event.target.value)}
                />
              </Field>
            </>
          ) : null}

          <div>
            <Button
              type="button"
              size="sm"
              disabled={busy}
              onClick={() => {
                void onKorak(imena);
              }}
            >
              {korak.label}
            </Button>
          </div>
        </div>
      ) : (
        <Alert>
          <LockIcon />
          <AlertTitle>Popis je proknjižen</AlertTitle>
          <AlertDescription>
            Rezultat je proknjižen (PoP čl. 14 st. 3) i popis se više ne menja —
            ispravka se sprovodi novim popisom (ZoRač čl. 8 st. 4).
          </AlertDescription>
        </Alert>
      )}

      {session.vrsta === "nivelacioni" ? (
        <NivelacijaObuhvatPanel services={services} session={session} />
      ) : null}

      <CountSheet
        session={session}
        onSaveLine={onSaveLine}
        busy={busy}
        prijavljene={prijavljene}
        onPrijava={(lista) =>
          setPrijavljene((current) =>
            current.includes(lista)
              ? current.filter((kandidat) => kandidat !== lista)
              : [...current, lista],
          )
        }
        provera={provera}
      />

      <Separator />

      <IzvestajPanel
        services={services}
        session={session}
        prijavljene={prijavljene}
      />
    </div>
  );
}

/**
 * Req. 33 / PoP čl. 8 st. 4 — the article list the commission is handed before
 * the count, under the scope the shop picked.
 *
 * **The scope is a control here and a duty nowhere.** The two options and the
 * reasoning behind them are stated once, by `NivelacijaPanel`, off the
 * backend's own strings; this block only picks between them, so the copy req.
 * 33 is about is not repeated in a second, softer wording. The narrowed one is
 * the default because the backend defaults to it, and the answer says which
 * scope it applied.
 *
 * **Four fields and no količina among them (req. 29).** The list is read before
 * anything is counted, so a perpetual stanje beside each article would hand the
 * book quantities over at the very start of the count — the leak no later
 * potpis can unring. Nothing is written either: `stvarna_kolicina_milli` is NOT
 * NULL, so a seeded stavka would carry a count of zero indistinguishable from a
 * counted zero.
 */
function NivelacijaObuhvatPanel({
  services,
  session,
}: {
  services: PosServices;
  session: PopisSessionView;
}) {
  const [obuhvat, setObuhvat] = useState<NivelacijaObuhvatId | null>(null);
  const [pregled, setPregled] = useState<NivelacijaObuhvatView | null>(null);
  const [greska, setGreska] = useState<string | undefined>();

  const popis = services.popis;

  useEffect(() => {
    let cancelled = false;
    popis
      .nivelacijaObuhvat(session.id, obuhvat)
      .then((odgovor) => {
        if (!cancelled) {
          setPregled(odgovor);
          setGreska(undefined);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setPregled(null);
          setGreska(poruka(cause));
        }
      });
    return () => {
      cancelled = true;
    };
    // On `session` and not on `session.id`: `vecNaListama` counts the artikli
    // already written down, so it goes stale the moment a stavka is saved — and
    // a count this block reports wrongly is worse than one it does not report.
  }, [popis, session, obuhvat]);

  return (
    <div
      role="group"
      aria-label="Artikli za popis po nivelaciji"
      className="flex flex-col gap-2 rounded-md border p-3"
    >
      <p className="text-sm font-medium">
        Artikli za popis po nivelaciji (PoP čl. 8 st. 4)
      </p>
      <p className="text-xs text-muted-foreground">
        Nomenklaturni brojevi, nazivi, vrsta i jedinice mere koji se predaju
        komisiji pre brojanja. Količine ovde ne stoje — brojanje ih tek utvrđuje
        (PoP čl. 8 st. 5). Aplikacija ne upisuje ove artikle na popisne liste:
        stavku upisuje komisija kad je prebroji.
      </p>

      {greska ? (
        <p className="text-sm text-muted-foreground">{greska}</p>
      ) : null}

      {pregled === null ? null : (
        <Field className="w-auto">
          <FieldLabel htmlFor="popis-nivelacija-obuhvat">
            Obim za ovu listu
          </FieldLabel>
          {/*
            The default is applied by the backend and reported back in
            `pregled.obuhvat` — the select shows what was applied rather than a
            default this screen keeps its own copy of.
          */}
          <NativeSelect
            id="popis-nivelacija-obuhvat"
            value={pregled.obuhvat}
            onChange={(event) =>
              setObuhvat(event.target.value as NivelacijaObuhvatId)
            }
          >
            {pregled.obavestenje.obuhvat.map((opcija) => (
              <NativeSelectOption key={opcija.obuhvat} value={opcija.obuhvat}>
                {opcija.naziv}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <FieldDescription>
            Obrazloženje oba obima stoji uz obavezu po nivelaciji, iznad.
          </FieldDescription>
        </Field>
      )}

      {pregled === null ? null : pregled.artikli.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          U ovom obimu nema nijednog artikla.
        </p>
      ) : (
        <>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Nomenklaturni broj</TableHead>
                <TableHead>Naziv</TableHead>
                <TableHead>Vrsta</TableHead>
                <TableHead>Jedinica mere</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {pregled.artikli.map((artikal) => (
                <TableRow key={`${artikal.sifra ?? ""}-${artikal.naziv}`}>
                  <TableCell>{artikal.sifra ?? "—"}</TableCell>
                  <TableCell>{artikal.naziv}</TableCell>
                  <TableCell>{artikal.vrsta ?? "—"}</TableCell>
                  <TableCell>{artikal.jedinicaMere ?? "—"}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <p className="text-xs text-muted-foreground">
            Već na popisnim listama: {pregled.vecNaListama}.
          </p>
        </>
      )}
    </div>
  );
}

import { AlertCircleIcon, FileTextIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { formatRsd } from "@/lib/money";
import type { PosServices } from "@/services/ports";
import type {
  IzvestajNarativ,
  IzvestajView,
  IzvestajZbir,
  PopisLista,
  PopisSessionView,
} from "@/services/types";

/**
 * Izveštaj o popisu — the PoP čl. 13 st. 1 template (req. 37) with its computed
 * rok and the čl. 14 st. 2 odluka beside it (req. 38).
 *
 * **Three things this panel is careful not to be.**
 *
 * It is not a *second* rule engine. The completeness of the eight elements, the
 * req. 36 lista declaration and the čl. 8 st. 5 gate are all decided backend
 * side, and what a refusal says is what the operator reads — one wording, not
 * two. The only thing decided here is whether to offer the button at all, and
 * that is decided by `knjigovodstvoDostupno`, the backend's own answer.
 *
 * It is not a *store*. Nothing in the schema records an izveštaj: the document
 * is composed when it is asked for and it survives only on paper. The backend's
 * own `upozorenja` say so and this panel prints them rather than paraphrasing.
 *
 * It does not *carry a deadline of its own*. `rok` is computed by
 * `crate::popis::izvestaj_due` from the configured rok za predaju FI — the
 * leap-year case is exactly the one a hardcoded table gets wrong — so the panel
 * renders the string it is handed, and offers the setting the computation needs
 * instead of a fallback date.
 */

/**
 * The five elements the commission writes, mapped onto the wire fields of
 * `IzvestajNarativ`. The label is the article's own term for the element; the
 * authoritative naziv, pravni osnov and uputstvo ride back **with the composed
 * document**, where they are rendered from the backend's own strings.
 */
const NARATIVNI_ELEMENTI: {
  polje: keyof IzvestajNarativ;
  id: string;
  label: string;
}[] = [
  {
    polje: "uzrociNeslaganja",
    id: "izvestaj-uzroci",
    label: "Uzroci neslaganja stvarnog i knjigovodstvenog stanja",
  },
  {
    polje: "predloziZaLikvidacijuRazlika",
    id: "izvestaj-predlozi",
    label: "Predlozi za likvidaciju utvrđenih razlika",
  },
  {
    polje: "nacinKnjizenja",
    id: "izvestaj-knjizenje",
    label: "Način knjiženja razlika",
  },
  {
    polje: "primedbeLicaKojaRukujuVrednostima",
    id: "izvestaj-primedbe",
    label: "Primedbe i objašnjenja lica koja rukuju vrednostima",
  },
  {
    polje: "ostalePrimedbeIPredlozi",
    id: "izvestaj-ostalo",
    label: "Ostale primedbe i predlozi",
  },
];

/** Req. 36 — the categories the shop declares present, checked against the liste. */
const PRIJAVLJIVE_LISTE: { vrsta: PopisLista; naziv: string }[] = [
  { vrsta: "roba", naziv: "Roba u objektu" },
  { vrsta: "ostecena", naziv: "Oštećena, zastarela i neupotrebljiva roba" },
  { vrsta: "van_objekta", naziv: "Roba van objekta" },
  { vrsta: "gotovina", naziv: "Gotovina po apoenima" },
  { vrsta: "potrazivanja", naziv: "Nedokumentovana potraživanja i obaveze" },
  { vrsta: "konsignacija", naziv: "Konsignaciona i druga tuđa roba" },
];

const PRAZAN_NARATIV: IzvestajNarativ = {
  uzrociNeslaganja: "",
  predloziZaLikvidacijuRazlika: "",
  nacinKnjizenja: "",
  primedbeLicaKojaRukujuVrednostima: "",
  ostalePrimedbeIPredlozi: "",
};

function poruka(cause: unknown): string {
  if (cause instanceof Error) {
    return cause.message;
  }
  if (typeof cause === "object" && cause !== null && "message" in cause) {
    return String((cause as { message: unknown }).message);
  }
  return "Izveštaj o popisu nije sastavljen.";
}

export function IzvestajPanel({
  services,
  session,
}: {
  services: PosServices;
  session: PopisSessionView;
}) {
  const [narativ, setNarativ] = useState<IzvestajNarativ>(PRAZAN_NARATIV);
  const [prijavljene, setPrijavljene] = useState<PopisLista[]>([]);
  const [izvestaj, setIzvestaj] = useState<IzvestajView | null>(null);
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  const [rokPredajeFi, setRokPredajeFi] = useState("");
  const [rokPoruka, setRokPoruka] = useState<string | undefined>();

  const popis = services.popis;
  const dostupno = session.knjigovodstvoDostupno;

  useEffect(() => {
    let cancelled = false;
    popis
      .getPodesavanja()
      .then((podesavanja) => {
        if (!cancelled) {
          setRokPredajeFi(podesavanja.rokPredajeFi ?? "");
        }
      })
      .catch(() => {
        // A settings read that failed is not an error the operator has to
        // clear before writing an izveštaj — the refusal that matters comes
        // from the composition itself, by name.
      });
    return () => {
      cancelled = true;
    };
  }, [popis]);

  const toggleLista = useCallback((lista: PopisLista) => {
    setPrijavljene((current) =>
      current.includes(lista)
        ? current.filter((candidate) => candidate !== lista)
        : [...current, lista],
    );
  }, []);

  async function sastavi(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      // `prijavljeneListe` always travels, empty or not: the req. 36 gate
      // refuses a DECLARED lista that is empty, so an absent field would
      // satisfy it vacuously and compose with no check behind it.
      const composed = await popis.izvestaj(session.id, {
        prijavljeneListe: [...prijavljene],
        narativ: { ...narativ },
      });
      setIzvestaj(composed);
    } catch (cause) {
      setIzvestaj(null);
      setError(poruka(cause));
    } finally {
      setBusy(false);
    }
  }

  async function sacuvajRok() {
    setRokPoruka(undefined);
    try {
      const saved = await popis.setPodesavanja({
        rokPredajeFi: rokPredajeFi.trim() === "" ? null : rokPredajeFi.trim(),
      });
      setRokPredajeFi(saved.rokPredajeFi ?? "");
      setRokPoruka("Rok je sačuvan.");
    } catch (cause) {
      setRokPoruka(poruka(cause));
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-2 rounded-md border p-3">
        <p className="text-sm font-medium">
          Rok za predaju redovnog godišnjeg finansijskog izveštaja
        </p>
        <p className="text-xs text-muted-foreground">
          Rok za izveštaj o godišnjem popisu računa se od ovog datuma (PoP
          čl. 13 st. 2). ZoRač čl. 44 st. 1 propisuje 31. mart, osim ako je
          posebnim zakonom drukčije uređeno — zato je ovo podešavanje, a ne
          konstanta u aplikaciji.
        </p>
        <Field className="w-auto">
          <FieldLabel htmlFor="popis-rok-fi">
            Rok za predaju finansijskog izveštaja
          </FieldLabel>
          <Input
            id="popis-rok-fi"
            type="date"
            value={rokPredajeFi}
            onChange={(event) => setRokPredajeFi(event.target.value)}
          />
        </Field>
        <div className="flex items-center gap-3">
          <Button type="button" size="sm" variant="outline" onClick={sacuvajRok}>
            Sačuvaj rok
          </Button>
          {rokPoruka ? (
            <span className="text-xs text-muted-foreground">{rokPoruka}</span>
          ) : null}
        </div>
      </div>

      {!dostupno ? (
        <Alert>
          <AlertCircleIcon />
          <AlertTitle>Izveštaj se sastavlja posle potpisa</AlertTitle>
          <AlertDescription>
            Izveštaj o popisu iskazuje knjigovodstveno stanje i razlike, pa se
            ne sastavlja pre nego što članovi komisije potpišu stvarno stanje
            (PoP čl. 8 st. 5).
          </AlertDescription>
        </Alert>
      ) : (
        <form onSubmit={sastavi} className="rounded-md border p-3">
          <FieldGroup>
            <p className="text-sm font-medium">
              Izveštaj o popisu (PoP čl. 13 st. 1)
            </p>
            {error ? <FieldError>{error}</FieldError> : null}

            <div className="flex flex-col gap-2">
              <p className="text-sm font-medium">
                Koje kategorije postoje u ovom popisu?
              </p>
              <p className="text-xs text-muted-foreground">
                Posebna lista se traži tamo gde kategorija postoji (PoP čl. 2
                st. 5, čl. 10–12). Šta postoji zna radnja, ne knjige — zato se
                prijavljuje ovde, a prijavljena kategorija bez ijedne stavke
                zaustavlja izveštaj.
              </p>
              {PRIJAVLJIVE_LISTE.map((lista) => (
                <label
                  key={lista.vrsta}
                  className="flex items-center gap-2 text-sm"
                  htmlFor={`prijava-${lista.vrsta}`}
                >
                  <Checkbox
                    id={`prijava-${lista.vrsta}`}
                    checked={prijavljene.includes(lista.vrsta)}
                    onCheckedChange={() => toggleLista(lista.vrsta)}
                  />
                  {lista.naziv}
                </label>
              ))}
            </div>

            <Separator />

            {NARATIVNI_ELEMENTI.map((element) => (
              <Field key={element.polje} className="w-auto">
                <FieldLabel htmlFor={element.id}>{element.label}</FieldLabel>
                <Textarea
                  id={element.id}
                  rows={2}
                  value={narativ[element.polje]}
                  onChange={(event) =>
                    setNarativ({
                      ...narativ,
                      [element.polje]: event.target.value,
                    })
                  }
                />
              </Field>
            ))}

            <FieldDescription>
              Preostala tri elementa (stvarno stanje, knjigovodstveno stanje i
              razlike) čitaju se iz popisnih listi i ne prekucavaju se.
            </FieldDescription>

            <div>
              <Button type="submit" size="sm" disabled={busy}>
                Sastavi izveštaj
              </Button>
            </div>
          </FieldGroup>
        </form>
      )}

      {izvestaj ? <IzvestajDokument izvestaj={izvestaj} /> : null}
    </div>
  );
}

function IzvestajDokument({ izvestaj }: { izvestaj: IzvestajView }) {
  return (
    <div className="flex flex-col gap-4 rounded-md border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <FileTextIcon className="size-4" />
        <p className="text-sm font-medium">Izveštaj o popisu</p>
        <Badge variant="outline">{izvestaj.prodajnoMesto}</Badge>
        <Badge variant="outline">{izvestaj.datumPopisa}</Badge>
      </div>

      <p className="text-xs text-muted-foreground">
        {izvestaj.obveznik} · PIB {izvestaj.pib} · matični broj{" "}
        {izvestaj.maticniBroj}
      </p>

      {izvestaj.upozorenja.map((upozorenje) => (
        <Alert key={upozorenje}>
          <AlertCircleIcon />
          <AlertDescription>{upozorenje}</AlertDescription>
        </Alert>
      ))}

      <div className="flex flex-col gap-3">
        {izvestaj.elementi.map((element) => (
          <div key={element.element} className="flex flex-col gap-1">
            <div className="flex flex-wrap items-baseline gap-2">
              <p className="text-sm font-medium">{element.naziv}</p>
              <span className="text-xs text-muted-foreground">
                {element.pravniOsnov}
              </span>
            </div>
            <p className="text-xs text-muted-foreground">{element.uputstvo}</p>
            {element.tekst === null ? null : (
              <p className="text-sm">{element.tekst}</p>
            )}
          </div>
        ))}
      </div>

      <Separator />

      <div className="flex flex-col gap-2">
        <p className="text-sm font-medium">Vrednosni pregled po listama</p>
        {/*
          Value and counts of stavki, and no summed količina anywhere: stavke on
          one lista can be in komadima, metrima and kilogramima at once, so a
          natural total would be a number with no unit. The naturalne razlike
          stay per stavka, on the popisne liste.
        */}
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Popisna lista</TableHead>
              <TableHead className="text-right">Stavki</TableHead>
              <TableHead className="text-right">Viškova</TableHead>
              <TableHead className="text-right">Manjkova</TableHead>
              <TableHead className="text-right">Po popisu</TableHead>
              <TableHead className="text-right">Po knjigama</TableHead>
              <TableHead className="text-right">Vrednosna razlika</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {izvestaj.liste.map((lista) => (
              <TableRow key={lista.vrsta}>
                <TableCell>
                  {lista.naziv}
                  <span className="block text-xs text-muted-foreground">
                    {lista.pravniOsnov}
                  </span>
                </TableCell>
                <ZbirCells zbir={lista.zbir} />
              </TableRow>
            ))}
            <TableRow>
              <TableCell className="font-medium">Ukupno</TableCell>
              <ZbirCells zbir={izvestaj.ukupno} />
            </TableRow>
          </TableBody>
        </Table>
        <p className="text-xs text-muted-foreground">
          Uz izveštaj se prilažu popisne liste — naturalne količine i naturalne
          razlike stoje po stavkama na njima (PoP čl. 9 st. 1 t. 4).
        </p>
        {izvestaj.ukupno.potpuno ? null : (
          <p className="text-xs text-muted-foreground">
            {izvestaj.ukupno.stavkeBezCene} stavki bez cene i{" "}
            {izvestaj.ukupno.stavkeBezKnjigovodstvenogStanja} bez
            knjigovodstvenog stanja nisu obuhvaćene iznosima iznad.
          </p>
        )}
      </div>

      <Separator />

      <div className="flex flex-col gap-1">
        <div className="flex flex-wrap items-baseline gap-2">
          <p className="text-sm font-medium">Rok za izveštaj</p>
          <span className="text-sm">{izvestaj.rok}</span>
          <span className="text-xs text-muted-foreground">
            {izvestaj.rokPravniOsnov}
          </span>
        </div>
        <div className="flex flex-wrap items-baseline gap-2">
          <p className="text-sm font-medium">Odluka o usvajanju izveštaja</p>
          <span className="text-sm">{izvestaj.odlukaOUsvajanju.rok}</span>
          <span className="text-xs text-muted-foreground">
            {izvestaj.odlukaOUsvajanju.pravniOsnov}
          </span>
        </div>
        <p className="text-xs text-muted-foreground">
          Donosilac: {izvestaj.odlukaOUsvajanju.donosilac}.{" "}
          {izvestaj.odlukaOUsvajanju.napomena}
        </p>
      </div>
    </div>
  );
}

function ZbirCells({ zbir }: { zbir: IzvestajZbir }) {
  return (
    <>
      <TableCell className="text-right tabular-nums">
        {zbir.brojStavki}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {zbir.stavkeSaViskom}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {zbir.stavkeSaManjkom}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {formatRsd(zbir.vrednostPoPopisuMinor)}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {formatRsd(zbir.vrednostPoKnjigamaMinor)}
      </TableCell>
      <TableCell className="text-right tabular-nums">
        {formatRsd(zbir.vrednosnaRazlikaMinor)}
      </TableCell>
    </>
  );
}

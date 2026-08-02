import { AlertCircleIcon, AlertTriangleIcon, FileDownIcon, PlusIcon } from "lucide-react";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";

import { errorMessage, formatInstant } from "./shared";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import type { PosServices } from "@/services/ports";
import type {
  Breach,
  BreachDraft,
  Cl53Izuzetak,
  LegalNotice,
  NotifyDecision,
  RiskOutcome,
} from "@/services/types";

/**
 * The SW-17 internal record of every personal-data breach (ZZPL čl. 52 st. 6).
 *
 * **Log first, decide notifiability second** (req. 43). Čl. 52 st. 6 documents
 * *„svaku povredu“*, with no risk qualifier anywhere in it; the risk test sits
 * one stav up, in st. 1, and governs only whether the **Poverenik** must be
 * told. So this form asks for the three st. 6 elements and the saznanje anchor
 * and nothing else, no control on it waits on a risk answer, and the assessment
 * is a field the operator fills in later on a record that already exists. A
 * wizard that asked „is this notifiable?“ first and discarded the No answers
 * would invert the statute and produce the Poverenik's own zero-score finding.
 *
 * **The anchor is immutable.** Čl. 52 st. 1 runs the 72 h from *saznanje*, so a
 * movable anchor would make the st. 2 delay justification optional in hindsight.
 * The backend refuses a change and this panel never offers one.
 *
 * **The obrazac is a document, not a filing.** Pravilnik 40/2019 čl. 5 is the
 * whole route — in writing, in person or by post, with a scanned copy to the
 * Poverenik's mailbox as the alternative — so the panel produces a form to
 * print and sign, and never says anything that reads as „the app notified the
 * Poverenik“.
 */
const PODNOSENJE =
  "Obrazac se podnosi u pisanom obliku, neposredno ili putem pošte, a skenirani primerak " +
  "može da se pošalje na adresu elektronske pošte Poverenika. Program ne podnosi obaveštenje " +
  "umesto rukovaoca — Pravilnik 40/2019, čl. 5.";

const RIZIK_LABELE: Record<RiskOutcome, string> = {
  bez_rizika: "Ne može da proizvede rizik po prava i slobode fizičkih lica",
  rizik: "Može da proizvede rizik po prava i slobode fizičkih lica",
  visok_rizik: "Može da proizvede visok rizik po prava i slobode fizičkih lica",
};

const ODLUKA_LABELE: Record<NotifyDecision, string> = {
  obavestiti: "Poverenik se obaveštava",
  ne_obavestiti: "Poverenik se ne obaveštava",
};

/**
 * The three čl. 53 st. 3 exceptions, closed — and the pin-cite beside each one
 * is what a reader checks the paraphrase against. Kept word-for-word with
 * `Cl53Izuzetak::label` in `commands/breaches.rs`, which is what the printed
 * obrazac carries: the screen and the filed document must not describe the same
 * exception in two ways.
 */
const IZUZETAK_LABELE: Record<Cl53Izuzetak, string> = {
  primenjene_mere_zastite:
    "primenjene su mere zaštite zbog kojih su podaci nerazumljivi neovlašćenim licima " +
    "(čl. 53 st. 3 tač. 1)",
  naknadne_mere:
    "naknadno su preduzete mere kojima je otklonjen visok rizik (čl. 53 st. 3 tač. 2)",
  nesrazmeran_utrosak_vremena_i_sredstava:
    "obaveštavanje svakog lica zahtevalo bi nesrazmeran utrošak vremena i sredstava, " +
    "pa je dato javno obaveštenje (čl. 53 st. 3 tač. 3)",
};

/**
 * The `datetime-local` control gives „gggg-MM-ddTčč:mm“ read off the operator's
 * OWN clock; the backend wants an RFC3339 instant.
 *
 * The zone marker must therefore never be appended to that string. „09:00“
 * typed in Beograd is 07:00Z in August, and stamping it „09:00Z“ would move the
 * anchor two hours later than the moment the operator meant — always in the
 * direction of granting the shop more time than čl. 52 st. 1 allows, because
 * both `rok_obavestavanja_istice_at` and the st. 2 delay test run off it. The
 * card and the printed obrazac would then also read back two hours later than
 * what was typed.
 *
 * ES parses an offset-less date-time form as **local** time, which is exactly
 * what the control means, so the conversion goes through `Date` and comes back
 * out in UTC. The milliseconds are trimmed so the stored instant has the same
 * second precision as every stamp `clock::utc_now` writes. A value the control
 * could not produce yields „“ and is refused by the caller, never a silent
 * `Invalid Date`. The mirror-image trap — `toISOString()` on what must stay a
 * local calendar day — is documented at `worktime/WorkTimeModule.tsx`.
 */
function toRfc3339(local: string): string {
  const trimmed = local.trim();
  if (!trimmed) {
    return "";
  }

  const parsed = new Date(trimmed);
  if (Number.isNaN(parsed.getTime())) {
    return "";
  }

  return parsed.toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** The inverse: a stored instant as the `datetime-local` control reads it. */
function toLocalInput(instant: string | null): string {
  if (!instant) {
    return "";
  }

  const parsed = new Date(instant);
  if (Number.isNaN(parsed.getTime())) {
    return "";
  }

  const pad = (value: number) => String(value).padStart(2, "0");

  return (
    `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())}` +
    `T${pad(parsed.getHours())}:${pad(parsed.getMinutes())}`
  );
}

function emptyDraft(): BreachDraft {
  return {
    saznanjeAt: "",
    occurredAt: null,
    discoveredAt: null,
    obradjivacSaznanjeAt: null,
    rukovalacObavestenAt: null,
    opis: "",
    posledice: "",
    mere: "",
    brojLica: null,
    kategorijePodataka: null,
    riskOutcome: null,
    notifyDecision: null,
    notifyObrazlozenje: null,
    poverenikNotifiedAt: null,
    delayReason: null,
    licaObavestena: null,
    licaObavestenaAt: null,
    cl53Izuzetak: null,
    cl53IzuzetakObrazlozenje: null,
  };
}

export function BreachLogPanel({ services }: { services: PosServices }) {
  const [breaches, setBreaches] = useState<Breach[]>([]);
  const [notice, setNotice] = useState<LegalNotice | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [saznanje, setSaznanje] = useState("");
  const [opis, setOpis] = useState("");
  const [posledice, setPosledice] = useState("");
  const [mere, setMere] = useState("");
  const [brojLica, setBrojLica] = useState("");

  useEffect(() => {
    let ignore = false;

    services.privacy
      .listBreaches()
      .then((loaded) => {
        if (!ignore) {
          setBreaches(loaded);
          setError(null);
        }
      })
      .catch((readError) => {
        if (!ignore) {
          setError(errorMessage(readError, "Evidencija povreda nije učitana."));
        }
      })
      .finally(() => {
        if (!ignore) {
          setLoading(false);
        }
      });

    services.privacy
      .breachNotice()
      .then((loaded) => {
        if (!ignore) {
          setNotice(loaded);
        }
      })
      .catch(() => {});

    return () => {
      ignore = true;
    };
  }, [services]);

  async function record(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    const anchor = toRfc3339(saznanje);
    if (!anchor) {
      setError(
        "Unesite kada je rukovalac saznao za povredu — od tog trenutka teče rok od 72 časa.",
      );
      return;
    }

    const parsedBroj = brojLica.trim() ? Number.parseInt(brojLica, 10) : null;
    if (parsedBroj !== null && (!Number.isInteger(parsedBroj) || parsedBroj < 0)) {
      setError("Broj lica na koja se podaci odnose unesite kao ceo broj.");
      return;
    }

    setSubmitting(true);
    try {
      const recorded = await services.privacy.recordBreach({
        ...emptyDraft(),
        saznanjeAt: anchor,
        opis: opis.trim(),
        posledice: posledice.trim(),
        mere: mere.trim(),
        brojLica: parsedBroj,
      });
      setBreaches((current) => [recorded, ...current]);
      setSaznanje("");
      setOpis("");
      setPosledice("");
      setMere("");
      setBrojLica("");
    } catch (recordError) {
      setError(errorMessage(recordError, "Povreda nije evidentirana."));
    } finally {
      setSubmitting(false);
    }
  }

  async function exportObrazac(id: number) {
    setError(null);
    try {
      const file = await services.privacy.exportBreachObrazac(id);
      await services.print.openForPrint(file.path);
    } catch (exportError) {
      setError(errorMessage(exportError, "Obrazac nije napravljen."));
    }
  }

  return (
    <section className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Evidencija povreda podataka o ličnosti</CardTitle>
          <CardDescription>
            Rukovalac dokumentuje svaku povredu podataka o ličnosti — ZZPL čl. 52
            st. 6. Zapis se vodi bez obzira na to da li se Poverenik obaveštava:
            procena rizika odlučuje samo o obaveštavanju, nikada o tome da li
            zapis postoji.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {notice ? (
            <Alert>
              <AlertTitle>Obaveza obaveštavanja Poverenika</AlertTitle>
              <AlertDescription className="flex flex-col gap-2">
                <span>{notice.summary}</span>
                {notice.penalty ? <span>{notice.penalty}</span> : null}
                <span>{notice.citation}</span>
              </AlertDescription>
            </Alert>
          ) : null}

          {error ? (
            <Alert variant="destructive">
              <AlertCircleIcon aria-hidden="true" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          <form className="flex flex-col gap-4" onSubmit={(event) => void record(event)}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="breach-saznanje">
                  Saznanje za povredu
                </FieldLabel>
                <Input
                  id="breach-saznanje"
                  type="datetime-local"
                  value={saznanje}
                  disabled={submitting}
                  onChange={(event) => setSaznanje(event.target.value)}
                />
                <FieldDescription>
                  Od ovog trenutka teče rok od 72 časa (ZZPL čl. 52 st. 1;
                  Pravilnik 40/2019 čl. 3). Posle upisa se više ne menja.
                </FieldDescription>
              </Field>
              <Field>
                <FieldLabel htmlFor="breach-opis">Činjenice o povredi</FieldLabel>
                <Textarea
                  id="breach-opis"
                  value={opis}
                  disabled={submitting}
                  onChange={(event) => setOpis(event.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="breach-posledice">Posledice povrede</FieldLabel>
                <Textarea
                  id="breach-posledice"
                  value={posledice}
                  disabled={submitting}
                  onChange={(event) => setPosledice(event.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="breach-mere">
                  Preduzete mere za otklanjanje
                </FieldLabel>
                <Textarea
                  id="breach-mere"
                  value={mere}
                  disabled={submitting}
                  onChange={(event) => setMere(event.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="breach-broj-lica">
                  Broj lica na koja se podaci odnose
                </FieldLabel>
                <Input
                  id="breach-broj-lica"
                  inputMode="numeric"
                  value={brojLica}
                  disabled={submitting}
                  onChange={(event) => setBrojLica(event.target.value)}
                />
                <FieldDescription>
                  Propisani obrazac traži broj lica. Spisak imena se ne vodi.
                </FieldDescription>
              </Field>
              <Field>
                <Button type="submit" disabled={submitting}>
                  {submitting ? (
                    <Spinner data-icon="inline-start" aria-hidden="true" />
                  ) : (
                    <PlusIcon data-icon="inline-start" />
                  )}
                  Evidentiraj povredu
                </Button>
              </Field>
            </FieldGroup>
          </form>

          <Separator />

          {loading ? (
            <Badge variant="outline">
              <Spinner data-icon="inline-start" aria-hidden="true" />
              Učitavanje evidencije
            </Badge>
          ) : breaches.length === 0 ? (
            <Alert>
              <AlertDescription>
                Nema evidentiranih povreda podataka o ličnosti.
              </AlertDescription>
            </Alert>
          ) : (
            <ul className="flex flex-col gap-3">
              {breaches.map((breach) => (
                <li key={breach.id}>
                  <Card size="sm">
                    <CardHeader>
                      <CardTitle>
                        Saznanje: {formatInstant(breach.saznanjeAt)}
                      </CardTitle>
                      <CardDescription>
                        Rok za obaveštavanje ističe{" "}
                        {formatInstant(breach.rokObavestavanjaIsticeAt)}.
                      </CardDescription>
                    </CardHeader>
                    <CardContent className="flex flex-col gap-3">
                      <p className="text-sm">{breach.opis}</p>
                      {breach.delayReasonRequired ? (
                        <Alert variant="destructive">
                          <AlertTriangleIcon aria-hidden="true" />
                          <AlertDescription>
                            Rok od 72 časa je istekao, a Poverenik nije obavešten
                            — razlozi moraju da budu obrazloženi (ZZPL čl. 52
                            st. 2).
                          </AlertDescription>
                        </Alert>
                      ) : null}
                      <div className="flex flex-wrap gap-2">
                        {breach.riskOutcome ? (
                          <Badge variant="outline">
                            {RIZIK_LABELE[breach.riskOutcome]}
                          </Badge>
                        ) : (
                          <Badge variant="outline">Procena rizika nije upisana</Badge>
                        )}
                        {breach.notifyDecision ? (
                          <Badge variant="secondary">
                            {ODLUKA_LABELE[breach.notifyDecision]}
                          </Badge>
                        ) : null}
                        {breach.obavestavanjeLicaObavezno ? (
                          <Badge variant="destructive">
                            Lica se obaveštavaju (ZZPL čl. 53 st. 1)
                          </Badge>
                        ) : null}
                      </div>
                      <div className="flex flex-wrap gap-2">
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() =>
                            setOpenId((current) =>
                              current === breach.id ? null : breach.id,
                            )
                          }
                        >
                          Otvori zapis
                        </Button>
                        <Button
                          type="button"
                          size="sm"
                          onClick={() => void exportObrazac(breach.id)}
                        >
                          <FileDownIcon data-icon="inline-start" />
                          Obrazac za Poverenika
                        </Button>
                      </div>
                      {openId === breach.id ? (
                        <BreachRecord
                          breach={breach}
                          onSave={async (draft) => {
                            const saved = await services.privacy.updateBreach(
                              breach.id,
                              draft,
                            );
                            setBreaches((current) =>
                              current.map((item) =>
                                item.id === saved.id ? saved : item,
                              ),
                            );
                          }}
                        />
                      ) : null}
                    </CardContent>
                  </Card>
                </li>
              ))}
            </ul>
          )}

          <FieldDescription>{PODNOSENJE}</FieldDescription>
        </CardContent>
      </Card>
    </section>
  );
}

/**
 * The open record: the two obrazac slots the opening form does not ask for, the
 * assessment, and the two notification blocks — filled in as the investigation
 * proceeds. The saznanje anchor is stated, never offered for edit — čl. 52
 * st. 1 runs the clock from it.
 *
 * **Every field the backend validates is reachable here.** A column that only
 * `validate` knows about is worse than a missing feature: `poverenik_notified_at`
 * is the one input that discharges the čl. 52 st. 2 test, so with no control for
 * it a shop that DID notify inside the 72 h has every later save refused until
 * it types a delay reason that never existed — a false statement on the very
 * document st. 7 makes the vehicle for proving compliance. The čl. 53 block is
 * the same story one article on: the panel already tells the operator the duty
 * is live, so it has to let the answer be recorded.
 */
function BreachRecord({
  breach,
  onSave,
}: {
  breach: Breach;
  onSave: (draft: BreachDraft) => Promise<void>;
}) {
  const [occurredAt, setOccurredAt] = useState(toLocalInput(breach.occurredAt));
  const [discoveredAt, setDiscoveredAt] = useState(
    toLocalInput(breach.discoveredAt),
  );
  const [kategorije, setKategorije] = useState(breach.kategorijePodataka ?? "");
  const [risk, setRisk] = useState<string>(breach.riskOutcome ?? "");
  const [decision, setDecision] = useState<string>(breach.notifyDecision ?? "");
  const [obrazlozenje, setObrazlozenje] = useState(
    breach.notifyObrazlozenje ?? "",
  );
  const [notifiedAt, setNotifiedAt] = useState(
    toLocalInput(breach.poverenikNotifiedAt),
  );
  const [delayReason, setDelayReason] = useState(breach.delayReason ?? "");
  const [licaObavestena, setLicaObavestena] = useState(
    breach.licaObavestena === null ? "" : breach.licaObavestena ? "da" : "ne",
  );
  const [licaObavestenaAt, setLicaObavestenaAt] = useState(
    toLocalInput(breach.licaObavestenaAt),
  );
  const [izuzetak, setIzuzetak] = useState<string>(breach.cl53Izuzetak ?? "");
  const [izuzetakObrazlozenje, setIzuzetakObrazlozenje] = useState(
    breach.cl53IzuzetakObrazlozenje ?? "",
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // Čl. 53 st. 1 attaches at visok rizik. The block opens on the live selection
  // as well as on the backend's flag, so the assessment and the answer to it can
  // be recorded in one pass; it also stays open on a record that already holds
  // an answer, so nothing already written becomes unreachable.
  const cl53Otvoren =
    risk === "visok_rizik" ||
    breach.obavestavanjeLicaObavezno ||
    breach.licaObavestena !== null;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setSaving(true);
    try {
      await onSave({
        ...breach,
        occurredAt: toRfc3339(occurredAt) || null,
        discoveredAt: toRfc3339(discoveredAt) || null,
        kategorijePodataka: kategorije.trim() || null,
        riskOutcome: (risk || null) as RiskOutcome | null,
        notifyDecision: (decision || null) as NotifyDecision | null,
        notifyObrazlozenje: obrazlozenje.trim() || null,
        poverenikNotifiedAt: toRfc3339(notifiedAt) || null,
        delayReason: delayReason.trim() || null,
        licaObavestena: licaObavestena === "" ? null : licaObavestena === "da",
        licaObavestenaAt: toRfc3339(licaObavestenaAt) || null,
        cl53Izuzetak: (izuzetak || null) as Cl53Izuzetak | null,
        cl53IzuzetakObrazlozenje: izuzetakObrazlozenje.trim() || null,
      });
    } catch (saveError) {
      setError(errorMessage(saveError, "Izmena nije sačuvana."));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form className="flex flex-col gap-4" onSubmit={(event) => void save(event)}>
      <FieldGroup>
        <FieldDescription>
          Vreme saznanja je nepromenljivo — od njega teče rok od 72 časa (ZZPL
          čl. 52 st. 1). Ako je uneto pogrešno, evidentira se nova povreda.
        </FieldDescription>
        {error ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}
        <Field>
          <FieldLabel htmlFor={`breach-vrsta-${breach.id}`}>
            Vrsta podataka o ličnosti
          </FieldLabel>
          <Textarea
            id={`breach-vrsta-${breach.id}`}
            value={kategorije}
            onChange={(event) => setKategorije(event.target.value)}
          />
          <FieldDescription>
            Propisani obrazac traži vrstu podataka — odeljak 2 (2), Pravilnik
            40/2019. Upisuju se vrste, nikada sami podaci.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor={`breach-povreda-${breach.id}`}>
            Datum i vreme povrede
          </FieldLabel>
          <Input
            id={`breach-povreda-${breach.id}`}
            type="datetime-local"
            value={occurredAt}
            onChange={(event) => setOccurredAt(event.target.value)}
          />
          <FieldDescription>
            Odeljak 2 (5) propisanog obrasca — ukoliko je poznat, ili prema
            proceni. Rok teče od saznanja, ne odavde.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor={`breach-otkrivanje-${breach.id}`}>
            Datum otkrivanja povrede
          </FieldLabel>
          <Input
            id={`breach-otkrivanje-${breach.id}`}
            type="datetime-local"
            value={discoveredAt}
            onChange={(event) => setDiscoveredAt(event.target.value)}
          />
        </Field>
        <Field>
          <FieldLabel htmlFor={`breach-risk-${breach.id}`}>
            Procena rizika
          </FieldLabel>
          <NativeSelect
            id={`breach-risk-${breach.id}`}
            value={risk}
            className="w-full"
            onChange={(event) => setRisk(event.target.value)}
          >
            <NativeSelectOption value="">Nije procenjeno</NativeSelectOption>
            {(Object.keys(RIZIK_LABELE) as RiskOutcome[]).map((outcome) => (
              <NativeSelectOption key={outcome} value={outcome}>
                {RIZIK_LABELE[outcome]}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
        <Field>
          <FieldLabel htmlFor={`breach-decision-${breach.id}`}>
            Odluka o obaveštavanju Poverenika
          </FieldLabel>
          <NativeSelect
            id={`breach-decision-${breach.id}`}
            value={decision}
            className="w-full"
            onChange={(event) => setDecision(event.target.value)}
          >
            <NativeSelectOption value="">Nije odlučeno</NativeSelectOption>
            {(Object.keys(ODLUKA_LABELE) as NotifyDecision[]).map((value) => (
              <NativeSelectOption key={value} value={value}>
                {ODLUKA_LABELE[value]}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
        <Field>
          <FieldLabel htmlFor={`breach-obrazlozenje-${breach.id}`}>
            Obrazloženje odluke
          </FieldLabel>
          <Textarea
            id={`breach-obrazlozenje-${breach.id}`}
            value={obrazlozenje}
            onChange={(event) => setObrazlozenje(event.target.value)}
          />
          <FieldDescription>
            Dokumentacija mora da omogući Povereniku da utvrdi da li je
            postupljeno u skladu sa članom 52 (ZZPL čl. 52 st. 7), pa uz svaku
            odluku stoji i razlog za nju.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor={`breach-notified-${breach.id}`}>
            Obaveštenje dostavljeno Povereniku
          </FieldLabel>
          <Input
            id={`breach-notified-${breach.id}`}
            type="datetime-local"
            value={notifiedAt}
            onChange={(event) => setNotifiedAt(event.target.value)}
          />
          <FieldDescription>
            Upisuje se kada je obrazac stvarno dostavljen. Ako je dostavljen u
            roku od 72 časa, obrazloženje kašnjenja iz ZZPL čl. 52 st. 2 se ne
            traži. Program ne dostavlja obaveštenje umesto rukovaoca.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor={`breach-delay-${breach.id}`}>
            Razlozi za kašnjenje preko 72 časa
          </FieldLabel>
          <Textarea
            id={`breach-delay-${breach.id}`}
            value={delayReason}
            onChange={(event) => setDelayReason(event.target.value)}
          />
        </Field>
        {cl53Otvoren ? (
          <>
            <FieldDescription>
              Povreda koja može da proizvede visok rizik po prava i slobode
              fizičkih lica saopštava se i licima na koja se podaci odnose —
              ZZPL čl. 53 st. 1. Ako nisu obaveštena, mora da se navede na koji
              se od tri izuzetka iz st. 3 rukovalac poziva.
            </FieldDescription>
            <Field>
              <FieldLabel htmlFor={`breach-lica-${breach.id}`}>
                Lica na koja se podaci odnose obaveštena
              </FieldLabel>
              <NativeSelect
                id={`breach-lica-${breach.id}`}
                value={licaObavestena}
                className="w-full"
                onChange={(event) => setLicaObavestena(event.target.value)}
              >
                <NativeSelectOption value="">Nije upisano</NativeSelectOption>
                <NativeSelectOption value="da">Da</NativeSelectOption>
                <NativeSelectOption value="ne">Ne</NativeSelectOption>
              </NativeSelect>
            </Field>
            {licaObavestena === "da" ? (
              <Field>
                <FieldLabel htmlFor={`breach-lica-at-${breach.id}`}>
                  Kada su lica obaveštena
                </FieldLabel>
                <Input
                  id={`breach-lica-at-${breach.id}`}
                  type="datetime-local"
                  value={licaObavestenaAt}
                  onChange={(event) => setLicaObavestenaAt(event.target.value)}
                />
              </Field>
            ) : null}
            {licaObavestena === "ne" ? (
              <>
                <Field>
                  <FieldLabel htmlFor={`breach-izuzetak-${breach.id}`}>
                    Izuzetak od obaveštavanja lica
                  </FieldLabel>
                  <NativeSelect
                    id={`breach-izuzetak-${breach.id}`}
                    value={izuzetak}
                    className="w-full"
                    onChange={(event) => setIzuzetak(event.target.value)}
                  >
                    <NativeSelectOption value="">
                      Nije izabran izuzetak
                    </NativeSelectOption>
                    {(Object.keys(IZUZETAK_LABELE) as Cl53Izuzetak[]).map(
                      (value) => (
                        <NativeSelectOption key={value} value={value}>
                          {IZUZETAK_LABELE[value]}
                        </NativeSelectOption>
                      ),
                    )}
                  </NativeSelect>
                </Field>
                <Field>
                  <FieldLabel htmlFor={`breach-izuzetak-obrazlozenje-${breach.id}`}>
                    Obrazloženje izuzetka
                  </FieldLabel>
                  <Textarea
                    id={`breach-izuzetak-obrazlozenje-${breach.id}`}
                    value={izuzetakObrazlozenje}
                    onChange={(event) =>
                      setIzuzetakObrazlozenje(event.target.value)
                    }
                  />
                </Field>
              </>
            ) : null}
          </>
        ) : null}
        <Field>
          <Button type="submit" size="sm" disabled={saving}>
            {saving ? <Spinner data-icon="inline-start" aria-hidden="true" /> : null}
            Sačuvaj zapis
          </Button>
        </Field>
      </FieldGroup>
    </form>
  );
}

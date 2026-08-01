import { AlertCircleIcon, ClockIcon, DownloadIcon, LockIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { PosServices } from "@/services/ports";
import type {
  AbsenceCategory,
  CapOverrideReason,
  CorrectionReason,
  ExportedFile,
  LegalNotice,
  SaveWorkTimeEntryRequest,
  UserAccount,
  WorkTimeEntryView,
  WorkTimeMonth,
  WorkTimeNotices,
  WorkTimeProtectionBlock,
} from "@/services/types";

interface WorkTimeModuleProps {
  services: PosServices;
  /**
   * The signed-in operator, used for one thing only: the §4 req. 25 access gate
   * over the absence category, which is a special-category field under ZZPL
   * čl. 17 and readable by the owner/payroll role alone.
   *
   * Optional, and **absent means closed**. A privacy gate that opens when a
   * caller forgets to wire it is not a gate; every other role — and every
   * unknown one — sees „odsutan" plus an hour total and nothing more.
   */
  currentUser?: UserAccount;
}

/** How many prior years the period selector offers alongside the current one. */
const YEAR_RANGE = 3;

const MESECI = [
  "januar",
  "februar",
  "mart",
  "april",
  "maj",
  "jun",
  "jul",
  "avgust",
  "septembar",
  "oktobar",
  "novembar",
  "decembar",
] as const;

/**
 * The closed absence vocabulary, with the letter of the statutory hour bucket
 * each one books into. Labels only — the bucket itself is DERIVED from the
 * category name backend-side, so this list can never misroute an hour.
 */
const KATEGORIJE_ODSUSTVA: { id: AbsenceCategory; label: string }[] = [
  { id: "godisnji_odmor", label: "Godišnji odmor" },
  { id: "praznik_odmor", label: "Odmor za dan državnog praznika" },
  { id: "odsustvo_uz_naknadu", label: "Odsustvo uz naknadu zarade" },
  {
    id: "strucno_osposobljavanje",
    label: "Stručno osposobljavanje i usavršavanje",
  },
  {
    id: "sprecenost_poslodavac",
    label: "Privremena sprečenost — sredstva poslodavca",
  },
  { id: "sprecenost_rfzo", label: "Privremena sprečenost — sredstva RFZO" },
  {
    id: "porodiljsko",
    label: "Porodiljsko i skraćeno radno vreme roditelja",
  },
  { id: "neplaceno_odsustvo", label: "Neplaćeno odsustvo" },
  {
    id: "naknada_drugi_poslodavci",
    label: "Naknada na teret drugih poslodavaca",
  },
  { id: "obustava_rada_strajk", label: "Obustava rada zbog štrajka" },
];

/**
 * The ZoR čl. 53 st. 1 grounds on which overtime may be ordered.
 *
 * Recording one does **not** make a čl. 53 st. 2/st. 3 breach lawful — it
 * records why the employer says the day happened, so the register can describe
 * a day that actually occurred instead of refusing it.
 */
const RAZLOZI_PREKORACENJA: { id: CapOverrideReason; label: string }[] = [
  { id: "visa_sila", label: "Viša sila" },
  {
    id: "iznenadno_povecanje_obima_posla",
    label: "Iznenadno povećanje obima posla",
  },
  {
    id: "neplanirani_posao_u_roku",
    label: "Neplanirani posao koji se mora završiti u roku",
  },
  { id: "drugo", label: "Drugo" },
];

const RAZLOZI_ISPRAVKE: { id: CorrectionReason; label: string }[] = [
  { id: "greska_u_unosu", label: "Greška u unosu" },
  { id: "ispravka_sati", label: "Ispravka sati" },
  { id: "ispravka_kategorije", label: "Ispravka kategorije" },
  {
    id: "naknadno_dostavljen_dokument",
    label: "Naknadno dostavljen dokument",
  },
  { id: "drugo", label: "Drugo" },
];

/** The form's minute fields, in entry order. Integer minutes, never hours. */
const MINUTE_FIELDS = [
  { id: "moguciMinuta", label: "Mogući časovi (min)" },
  { id: "efektivnoIzvrseniMinuta", label: "Efektivno izvršeni časovi (min)" },
  {
    id: "casoviCekanjaIZastojaMinuta",
    label: "Časovi čekanja, zastoja i prekida (min)",
  },
  { id: "prekovremeniMinuta", label: "Prekovremeni časovi (min)" },
] as const;

/** Every minute-bearing form field, in the order the request carries them. */
const MINUTE_KEYS = [
  "moguciMinuta",
  "efektivnoIzvrseniMinuta",
  "casoviCekanjaIZastojaMinuta",
  "prekovremeniMinuta",
  "nocniMinuta",
  "radNaPraznikMinuta",
  "odsustvoMinuta",
] as const satisfies readonly (keyof SaveWorkTimeEntryRequest)[];

type MinuteKey = (typeof MINUTE_KEYS)[number];

interface EntryFormState {
  dan: string;
  moguciMinuta: string;
  efektivnoIzvrseniMinuta: string;
  casoviCekanjaIZastojaMinuta: string;
  prekovremeniMinuta: string;
  nocniMinuta: string;
  radNaPraznikMinuta: string;
  kategorijaOdsustva: string;
  odsustvoMinuta: string;
  capOverrideRazlog: string;
  korekcijaRazlog: string;
}

function emptyForm(): EntryFormState {
  return {
    dan: todayIso(),
    moguciMinuta: "480",
    efektivnoIzvrseniMinuta: "",
    casoviCekanjaIZastojaMinuta: "",
    prekovremeniMinuta: "",
    nocniMinuta: "",
    radNaPraznikMinuta: "",
    kategorijaOdsustva: "",
    odsustvoMinuta: "",
    capOverrideRazlog: "",
    korekcijaRazlog: "",
  };
}

/**
 * The ZoR čl. 55 st. 6 daily working-time register.
 *
 * Four things this surface must not get wrong:
 *
 * 1. **No obrazac is claimed.** čl. 55 st. 6 delegates nothing and no ministerial
 *    act was ever made, so the heading sentence comes from the backend verbatim
 *    and nothing here is labelled a propisani obrazac.
 * 2. **The two computed columns are tagged as computed.** No Serbian provision
 *    requires night hours or holiday hours, so they carry the advisory tag and
 *    are never presented as a statutory field.
 * 3. **A cap breach asks, it never refuses.** The čl. 53 caps carry the larger
 *    fine, and a register that will not describe an unlawful day hides that
 *    exposure instead of surfacing it. A čl. 87–91 protection block is the
 *    opposite — the statute bans the work, so the row is refused.
 * 4. **The absence category is gated.** It is special-category data; every role
 *    but payroll sees „odsutan" plus an hour total.
 *
 * No penalty figure is composed here. Both notices arrive already resolved
 * against the shop's stored legal form.
 */
export function WorkTimeModule({ services, currentUser }: WorkTimeModuleProps) {
  const worktime = services.worktime;
  const [employees, setEmployees] = useState<UserAccount[]>([]);
  const [employeeId, setEmployeeId] = useState<number | null>(null);
  const [godina, setGodina] = useState(() => new Date().getFullYear());
  const [mesec, setMesec] = useState(() => new Date().getMonth() + 1);
  const [month, setMonth] = useState<WorkTimeMonth | null>(null);
  const [monthStatus, setMonthStatus] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [monthError, setMonthError] = useState<string | undefined>();
  const [notices, setNotices] = useState<WorkTimeNotices | null>(null);
  const [form, setForm] = useState<EntryFormState>(emptyForm);
  // Set the moment the backend answers `cap_override_required`: the write is not
  // refused, it is waiting for a čl. 53 st. 1 ground.
  const [capWarning, setCapWarning] = useState<string | null>(null);
  const [protections, setProtections] = useState<WorkTimeProtectionBlock[]>([]);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);
  const [reloadToken, setReloadToken] = useState(0);

  // Payroll-only, and closed whenever the role is unknown (§4 req. 25). The same
  // predicate gates the entry form: every write here is admin-gated backend-side,
  // so offering the controls to anyone else only invites a rejected command — and
  // the category picker would itself put the special-category vocabulary on a
  // screen that must not carry it.
  const isPayrollRole = currentUser?.role === "admin";
  const canSeeAbsenceReason = isPayrollRole;

  useEffect(() => {
    let cancelled = false;

    services.users.listUsers().then(
      (accounts) => {
        if (cancelled) {
          return;
        }
        const active = accounts.filter((account) => account.active);
        setEmployees(active);
        setEmployeeId((current) => current ?? active[0]?.id ?? null);
      },
      () => {
        // The employee list is a selector, not the record: a failed read must
        // leave the module diagnosable rather than blank.
        if (!cancelled) {
          setEmployees([]);
        }
      },
    );

    return () => {
      cancelled = true;
    };
  }, [services]);

  useEffect(() => {
    let cancelled = false;

    worktime.notices().then(
      (result) => {
        if (!cancelled) {
          setNotices(result);
        }
      },
      () => {
        // Advisory copy: a failed read costs the legal-basis panel, never the
        // register.
        if (!cancelled) {
          setNotices(null);
        }
      },
    );

    return () => {
      cancelled = true;
    };
  }, [worktime]);

  useEffect(() => {
    if (employeeId == null) {
      return;
    }

    let cancelled = false;
    setMonthStatus("loading");
    setMonthError(undefined);

    worktime.listMonth(employeeId, godina, mesec).then(
      (result) => {
        if (!cancelled) {
          setMonth(result);
          setMonthStatus("ready");
        }
      },
      (error) => {
        if (!cancelled) {
          setMonthError(errorMessage(error, "Evidencija nije učitana."));
          setMonthStatus("error");
        }
      },
    );

    return () => {
      cancelled = true;
    };
  }, [worktime, employeeId, godina, mesec, reloadToken]);

  // A pending čl. 53 ground, a protection finding and a half-typed day all
  // belong to the employee and month they were raised on. Carrying them across a
  // selector change would attach a warning to a period that never earned it —
  // and, worse, leave a chosen override reason armed for someone else's day.
  useEffect(() => {
    setForm(emptyForm());
    setCapWarning(null);
    setProtections([]);
    setSaveError(null);
  }, [employeeId, godina, mesec]);

  const zatvoren = month?.zatvoren ?? false;
  const liveEntries = (month?.entries ?? []).filter((entry) => !entry.zamenjen);

  const resetForm = useCallback(() => {
    setForm(emptyForm());
    setCapWarning(null);
    setProtections([]);
    setSaveError(null);
  }, []);

  async function submitEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (employeeId == null || zatvoren) {
      return;
    }

    const request = toRequest(form, employeeId);
    if (!request) {
      setSaveError("Datum i broj minuta moraju biti ispravni.");
      return;
    }

    setSaving(true);
    setSaveError(null);

    try {
      const saved = form.korekcijaRazlog
        ? await worktime.correctEntry({
            ...request,
            korekcijaRazlog: form.korekcijaRazlog as CorrectionReason,
          })
        : await worktime.saveEntry(request);

      setProtections(saved.protections);
      setCapWarning(null);
      resetForm();
      setReloadToken((token) => token + 1);
      toast.success("Dan je evidentiran", {
        description: formatDan(saved.entry.dan),
      });
    } catch (error) {
      const code = errorCode(error);

      if (code === "cap_override_required") {
        // Not a refusal — the day is recordable once a ground is chosen.
        setCapWarning(
          errorMessage(error, "Prekoračen je zakonski limit iz ZoR čl. 53."),
        );
        setSaveError(null);
      } else if (code === "protection_block") {
        setProtections(protectionsFrom(error));
        setSaveError(errorMessage(error, "Unos nije dozvoljen."));
      } else {
        setSaveError(errorMessage(error, "Dan nije evidentiran."));
      }
    } finally {
      setSaving(false);
    }
  }

  async function closePeriod() {
    if (employeeId == null) {
      return;
    }

    try {
      await worktime.closePeriod(employeeId, godina, mesec);
      setCloseDialogOpen(false);
      setReloadToken((token) => token + 1);
      toast.success("Period je zaključen", {
        description: `${MESECI[mesec - 1]} ${godina}.`,
      });
    } catch (error) {
      toast.error("Period nije zaključen", {
        description: errorMessage(error, "Zaključenje nije uspelo."),
      });
    }
  }

  async function exportCsv() {
    if (employeeId == null) {
      return;
    }

    let exported: ExportedFile;
    try {
      exported = await worktime.exportCsv(employeeId, godina, mesec);
    } catch (error) {
      toast.error("Izvoz nije uspeo", {
        description: errorMessage(error, "Evidencija nije izvezena."),
      });
      return;
    }

    toast.success("Evidencija je sačuvana", { description: exported.path });
  }

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <h2 className="text-lg font-semibold">Evidencija radnog vremena</h2>
            {zatvoren ? (
              <Badge variant="secondary">
                <LockIcon data-icon="inline-start" aria-hidden="true" />
                Period je zaključen
              </Badge>
            ) : null}
          </div>
          <p className="text-sm text-muted-foreground">
            {month?.napomena ??
              "Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac."}
          </p>
        </div>
        <div className="flex flex-wrap items-end gap-2">
          <Field className="w-auto">
            <FieldLabel htmlFor="worktime-zaposleni">Zaposleni</FieldLabel>
            <NativeSelect
              id="worktime-zaposleni"
              value={employeeId == null ? "" : String(employeeId)}
              onChange={(event) => setEmployeeId(Number(event.target.value))}
            >
              {employees.map((employee) => (
                <NativeSelectOption key={employee.id} value={String(employee.id)}>
                  {employee.displayName}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          <Field className="w-auto">
            <FieldLabel htmlFor="worktime-godina">Godina</FieldLabel>
            <NativeSelect
              id="worktime-godina"
              value={String(godina)}
              onChange={(event) => setGodina(Number(event.target.value))}
            >
              {buildYearOptions(new Date().getFullYear()).map((year) => (
                <NativeSelectOption key={year} value={String(year)}>
                  {year}.
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          <Field className="w-auto">
            <FieldLabel htmlFor="worktime-mesec">Mesec</FieldLabel>
            <NativeSelect
              id="worktime-mesec"
              value={String(mesec)}
              onChange={(event) => setMesec(Number(event.target.value))}
            >
              {MESECI.map((label, index) => (
                <NativeSelectOption key={label} value={String(index + 1)}>
                  {label}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          {isPayrollRole ? (
            <Button type="button" variant="outline" onClick={() => void exportCsv()}>
              <DownloadIcon aria-hidden="true" />
              Izvezi
            </Button>
          ) : null}
          {isPayrollRole && !zatvoren ? (
            <Button
              type="button"
              variant="outline"
              onClick={() => setCloseDialogOpen(true)}
            >
              <LockIcon aria-hidden="true" />
              Zaključi period
            </Button>
          ) : null}
        </div>
      </div>

      <Alert>
        <ClockIcon aria-hidden="true" />
        <AlertTitle>Noćni časovi i časovi rada na praznik</AlertTitle>
        <AlertDescription>
          Ove dve kolone su {month?.advisoryNapomena ?? "izračunato radi provere usklađenosti"}.
        </AlertDescription>
      </Alert>

      {capWarning ? (
        <Alert variant="destructive" role="alert">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Prekoračen limit radnog vremena</AlertTitle>
          <AlertDescription>
            <p>{capWarning}</p>
            {notices ? <p>{notices.capsExceeded.summary}</p> : null}
            {notices?.capsExceeded.penalty ? (
              <p>{notices.capsExceeded.penalty}</p>
            ) : null}
            {notices ? <p>{notices.capsExceeded.citation}</p> : null}
          </AlertDescription>
        </Alert>
      ) : null}

      {protections.map((block) => (
        <Alert
          key={block.kind}
          variant={block.blocking ? "destructive" : "default"}
        >
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>
            {block.blocking ? "Unos nije dozvoljen" : "Napomena o zaštiti zaposlenog"}
          </AlertTitle>
          <AlertDescription>{block.poruka}</AlertDescription>
        </Alert>
      ))}

      {saveError ? (
        <Alert variant="destructive">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Dan nije evidentiran</AlertTitle>
          <AlertDescription>{saveError}</AlertDescription>
        </Alert>
      ) : null}

      {isPayrollRole ? (
        <form
          className="rounded-lg border p-4"
          onSubmit={(event) => void submitEntry(event)}
        >
          <FieldGroup>
            <div className="flex flex-wrap items-end gap-3">
              <Field className="w-auto">
                <FieldLabel htmlFor="worktime-dan">Datum</FieldLabel>
                <Input
                  id="worktime-dan"
                  type="date"
                  value={form.dan}
                  disabled={zatvoren}
                  onChange={(event) =>
                    setForm({ ...form, dan: event.target.value })
                  }
                />
              </Field>
              {MINUTE_FIELDS.map((field) => (
                <Field key={field.id} className="w-auto">
                  <FieldLabel htmlFor={`worktime-${field.id}`}>
                    {field.label}
                  </FieldLabel>
                  <Input
                    id={`worktime-${field.id}`}
                    type="number"
                    min={0}
                    step={1}
                    value={form[field.id]}
                    disabled={zatvoren}
                    onChange={(event) =>
                      setForm({
                        ...form,
                        [field.id]: event.target.value,
                      } as EntryFormState)
                    }
                  />
                </Field>
              ))}
            </div>

            <div className="flex flex-wrap items-end gap-3">
              <Field className="w-auto">
                <FieldLabel htmlFor="worktime-nocniMinuta">
                  Noćni časovi (min)
                </FieldLabel>
                <Input
                  id="worktime-nocniMinuta"
                  type="number"
                  min={0}
                  step={1}
                  value={form.nocniMinuta}
                  disabled={zatvoren}
                  onChange={(event) =>
                    setForm({ ...form, nocniMinuta: event.target.value })
                  }
                />
                <FieldDescription>{ADVISORY_HINT}</FieldDescription>
              </Field>
              <Field className="w-auto">
                <FieldLabel htmlFor="worktime-radNaPraznikMinuta">
                  Časovi rada na praznik (min)
                </FieldLabel>
                <Input
                  id="worktime-radNaPraznikMinuta"
                  type="number"
                  min={0}
                  step={1}
                  value={form.radNaPraznikMinuta}
                  disabled={zatvoren}
                  onChange={(event) =>
                    setForm({ ...form, radNaPraznikMinuta: event.target.value })
                  }
                />
                <FieldDescription>{ADVISORY_HINT}</FieldDescription>
              </Field>
            </div>

            <div className="flex flex-wrap items-end gap-3">
              <Field className="w-auto">
                <FieldLabel htmlFor="worktime-kategorija">
                  Kategorija odsustva
                </FieldLabel>
                <NativeSelect
                  id="worktime-kategorija"
                  value={form.kategorijaOdsustva}
                  disabled={zatvoren}
                  onChange={(event) =>
                    setForm({ ...form, kategorijaOdsustva: event.target.value })
                  }
                >
                  <NativeSelectOption value="">Bez odsustva</NativeSelectOption>
                  {KATEGORIJE_ODSUSTVA.map((kategorija) => (
                    <NativeSelectOption key={kategorija.id} value={kategorija.id}>
                      {kategorija.label}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </Field>
              <Field className="w-auto">
                <FieldLabel htmlFor="worktime-odsustvo">
                  Časovi odsustva (min)
                </FieldLabel>
                <Input
                  id="worktime-odsustvo"
                  type="number"
                  min={0}
                  step={1}
                  value={form.odsustvoMinuta}
                  disabled={zatvoren || !form.kategorijaOdsustva}
                  onChange={(event) =>
                    setForm({ ...form, odsustvoMinuta: event.target.value })
                  }
                />
              </Field>
              <Field className="w-auto">
                <FieldLabel htmlFor="worktime-korekcija">Razlog ispravke</FieldLabel>
                <NativeSelect
                  id="worktime-korekcija"
                  value={form.korekcijaRazlog}
                  disabled={zatvoren}
                  onChange={(event) =>
                    setForm({ ...form, korekcijaRazlog: event.target.value })
                  }
                >
                  <NativeSelectOption value="">Novi unos</NativeSelectOption>
                  {RAZLOZI_ISPRAVKE.map((razlog) => (
                    <NativeSelectOption key={razlog.id} value={razlog.id}>
                      {razlog.label}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
                <FieldDescription>
                  Ispravka se dodaje kao nova verzija dana; raniji unos ostaje.
                </FieldDescription>
              </Field>
            </div>

            {capWarning || form.capOverrideRazlog ? (
              <Field className="w-auto">
                <FieldLabel htmlFor="worktime-cap-razlog">
                  Razlog prekoračenja
                </FieldLabel>
                <NativeSelect
                  id="worktime-cap-razlog"
                  value={form.capOverrideRazlog}
                  disabled={zatvoren}
                  onChange={(event) =>
                    setForm({ ...form, capOverrideRazlog: event.target.value })
                  }
                >
                  <NativeSelectOption value="">Izaberite razlog</NativeSelectOption>
                  {RAZLOZI_PREKORACENJA.map((razlog) => (
                    <NativeSelectOption key={razlog.id} value={razlog.id}>
                      {razlog.label}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
                <FieldDescription>
                  Razlog se evidentira uz dan. Evidentiranje razloga ne čini
                  prekoračenje dozvoljenim.
                </FieldDescription>
              </Field>
            ) : null}

            <div className="flex items-center gap-2">
              <Button type="submit" disabled={zatvoren || saving}>
                {saving ? (
                  <Spinner data-icon="inline-start" aria-hidden="true" />
                ) : null}
                Sačuvaj dan
              </Button>
              <Button type="button" variant="ghost" onClick={resetForm}>
                Poništi
              </Button>
            </div>
          </FieldGroup>
        </form>
      ) : (
        <Alert>
          <ClockIcon aria-hidden="true" />
          <AlertTitle>Pregled evidencije</AlertTitle>
          <AlertDescription>
            Unos i ispravku evidencije radnog vremena vodi poslodavac.
          </AlertDescription>
        </Alert>
      )}

      <Separator />

      {monthStatus === "loading" ? (
        <Badge variant="outline">
          <Spinner data-icon="inline-start" aria-hidden="true" />
          Učitavanje evidencije
        </Badge>
      ) : null}

      {monthStatus === "error" ? (
        <Alert variant="destructive">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Evidencija nije učitana</AlertTitle>
          <AlertDescription>{monthError}</AlertDescription>
        </Alert>
      ) : null}

      {monthStatus === "ready" && month && month.entries.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ClockIcon aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>Nema unosa za izabrani mesec</EmptyTitle>
            <EmptyDescription>
              Evidencija se vodi po danu, za svakog zaposlenog.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : null}

      {monthStatus === "ready" && month && month.entries.length > 0 ? (
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Datum</TableHead>
                <TableHead>Verzija</TableHead>
                <TableHead>Odsustvo</TableHead>
                <TableHead className="text-right">Mogući</TableHead>
                <TableHead className="text-right">Ukupno ostvareni</TableHead>
                <TableHead className="text-right">Efektivno izvršeni</TableHead>
                <TableHead className="text-right">Čekanja i zastoji</TableHead>
                <TableHead className="text-right">Ukupno neizvršeni</TableHead>
                <TableHead className="text-right">Prekovremeni</TableHead>
                <TableHead className="text-right">Noćni*</TableHead>
                <TableHead className="text-right">Rad na praznik*</TableHead>
                <TableHead>Uneo</TableHead>
                <TableHead>Napomena</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {month.entries.map((entry) => (
                <TableRow
                  key={entry.id}
                  className={entry.zamenjen ? "text-muted-foreground line-through" : undefined}
                >
                  <TableCell>{formatDan(entry.dan)}</TableCell>
                  <TableCell>{entry.verzija}</TableCell>
                  <TableCell>
                    <AbsenceCell
                      entry={entry}
                      canSeeAbsenceReason={canSeeAbsenceReason}
                    />
                  </TableCell>
                  <MinuteCell value={entry.minuti.moguciMinuta} />
                  <MinuteCell value={entry.minuti.ukupnoOstvareniMinuta} />
                  <MinuteCell value={entry.minuti.efektivnoIzvrseniMinuta} />
                  <MinuteCell value={entry.minuti.casoviCekanjaIZastojaMinuta} />
                  <MinuteCell value={entry.minuti.ukupnoNeizvrseniMinuta} />
                  <MinuteCell value={entry.minuti.prekovremeniMinuta} />
                  <MinuteCell value={entry.minuti.nocniMinuta} />
                  <MinuteCell value={entry.minuti.radNaPraznikMinuta} />
                  <TableCell>{entry.unioIme ?? "—"}</TableCell>
                  <TableCell>{napomenaZa(entry)}</TableCell>
                </TableRow>
              ))}
              <TableRow className="font-medium">
                <TableCell>Ukupno</TableCell>
                <TableCell />
                <TableCell>{`${liveEntries.length} dana`}</TableCell>
                <MinuteCell value={month.ukupno.moguciMinuta} />
                <MinuteCell value={month.ukupno.ukupnoOstvareniMinuta} />
                <MinuteCell value={month.ukupno.efektivnoIzvrseniMinuta} />
                <MinuteCell value={month.ukupno.casoviCekanjaIZastojaMinuta} />
                <MinuteCell value={month.ukupno.ukupnoNeizvrseniMinuta} />
                <MinuteCell value={month.ukupno.prekovremeniMinuta} />
                <MinuteCell value={month.ukupno.nocniMinuta} />
                <MinuteCell value={month.ukupno.radNaPraznikMinuta} />
                <TableCell />
                <TableCell />
              </TableRow>
            </TableBody>
          </Table>
          <p className="mt-2 text-xs text-muted-foreground">
            * {month.advisoryNapomena}.
          </p>
        </div>
      ) : null}

      {notices ? (
        <div className="rounded-lg border p-4">
          <h3 className="text-sm font-semibold">Zakonska osnova</h3>
          <div className="mt-2 flex flex-col gap-3 text-sm text-muted-foreground">
            <NoticeBlock notice={notices.recordMissing} />
            <NoticeBlock notice={notices.capsExceeded} />
          </div>
        </div>
      ) : null}

      <Dialog open={closeDialogOpen} onOpenChange={setCloseDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Zaključivanje perioda</DialogTitle>
            <DialogDescription>
              {`Zaključujete ${MESECI[mesec - 1]} ${godina}. za zaposlenog ${
                month?.zaposleni ?? ""
              }. Zaključenje je konačno — posle njega se ni jedan dan tog meseca više ne može evidentirati ni ispraviti.`}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setCloseDialogOpen(false)}
            >
              Odustani
            </Button>
            <Button type="button" onClick={() => void closePeriod()}>
              Zaključi
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </section>
  );
}

const ADVISORY_HINT =
  "Kolona izračunata radi provere usklađenosti — nije obavezno polje.";

function MinuteCell({ value }: { value: number }) {
  return (
    <TableCell className="text-right tabular-nums">
      {value === 0 ? "—" : formatMinutes(value)}
    </TableCell>
  );
}

/**
 * The §4 req. 25 gate, rendered.
 *
 * The absence **category** is special-category data under ZZPL čl. 17 — the two
 * sprečenost buckets alone say which of a poslodavac-funded and an RFZO-funded
 * sick leave a day was. Payroll sees the category; every other role, and every
 * unknown one, sees „odsutan" plus the hour total and nothing that identifies
 * why.
 */
function AbsenceCell({
  entry,
  canSeeAbsenceReason,
}: {
  entry: WorkTimeEntryView;
  canSeeAbsenceReason: boolean;
}) {
  if (!entry.kategorijaOdsustva) {
    return <span className="text-muted-foreground">—</span>;
  }

  if (!canSeeAbsenceReason) {
    return <span>Odsutan</span>;
  }

  const kategorija = KATEGORIJE_ODSUSTVA.find(
    (candidate) => candidate.id === entry.kategorijaOdsustva,
  );

  return <span>{kategorija?.label ?? entry.kategorijaOdsustva}</span>;
}

function NoticeBlock({ notice }: { notice: LegalNotice }) {
  return (
    <div>
      <p>{notice.summary}</p>
      {notice.penalty ? <p>{notice.penalty}</p> : null}
      <p>{notice.citation}</p>
    </div>
  );
}

/** The correction / override grounds recorded on a row, as readable labels. */
function napomenaZa(entry: WorkTimeEntryView): string {
  const parts: string[] = [];

  if (entry.korekcijaRazlog) {
    const razlog = RAZLOZI_ISPRAVKE.find(
      (candidate) => candidate.id === entry.korekcijaRazlog,
    );
    parts.push(`Ispravka: ${razlog?.label ?? entry.korekcijaRazlog}`);
  }

  if (entry.capOverrideRazlog) {
    const razlog = RAZLOZI_PREKORACENJA.find(
      (candidate) => candidate.id === entry.capOverrideRazlog,
    );
    parts.push(`Prekoračenje: ${razlog?.label ?? entry.capOverrideRazlog}`);
  }

  return parts.length > 0 ? parts.join(" · ") : "—";
}

/**
 * Integer minutes rendered as časovi + minuti. Never a decimal hour: 7 h 30 min
 * is 450, and 7,5 is a number this application does not compute with.
 */
export function formatMinutes(minutes: number): string {
  const sign = minutes < 0 ? "-" : "";
  const absolute = Math.abs(minutes);
  const hours = Math.floor(absolute / 60);
  const rest = absolute % 60;

  return `${sign}${hours} č ${String(rest).padStart(2, "0")} min`;
}

/**
 * A form field to stored minutes. Empty is zero; anything that is not a
 * non-negative whole number is rejected, never coerced — a silently coerced
 * figure lands in a statutory register.
 */
function parseMinutes(value: string): number | null {
  const trimmed = value.trim();

  if (trimmed === "") {
    return 0;
  }

  if (!/^\d+$/.test(trimmed)) {
    return null;
  }

  return Number(trimmed);
}

function toRequest(
  form: EntryFormState,
  userId: number,
): SaveWorkTimeEntryRequest | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(form.dan)) {
    return null;
  }

  const minutes = {} as Record<MinuteKey, number>;

  for (const key of MINUTE_KEYS) {
    const parsed = parseMinutes(form[key]);

    if (parsed == null) {
      return null;
    }

    minutes[key] = parsed;
  }

  return {
    userId,
    dan: form.dan,
    ...minutes,
    kategorijaOdsustva: form.kategorijaOdsustva
      ? (form.kategorijaOdsustva as AbsenceCategory)
      : null,
    capOverrideRazlog: form.capOverrideRazlog
      ? (form.capOverrideRazlog as CapOverrideReason)
      : null,
  };
}

function buildYearOptions(currentYear: number): number[] {
  const years: number[] = [];

  for (let offset = 0; offset <= YEAR_RANGE; offset += 1) {
    years.push(currentYear - offset);
  }

  return years;
}

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

/**
 * Formats a `YYYY-MM-DD` day off its own calendar components. Parsing to a
 * `Date` first would re-project the instant into the viewer's timezone and can
 * shift a legally-meaningful day by one.
 */
function formatDan(value: string): string {
  const [year, month, day] = value.slice(0, 10).split("-");

  if (!year || !month || !day) {
    return value;
  }

  return `${day}.${month}.${year}.`;
}

function errorCode(error: unknown): string | null {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof (error as { code: unknown }).code === "string"
  ) {
    return (error as { code: string }).code;
  }

  return null;
}

function protectionsFrom(error: unknown): WorkTimeProtectionBlock[] {
  if (
    typeof error === "object" &&
    error !== null &&
    "details" in error &&
    typeof (error as { details: unknown }).details === "object" &&
    (error as { details: unknown }).details !== null
  ) {
    const details = (error as { details: Record<string, unknown> }).details;

    if (Array.isArray(details.protections)) {
      return details.protections as WorkTimeProtectionBlock[];
    }
  }

  return [];
}

function errorMessage(error: unknown, fallback: string): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  return fallback;
}

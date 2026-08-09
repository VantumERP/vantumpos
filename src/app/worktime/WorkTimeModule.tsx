import {
  AlertCircleIcon,
  ClockIcon,
  DownloadIcon,
  EyeOffIcon,
  LockIcon,
} from "lucide-react";
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
import { MESECI, nazivPerioda } from "@/lib/period";
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
  WorkTimeCapAssessment,
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
   * unknown one — sees „odsutan“ plus an hour total and nothing more.
   */
  currentUser?: UserAccount;
}

/** How many prior years the period selector offers alongside the current one. */
const YEAR_RANGE = 3;


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

function emptyForm(godina: number, mesec: number): EntryFormState {
  return {
    dan: defaultDan(godina, mesec),
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
 *    but payroll sees „odsutan“ plus an hour total.
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
  const [form, setForm] = useState<EntryFormState>(() =>
    emptyForm(godina, mesec),
  );
  // Set the moment the backend answers `cap_override_required`: the write is not
  // refused, it is waiting for a čl. 53 st. 1 ground.
  const [capWarning, setCapWarning] = useState<string | null>(null);
  // The assessment the last write was measured against, from BOTH paths: the
  // `cap_override_required` detail and the successful save alike.
  //
  // It decides two things the module cannot get from anywhere else. Which of the
  // two ceiling notices applies — čl. 53 or the čl. 57 st. 5 preraspodela one —
  // and whether a day that saved without needing an override nonetheless carries
  // a standing daily-cap finding, which is the only way a thirteen-hour
  // preraspodela day is ever mentioned to anyone.
  const [caps, setCaps] = useState<WorkTimeCapAssessment | null>(null);
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
    setForm(emptyForm(godina, mesec));
    setCapWarning(null);
    setCaps(null);
    setProtections([]);
    setSaveError(null);
  }, [employeeId, godina, mesec]);

  const zatvoren = month?.zatvoren ?? false;
  const liveEntries = (month?.entries ?? []).filter((entry) => !entry.zamenjen);
  const capNotice = capNoticeFor(notices, caps);

  // The day picker never offers a day the register may not describe: outside the
  // displayed period the row would save and then be invisible (`listMonth`
  // filters by period), and after today it would describe hours that have not
  // been worked. `toRequest` re-checks both — the picker is a convenience.
  const danMin = periodStart(godina, mesec);
  const danKrajPerioda = periodEnd(godina, mesec);
  const danas = todayIso();
  const danMax = danKrajPerioda < danas ? danKrajPerioda : danas;

  const resetForm = useCallback(() => {
    setForm(emptyForm(godina, mesec));
    setCapWarning(null);
    setCaps(null);
    setProtections([]);
    setSaveError(null);
  }, [godina, mesec]);

  async function submitEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (employeeId == null || zatvoren) {
      return;
    }

    // Every finding on screen belongs to an attempt that is over the moment this
    // one starts, so the whole assessment is dropped here — above `toRequest`,
    // not below it. `toRequest` is a second exit from this function and a
    // client-side refusal is an attempt too: an operator whose Saturday was
    // refused on čl. 87 and who then clears the Datum field or types „7,5“ into
    // a minute box would otherwise read „Unos nije dozvoljen … 35 časova
    // nedeljno … već je evidentirano 35 č 00 min“ above „Dan nije evidentiran —
    // Datum mora biti u obliku gggg-MM-dd“, with nothing submitted at all. That
    // is a destructive alert asserting a refusal that did not happen, naming a
    // weekly figure for a week this attempt never touched. Same rationale as the
    // `[employeeId, godina, mesec]` effect above, one scope narrower.
    setProtections([]);
    setCaps(null);
    setCapWarning(null);
    setSaveError(null);

    const validated = toRequest(form, employeeId, godina, mesec);
    if (!validated.ok) {
      setSaveError(validated.poruka);
      return;
    }

    const request = validated.request;

    setSaving(true);

    try {
      const saved = form.korekcijaRazlog
        ? await worktime.correctEntry({
            ...request,
            korekcijaRazlog: form.korekcijaRazlog as CorrectionReason,
          })
        : await worktime.saveEntry(request);

      resetForm();
      // AFTER the reset, never before it: `resetForm` clears the previous
      // assessment, so applying the findings first would leave the array empty
      // by the time React renders. The backend hands čl. 87–91 findings back on
      // the SUCCESS path precisely because they are non-blocking — čl. 90 is a
      // warning the operator must see, and `NeispravanDatumUProfilu` says the
      // under-18 and čl. 91 consent guards could not run for this day at all.
      // Dropping them here is the only place they can be silently lost.
      setProtections(saved.protections);
      // Same reasoning, same path, and the same failure mode: `assess_caps_for_employee`
      // deliberately leaves `dailyCapExceeded` standing for an employee in
      // preraspodela — the stored profile carries one `radi_u_preraspodeli`
      // flag and it cannot tell čl. 57 preraspodela, which has no daily leg,
      // from the čl. 56 st. 3 scheme, where čl. 56 st. 4 expressly restates 12
      // časova dnevno. The write is not gated on it, so discarding it here is
      // the only place a thirteen-hour day can pass in total silence — and it is
      // a finding on the čl. 274 side, the larger fine.
      setCaps(saved.caps);
      setReloadToken((token) => token + 1);
      toast.success("Dan je evidentiran", {
        description: formatDan(saved.entry.dan),
      });
    } catch (error) {
      const code = errorCode(error);

      // The drop at the top of `submitEntry` already covers every attempt that
      // reaches this far, and this one is kept for the narrow case it does not:
      // the success path sets `protections` and `caps` from the saved day
      // *before* the reload and the toast, so a throw after that point would
      // otherwise leave a successful save's findings standing over „Dan nije
      // evidentiran“. The branches below each set a different subset of the
      // assessment and none of them clears the rest.
      setProtections([]);
      setCaps(null);
      setCapWarning(null);
      setSaveError(null);

      if (code === "cap_override_required") {
        // Not a refusal — the day is recordable once a ground is chosen. The
        // assessment rides along so the alert can name the ceiling that was
        // actually breached: čl. 53, or the čl. 57 st. 5 preraspodela one.
        setCapWarning(
          errorMessage(error, "Prekoračen je zakonski limit iz ZoR čl. 53."),
        );
        setCaps(capsFrom(error));
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
          {/*
            The backend tag is an invariable phrase — „izračunato radi provere
            usklađenosti“ cannot agree with a plural feminine subject — so the
            sentence quotes it instead of inflecting it.
          */}
          {`Ove dve kolone nose oznaku „${month?.advisoryNapomena ?? ADVISORY_TAG}“.`}
        </AlertDescription>
      </Alert>

      {capWarning ? (
        <Alert variant="destructive" role="alert">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Prekoračen limit radnog vremena</AlertTitle>
          <AlertDescription>
            <p>{capWarning}</p>
            {capNotice ? <p>{capNotice.summary}</p> : null}
            {capNotice?.penalty ? <p>{capNotice.penalty}</p> : null}
            {capNotice ? <p>{capNotice.citation}</p> : null}
          </AlertDescription>
        </Alert>
      ) : null}

      {/*
        A day that exceeded twelve hours and was NOT gated on it. Only one
        situation produces that combination: the employee's profile says
        preraspodela, so `assess_caps_for_employee` swapped the čl. 53 st. 3
        daily leg for the čl. 57 st. 5 weekly one — and the week stayed under
        sixty hours.

        Which ceiling actually binds him is a question the stored profile cannot
        answer, so this states both and says so instead of picking one: čl. 57
        preraspodela has no daily leg at all, while čl. 56 st. 4 expressly
        restates twelve hours for the čl. 56 st. 3 scheme. Non-blocking and not
        `destructive` — the day is recorded, and this is a finding to check, not
        a refusal.
      */}
      {caps?.dailyCapExceeded && !caps.requiresOverride ? (
        <Alert>
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Dnevno radno vreme prelazi 12 časova</AlertTitle>
          <AlertDescription>
            <p>
              Dan je evidentiran. Ukupno radno vreme tog dana prelazi 12 časova,
              a koji dnevni limit važi zavisi od šeme po kojoj zaposleni radi:
              preraspodela iz ZoR čl. 57 nema dnevni limit, dok za preraspodelu
              iz ZoR čl. 56 st. 3 važi ZoR čl. 56 st. 4, koji izričito propisuje
              najviše 12 časova dnevno. Van preraspodele važi ZoR čl. 53 st. 3.
            </p>
            <p>Proverite po kojoj šemi zaposleni radi.</p>
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
                  min={danMin}
                  max={danMax}
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

      {monthStatus === "ready" && month?.razlogOdsustvaSkriven ? (
        <Alert>
          <EyeOffIcon aria-hidden="true" />
          <AlertTitle>Razlog odsustva je skriven</AlertTitle>
          <AlertDescription>{RAZLOG_SKRIVEN_OBJASNJENJE}</AlertDescription>
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
                      razlogSkriven={month.razlogOdsustvaSkriven}
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
                <TableCell>{danaSaUnosom(liveEntries.length)}</TableCell>
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
          {/*
            Neutral container heading. Each notice states its own standing —
            a duty is not the same thing as a recommendation, and only the
            notice itself knows which it is.
          */}
          <h3 className="text-sm font-semibold">Pravne napomene</h3>
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

/**
 * The notice that belongs beside a cap refusal — the čl. 53 one, or the čl. 57
 * st. 5 preraspodela one.
 *
 * They are not interchangeable. čl. 58 says hours worked in preraspodela are not
 * prekovremeni rad, so čl. 53 st. 2 and st. 3 do not bind such an employee at
 * all, and the backend refuses his day on the 60 h weekly ceiling instead
 * (`assess_caps_for_employee`). Showing `capsExceeded` there states a rule that
 * does not apply to him and cites čl. 274 st. 1 **tač. 3**, when čl. 57 and
 * čl. 60 are **tač. 4**.
 *
 * Both tačke resolve through the same čl. 274 st. 2 for a preduzetnik, so the
 * amount is identical and nothing about the money would ever betray the swap —
 * which is exactly why the choice has to be made here rather than left to the
 * one notice that happens to be nearest.
 */
export function capNoticeFor(
  notices: WorkTimeNotices | null,
  caps: WorkTimeCapAssessment | null,
): LegalNotice | null {
  if (!notices) {
    return null;
  }

  return caps?.preraspodelaWeeklyCapExceeded
    ? notices.preraspodelaCapsExceeded
    : notices.capsExceeded;
}

const ADVISORY_HINT =
  "Kolona izračunata radi provere usklađenosti — nije obavezno polje.";

/**
 * `crate::commands::worktime::ADVISORY_TAG`, used only when the month has not
 * loaded yet. An invariable phrase: quote it, never inflect it.
 */
const ADVISORY_TAG = "izračunato radi provere usklađenosti";

/**
 * Why the „Odsustvo“ column is withholding, and where the shop decides otherwise.
 *
 * A „skriveno“ state with no stated cause reads as a defect in the register — an
 * operator who cannot see why assumes the data is missing. So the sentence names
 * the control doing it (the čl. 46 nalog), says it lasts only as long as that
 * nalog, and points at the one surface that can lift it. It does **not** offer to
 * lift it here: the decision is the shop's and belongs beside the nalog, and a
 * second entry point would let it be made by someone reading a payroll screen.
 *
 * **Scoped to this register, because that is all the mask covers.** `my_hours`
 * reads the category unmasked whatever the nalog says — the deliberate ZZPL
 * čl. 26 carve-out — so a sentence saying the category „se ne prikazuje“ without
 * naming where would deny behaviour „Moji sati“ has, which house rule 11 bars
 * even when the reader happens to be standing in front of the register.
 */
const RAZLOG_SKRIVEN_OBJASNJENJE =
  "Kategorija odsustva se u ovoj evidenciji ne prikazuje dok važi nalog za pristup " +
  "tehničke podrške (ZZPL čl. 46). Broj časova odsustva ostaje prikazan. Otkrivanje " +
  "za taj nalog vlasnik odobrava na kartici „Privatnost“, uz „Daljinska podrška“.";

function MinuteCell({ value }: { value: number }) {
  return (
    <TableCell className="text-right tabular-nums">
      {value === 0 ? "—" : formatMinutes(value)}
    </TableCell>
  );
}

/**
 * The §4 req. 25 gate and the req. 28 mask, rendered.
 *
 * The absence **category** is special-category data under ZZPL čl. 17 — the two
 * sprečenost buckets alone say which of a poslodavac-funded and an RFZO-funded
 * sick leave a day was. With no support nalog live, payroll sees the category;
 * every other role, and every unknown one, sees „odsutan“ plus the hour total
 * and nothing that identifies why.
 *
 * **`razlogSkriven` comes from the read and is not re-derived here, which is the
 * whole point.** Since 08.08.2026 the backend withholds the category while a
 * ZZPL čl. 46 nalog za daljinsku podršku is open (SW-14 req. 28), so
 * `entry.kategorijaOdsustva` is `null` on a day that *is* an absence. A `null`
 * has two causes — nothing was recorded, or nothing was read — and this column's
 * „—“ asserts the first. Rendering it under a mask says „nema odsustva“ beside an
 * „Ukupno neizvršeni“ cell reading 480, which is false by omission and was the
 * defect this branch order fixes. `WorkTimeMonth.razlogOdsustvaSkriven` is the
 * read's own statement of which cause applies, so the cell states it instead of
 * guessing, and the mask branch runs first: while it is set, no category is
 * printed even if one somehow arrived on the row.
 *
 * **The absence itself is read off the absence buckets, and v) is not all of
 * them.** `ukupnoNeizvrseniMinuta` is the sum of **nine** of the ten categories'
 * ZEOR čl. 24 tač. 1 buckets; the tenth, `obustava_rada_strajk`, books into
 * `obustavaRadaStrajkMinuta`, which `derive_totals` puts inside b) „ukupno
 * ostvareni“ as its third indent. Reading v) alone therefore rendered „—“ on a
 * full shift of štrajk — the same „nema odsustva“ falsehood, for one category in
 * ten, at any minute count. Both are tested, and neither names
 * `kategorijaOdsustva`: the buckets ride on every read, masked or not, so this
 * statement is derived from what the masked payload already carries and the
 * guard `the_masked_register_read_never_names_the_absence_reason_column` has
 * nothing to say about it. Zero in both is the row saying it booked no absence
 * minutes, and the cell stays „—“, because „Odsutan“ there would invent an
 * absence.
 *
 * **The residual, stated rather than implied.** A category booked with **zero**
 * minutes moves no bucket at all, so while masked such a day is indistinguishable
 * from a day with no absence and renders „—“. That is the whole of what is left
 * open, and the reason is that nothing on the payload changes — not that the bit
 * would require the withheld column. The narrow corner is left as it is rather
 * than bought with a per-entry flag nobody has asked the read to carry.
 */
export function AbsenceCell({
  entry,
  canSeeAbsenceReason,
  razlogSkriven,
}: {
  entry: WorkTimeEntryView;
  canSeeAbsenceReason: boolean;
  razlogSkriven: boolean;
}) {
  if (razlogSkriven) {
    const odsustvoMinuta =
      entry.minuti.ukupnoNeizvrseniMinuta + entry.minuti.obustavaRadaStrajkMinuta;
    return odsustvoMinuta > 0 ? (
      <span title={RAZLOG_SKRIVEN_OBJASNJENJE}>Odsutan (razlog skriven)</span>
    ) : (
      <span className="text-muted-foreground">—</span>
    );
  }

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

/**
 * One notice, under a heading that states what it actually is.
 *
 * `isLegalDuty` is the whole reason the flag exists. A [PRUDENTIAL] item — good
 * practice with no provision behind it — must never appear under „Zakonska
 * osnova“, because that heading asserts an obligation the shop does not carry.
 * Both notices routed here today are duties; the branch is what keeps the first
 * non-duty one from being misstated the day it is added.
 */
function NoticeBlock({ notice }: { notice: LegalNotice }) {
  return (
    <div>
      <h4 className="font-medium text-foreground">
        {notice.isLegalDuty ? "Zakonska osnova" : "Preporuka"}
      </h4>
      <p>{notice.summary}</p>
      {notice.penalty ? <p>{notice.penalty}</p> : null}
      <p>{notice.citation}</p>
    </div>
  );
}

/** The correction / override grounds recorded on a row, as readable labels. */
export function napomenaZa(entry: WorkTimeEntryView): string {
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
 * The totals row's day count, as it appears in the „Odsustvo“ column.
 *
 * Two things it has to get right. It counts days that carry a live entry —
 * `crate::commands::worktime::Klasifikacija::dana_sa_unosom` — and *not* days
 * of absence, so it names itself: a bare figure under the „Odsustvo“ header
 * reads as absent days, on the one column carrying ZZPL čl. 17 special-category
 * data. And it agrees with the number — 1 dan, 2 dana, 5 dana, 21 dan — because
 * „1 dana“ is not Serbian.
 */
export function danaSaUnosom(count: number): string {
  const jednina = count % 10 === 1 && count % 100 !== 11;

  return `${count} ${jednina ? "dan" : "dana"} sa unosom`;
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

/** A validated request, or the Serbian sentence that says why there isn't one. */
type RequestResult =
  | { ok: true; request: SaveWorkTimeEntryRequest }
  | { ok: false; poruka: string };

/**
 * Whether a day may be written into the register — the Serbian sentence refusing
 * it, or `null` when it may.
 *
 * The picker's `min`/`max` narrow what can be chosen; **this** is the check. A
 * `type="date"` input is a convenience, not a guarantee: its value can be
 * pasted, autofilled or set by a browser that honours neither attribute, and a
 * statutory register must not depend on a widget for its accuracy.
 */
export function validateDan(
  dan: string,
  godina: number,
  mesec: number,
): string | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(dan)) {
    return "Datum mora biti u obliku gggg-MM-dd.";
  }

  // A day that has not happened is not a „dnevna evidencija“ within ZoR čl. 55
  // st. 6, and a register describing hours nobody has worked yet is not an
  // accurate one.
  if (dan > todayIso()) {
    return "Ne može se evidentirati dan koji još nije protekao. Evidentira se dan koji se dogodio.";
  }

  // Outside the displayed period the row saves and then vanishes: `listMonth`
  // filters by period, so the operator would be told „Dan je evidentiran“ about
  // a row they can never see again from this screen.
  if (dan < periodStart(godina, mesec) || dan > periodEnd(godina, mesec)) {
    return `Datum mora pripadati izabranom periodu — ${nazivPerioda(godina, mesec)}.`;
  }

  return null;
}

function toRequest(
  form: EntryFormState,
  userId: number,
  godina: number,
  mesec: number,
): RequestResult {
  const danPoruka = validateDan(form.dan, godina, mesec);
  if (danPoruka) {
    return { ok: false, poruka: danPoruka };
  }

  const minutes = {} as Record<MinuteKey, number>;

  for (const key of MINUTE_KEYS) {
    const parsed = parseMinutes(form[key]);

    if (parsed == null) {
      return {
        ok: false,
        poruka: "Broj minuta mora biti ceo broj, bez decimala i bez minusa.",
      };
    }

    minutes[key] = parsed;
  }

  return {
    ok: true,
    request: {
      userId,
      dan: form.dan,
      godina,
      mesec,
      ...minutes,
      kategorijaOdsustva: form.kategorijaOdsustva
        ? (form.kategorijaOdsustva as AbsenceCategory)
        : null,
      capOverrideRazlog: form.capOverrideRazlog
        ? (form.capOverrideRazlog as CapOverrideReason)
        : null,
    },
  };
}

export function buildYearOptions(currentYear: number): number[] {
  const years: number[] = [];

  for (let offset = 0; offset <= YEAR_RANGE; offset += 1) {
    years.push(currentYear - offset);
  }

  return years;
}

/**
 * Today as `YYYY-MM-DD`, off the LOCAL calendar.
 *
 * Never `toISOString()`: that is a UTC date, and between 00:00 and 02:00 CEST it
 * is still yesterday — on the 1st of a month, still the previous month, which
 * may already be closed. This register's whole legal point is per-calendar-day
 * granularity, so an off-by-one default day is a data-integrity defect. The same
 * reasoning is why `formatDan` never round-trips through `Date` either.
 */
export function todayIso(): string {
  const now = new Date();
  const month = `${now.getMonth() + 1}`.padStart(2, "0");
  const day = `${now.getDate()}`.padStart(2, "0");

  return `${now.getFullYear()}-${month}-${day}`;
}

/** The first day of a period, as `YYYY-MM-DD`. */
function periodStart(godina: number, mesec: number): string {
  return `${godina}-${String(mesec).padStart(2, "0")}-01`;
}

/**
 * The last day of a period. Day 0 of the following month is the last day of this
 * one; computed in UTC so no local offset can shift the boundary.
 */
function periodEnd(godina: number, mesec: number): string {
  const last = new Date(Date.UTC(godina, mesec, 0)).getUTCDate();

  return `${godina}-${String(mesec).padStart(2, "0")}-${String(last).padStart(2, "0")}`;
}

/**
 * The day the form opens on: today when today falls inside the displayed
 * period, otherwise that period's first day. A default outside the period would
 * write a row the grid cannot show.
 */
function defaultDan(godina: number, mesec: number): string {
  const pocetak = periodStart(godina, mesec);
  const danas = todayIso();

  return danas.slice(0, 7) === pocetak.slice(0, 7) ? danas : pocetak;
}

/**
 * Formats a `YYYY-MM-DD` day off its own calendar components. Parsing to a
 * `Date` first would re-project the instant into the viewer's timezone and can
 * shift a legally-meaningful day by one.
 */
export function formatDan(value: string): string {
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

/**
 * The `cap_override_required` detail's assessment, or `null` when the error does
 * not carry one.
 *
 * `null` is the safe miss: `capNoticeFor` then falls back to the čl. 53 notice,
 * which is the ceiling that applies to every employee not in preraspodela.
 */
function capsFrom(error: unknown): WorkTimeCapAssessment | null {
  if (
    typeof error === "object" &&
    error !== null &&
    "details" in error &&
    typeof (error as { details: unknown }).details === "object" &&
    (error as { details: unknown }).details !== null
  ) {
    const details = (error as { details: Record<string, unknown> }).details;

    if (typeof details.caps === "object" && details.caps !== null) {
      return details.caps as WorkTimeCapAssessment;
    }
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

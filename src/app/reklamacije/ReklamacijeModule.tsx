import {
  InfoIcon,
  MessageSquareWarningIcon,
  PlusIcon,
  PrinterIcon,
  XIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { Spinner } from "@/components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { PosServices, ReklamacijeService } from "@/services/ports";
import type {
  DeadlineState,
  ExportedFile,
  ReklamacijaInput,
  ReklamacijaRegime,
  ReklamacijaSummary,
  ReklamacijaView,
  RobaKind,
} from "@/services/types";

interface ReklamacijeModuleProps {
  services: PosServices;
}

// Serbian status vocabulary. `open`/`answered`/`awaiting_consumer`/`resolved`
// are the stored `status` column; `impasse` is the derived-clock label the
// detail view reuses when the consumer went silent past the 3-day window.
const STATUS_LABELS: Record<string, string> = {
  open: "Otvorena",
  answered: "Odgovoreno",
  awaiting_consumer: "Čeka potrošača",
  impasse: "Zastoj",
  resolved: "Rešena",
};

// The 30-day track is `nameštaj` is legally undefined and `tehnička roba` is a
// power-driven-device test — both are the shop's call. The default is the
// shorter 15-day track: over-flagging a plain good grants unearned time (harm),
// under-flagging is safe (verified rules §84, §39).
const ROBA_KIND_LABELS: Record<RobaKind, string> = {
  opsta: "Opšta roba (15 dana)",
  tehnicka: "Tehnička roba (30 dana)",
  namestaj: "Nameštaj (30 dana)",
};

const ROBA_KIND_NOTE =
  'Razvrstavanje robe u „tehničku robu" ili „nameštaj" (rok 30 dana) je pravna procena prodavca; ostala roba ima rok od 15 dana.';

// Read-only context in the detail header: which frozen regime the record filed
// under. Never recomputed after intake (verified rules — regime pinned at
// filing date vs the cutover).
const REGIME_LABELS: Record<ReklamacijaRegime, string> = {
  old: "Stari režim",
  new: "Novi režim",
};

// The derived clock verdict. `resolutionDue` is null while paused, at impasse,
// or resolved — this label is what the detail shows in the resolution-due slot
// instead of a date.
const CLOCK_LABELS: Record<string, string> = {
  running: "Rok teče",
  paused: "Pauzirano",
  impasse: "Zastoj",
  resolved: "Rešeno",
};

// Timeline vocabulary — the five persisted event types (verified rules §2).
const EVENT_LABELS: Record<string, string> = {
  answer_given: "Odgovor poslat",
  consumer_received_answer: "Potrošač primio odgovor",
  consumer_responded: "Potrošač se izjasnio",
  extension_granted: "Rok produžen",
  resolved: "Reklamacija rešena",
};

// The memo §4(a) express-warning template (verified rules, čl. 63 st. 10), split
// into the three mandated parts: (1) obaveza izjašnjenja, (2) posledice
// propuštanja roka, (3) zastoj rokova. The law prescribes content, not exact
// wording — these are editable defaults, gated non-empty for the NEW regime.
const WARNING_DUTY_TEMPLATE =
  "Dužni ste da se na ovaj odgovor izjasnite najkasnije u roku od 3 (tri) dana od dana njegovog prijema.";
const WARNING_CONSEQUENCES_TEMPLATE =
  "Ako se u tom roku ne izjasnite, smatraće se da niste saglasni sa našim predlogom.";
const WARNING_ZASTOJ_TEMPLATE =
  "Rok za rešavanje reklamacije zastaje danom Vašeg prijema ovog odgovora i nastavlja da teče danom kada primimo Vaše izjašnjenje.";

export function ReklamacijeModule({ services }: ReklamacijeModuleProps) {
  const reklamacije = services.reklamacije;
  const printService = services.print;
  const [rows, setRows] = useState<ReklamacijaSummary[]>([]);
  const [listStatus, setListStatus] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [listError, setListError] = useState<string | undefined>();
  const [intakeOpen, setIntakeOpen] = useState(false);
  const [detail, setDetail] = useState<ReklamacijaView | null>(null);
  const [detailStatus, setDetailStatus] = useState<
    "idle" | "loading" | "ready"
  >("idle");

  useEffect(() => {
    let cancelled = false;

    setListStatus("loading");
    setListError(undefined);

    reklamacije
      .list()
      .then((result) => {
        if (!cancelled) {
          setRows(result);
          setListStatus("ready");
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setListError(errorMessage(error, "Reklamacije nisu učitane."));
          setListStatus("error");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [reklamacije]);

  async function openDetail(id: number) {
    setDetail(null);
    setDetailStatus("loading");
    try {
      const view = await reklamacije.get(id);
      setDetail(view);
      setDetailStatus("ready");
    } catch (error) {
      setDetailStatus("idle");
      toast.error("Reklamacija nije učitana", {
        description: errorMessage(error, "Detalji nisu dostupni."),
      });
    }
  }

  // The lifecycle transitions return the freshly-derived view; fold it back into
  // both the open detail and the list row so the deadline verdict stays current.
  function applyUpdate(view: ReklamacijaView) {
    setDetail(view);
    setRows((current) =>
      current.map((row) => (row.id === view.id ? summaryFromView(view) : row)),
    );
  }

  // SW-8 export-then-open: export the document, then hand its path to the OS
  // print handler. A failed open still leaves the saved file surfaced.
  async function runPrint(
    action: () => Promise<ExportedFile>,
    fallback: string,
  ) {
    let exported: ExportedFile;
    try {
      exported = await action();
    } catch (error) {
      toast.error("Izvoz nije uspeo", {
        description: errorMessage(error, fallback),
      });
      return;
    }
    try {
      await printService.openForPrint(exported.path);
      toast.success("Otvoreno za štampu", { description: exported.path });
    } catch {
      toast.warning("Dokument je sačuvan — otvorite ga ručno za štampu", {
        description: exported.path,
      });
    }
  }

  return (
    <section className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Reklamacije</h2>
          <p className="text-xs text-muted-foreground">
            Evidencija reklamacija sa rokovima za odgovor i rešavanje.
          </p>
        </div>
        <Button type="button" onClick={() => setIntakeOpen(true)}>
          <PlusIcon data-icon="inline-start" />
          Nova reklamacija
        </Button>
      </div>

      {listError ? (
        <Alert variant="destructive">
          <AlertTitle>Reklamacije nisu učitane</AlertTitle>
          <AlertDescription>{listError}</AlertDescription>
        </Alert>
      ) : null}

      {listStatus === "loading" ? (
        <Badge variant="outline">
          <Spinner data-icon="inline-start" aria-hidden="true" />
          Učitavanje reklamacija
        </Badge>
      ) : rows.length === 0 && listStatus === "ready" ? (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <MessageSquareWarningIcon aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>Nema reklamacija</EmptyTitle>
            <EmptyDescription>
              Evidentirajte reklamaciju da biste pratili zakonske rokove.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Broj</TableHead>
              <TableHead>Podnosilac</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Rok za odgovor</TableHead>
              <TableHead>Rok za rešavanje</TableHead>
              <TableHead className="text-right">Radnje</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.id}>
                <TableCell>{row.registerNumber}</TableCell>
                <TableCell>{row.podnosilacImePrezime}</TableCell>
                <TableCell>
                  <Badge variant="outline">
                    {STATUS_LABELS[row.status] ?? row.status}
                  </Badge>
                </TableCell>
                <TableCell>
                  <DeadlineDate value={row.answerDue} overdue={row.answerOverdue} />
                </TableCell>
                <TableCell>
                  <DeadlineDate
                    value={row.resolutionDue}
                    overdue={row.resolutionOverdue}
                  />
                </TableCell>
                <TableCell className="text-right">
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    aria-label={`Detalji za reklamaciju #${row.registerNumber}`}
                    onClick={() => openDetail(row.id)}
                  >
                    Detalji
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {detailStatus === "loading" ? (
        <Badge variant="outline">
          <Spinner data-icon="inline-start" aria-hidden="true" />
          Učitavanje detalja
        </Badge>
      ) : null}

      {detail ? (
        <DetailPanel
          key={detail.id}
          view={detail}
          service={reklamacije}
          onUpdated={applyUpdate}
          onClose={() => {
            setDetail(null);
            setDetailStatus("idle");
          }}
          onPrint={runPrint}
        />
      ) : null}

      <IntakeDialog
        open={intakeOpen}
        onOpenChange={setIntakeOpen}
        onCreate={(input) => reklamacije.create(input)}
        onCreated={(view) => {
          setRows((current) => [summaryFromView(view), ...current]);
          setIntakeOpen(false);
          toast.success(`Reklamacija #${view.registerNumber} je evidentirana.`);
        }}
      />
    </section>
  );
}

function DetailPanel({
  view,
  service,
  onUpdated,
  onClose,
  onPrint,
}: {
  view: ReklamacijaView;
  service: ReklamacijeService;
  onUpdated: (view: ReklamacijaView) => void;
  onClose: () => void;
  onPrint: (
    action: () => Promise<ExportedFile>,
    fallback: string,
  ) => void | Promise<void>;
}) {
  const resolved = view.deadlines.clock === "resolved";
  const answered = view.events.some(
    (event) => event.eventType === "answer_given",
  );
  const awaitingConsumer = view.status === "awaiting_consumer";
  const overdue =
    view.deadlines.answerOverdue || view.deadlines.resolutionOverdue;
  const resolution = resolutionDisplay(view.deadlines);

  // A single lifecycle transition: run the service call, fold the derived view
  // back up, and surface the outcome. The backend enforces legal ordering, so a
  // rejected transition (e.g. a premature consumer event) arrives here as an
  // error to show, never a silent no-op.
  async function runAction(
    action: () => Promise<ReklamacijaView>,
    successMessage: string,
  ): Promise<boolean> {
    try {
      const updated = await action();
      onUpdated(updated);
      toast.success(successMessage);
      return true;
    } catch (error) {
      toast.error("Radnja nije uspela", {
        description: errorMessage(error, "Pokušajte ponovo."),
      });
      return false;
    }
  }

  return (
    <section
      aria-label={`Detalji reklamacije #${view.registerNumber}`}
      className="flex flex-col gap-4 rounded-md border border-border p-4"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <h3 className="text-base font-semibold">
            Reklamacija #{view.registerNumber}
          </h3>
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="outline">
              {STATUS_LABELS[view.status] ?? view.status}
            </Badge>
            {/* Regime is read-only context — frozen at intake, never recomputed. */}
            <Badge variant="secondary">{REGIME_LABELS[view.regime]}</Badge>
          </div>
        </div>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          aria-label="Zatvori detalje"
          onClick={onClose}
        >
          <XIcon aria-hidden="true" />
        </Button>
      </div>

      <dl className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <InfoRow label="Podnosilac" value={view.podnosilacImePrezime} />
        <InfoRow label="Kontakt" value={view.kontakt ?? "—"} />
        <InfoRow label="Datum podnošenja" value={formatDate(view.filedAt)} />
        <InfoRow label="Vrsta robe" value={ROBA_KIND_LABELS[view.robaKind]} />
        <InfoRow
          className="sm:col-span-2"
          label="Podaci o robi"
          value={view.podaciORobi}
        />
        <InfoRow
          className="sm:col-span-2"
          label="Opis nesaobraznosti"
          value={view.opisNesaobraznosti}
        />
        <InfoRow
          className="sm:col-span-2"
          label="Zahtev potrošača"
          value={view.zahtev}
        />
      </dl>

      <Separator />

      <div className="flex flex-col gap-2">
        <h4 className="text-sm font-medium">Rokovi</h4>
        <dl className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <InfoRow
            label="Rok za odgovor"
            value={
              <DeadlineDate
                value={view.deadlines.answerDue}
                overdue={view.deadlines.answerOverdue}
              />
            }
          />
          <InfoRow
            label="Rok za rešavanje"
            value={
              <span
                data-overdue={resolution.overdue ? "true" : undefined}
                className={
                  resolution.overdue
                    ? "font-medium text-destructive"
                    : undefined
                }
              >
                {resolution.text}
              </span>
            }
          />
          <InfoRow
            label="Status roka"
            value={CLOCK_LABELS[view.deadlines.clock] ?? view.deadlines.clock}
          />
          {view.deadlines.consumerWindowDue ? (
            <InfoRow
              label="Rok za izjašnjenje potrošača"
              value={formatDate(view.deadlines.consumerWindowDue)}
            />
          ) : null}
        </dl>
        {overdue ? (
          // Advisory context only (verified rules §5) — never a threat, and
          // nothing here asserts the shop is or is not in violation.
          //
          // Legal: every figure and every word of the penalty comes from the
          // backend (`legal.rs::reklamacija_breach`), tier-resolved from the
          // shop's pravna forma and versioned by this record's frozen regime.
          // When the legal form is unanswered the penalty is `null` and we
          // point at Podešavanja → Profil instead of guessing a tier.
          <div className="flex flex-col gap-1 text-xs text-muted-foreground">
            <p>Informativno: {view.notice.summary}</p>
            {view.notice.penalty ? (
              <p>{view.notice.penalty}</p>
            ) : (
              <p>
                Unesite pravnu formu u Podešavanja → Profil da bi kazna bila
                prikazana.
              </p>
            )}
            <p>{view.notice.citation}</p>
            <p>Ovo je informativni podatak, a ne pravni savet.</p>
          </div>
        ) : null}
      </div>

      <Separator />

      <div className="flex flex-col gap-2">
        <h4 className="text-sm font-medium">Tok reklamacije</h4>
        {view.events.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            Još nema evidentiranih događaja.
          </p>
        ) : (
          <ol className="flex flex-col gap-2">
            {view.events.map((event, index) => (
              <li
                key={`${event.eventType}-${event.eventDate}-${index}`}
                className="flex items-baseline justify-between gap-3 text-sm"
              >
                <span>{EVENT_LABELS[event.eventType] ?? event.eventType}</span>
                <span className="text-muted-foreground">
                  {formatDate(event.eventDate)}
                </span>
              </li>
            ))}
          </ol>
        )}
      </div>

      {resolved ? null : (
        <>
          <Separator />
          <div className="flex flex-col gap-4">
            <h4 className="text-sm font-medium">Radnje</h4>

            {answered ? null : (
              <AnswerForm view={view} service={service} runAction={runAction} />
            )}

            {answered && !awaitingConsumer ? (
              <DateEventForm
                inputId="reklamacija-primio-datum"
                dateLabel="Datum kada je potrošač primio odgovor"
                buttonLabel="Potrošač primio odgovor"
                onSubmit={(eventDate) =>
                  runAction(
                    () => service.consumerReceived(view.id, eventDate),
                    "Evidentiran prijem odgovora.",
                  )
                }
              />
            ) : null}

            {awaitingConsumer ? (
              <DateEventForm
                inputId="reklamacija-izjasnio-datum"
                dateLabel="Datum izjašnjenja potrošača"
                buttonLabel="Potrošač se izjasnio"
                onSubmit={(eventDate) =>
                  runAction(
                    () => service.consumerResponded(view.id, eventDate),
                    "Evidentirano izjašnjenje potrošača.",
                  )
                }
              />
            ) : null}

            <ExtensionForm
              view={view}
              service={service}
              runAction={runAction}
            />

            <ResolveForm view={view} service={service} runAction={runAction} />
          </div>
        </>
      )}

      <Separator />

      <div className="flex flex-col gap-2">
        <h4 className="text-sm font-medium">Štampa</h4>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              onPrint(
                () => service.exportPotvrda(view.id),
                "Izvoz potvrde nije uspeo.",
              )
            }
          >
            <PrinterIcon data-icon="inline-start" />
            Štampaj potvrdu
          </Button>
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              onPrint(
                () => service.exportNotice(),
                "Izvoz obaveštenja nije uspeo.",
              )
            }
          >
            <PrinterIcon data-icon="inline-start" />
            Štampaj obaveštenje
          </Button>
        </div>
      </div>
    </section>
  );
}

function AnswerForm({
  view,
  service,
  runAction,
}: {
  view: ReklamacijaView;
  service: ReklamacijeService;
  runAction: (
    action: () => Promise<ReklamacijaView>,
    successMessage: string,
  ) => Promise<boolean>;
}) {
  const isNew = view.regime === "new";
  const [answerText, setAnswerText] = useState("");
  const [duty, setDuty] = useState(isNew ? WARNING_DUTY_TEMPLATE : "");
  const [consequences, setConsequences] = useState(
    isNew ? WARNING_CONSEQUENCES_TEMPLATE : "",
  );
  const [zastoj, setZastoj] = useState(isNew ? WARNING_ZASTOJ_TEMPLATE : "");
  const [datum, setDatum] = useState(() => today());
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(undefined);

    if (!answerText.trim()) {
      setError("Odgovor je obavezan.");
      return;
    }
    // NEW-regime gate (čl. 63 st. 10): the answer must carry the express
    // 3-part warning. The OLD regime is NOT gated — no warning required.
    if (isNew && (!duty.trim() || !consequences.trim() || !zastoj.trim())) {
      setError(
        "Za novi režim sva tri obaveštenja o roku su obavezna (čl. 63 st. 10).",
      );
      return;
    }

    setSubmitting(true);
    const ok = await runAction(
      () =>
        service.logAnswer(view.id, {
          answerText: answerText.trim(),
          warningDuty: isNew ? duty.trim() : null,
          warningConsequences: isNew ? consequences.trim() : null,
          warningZastoj: isNew ? zastoj.trim() : null,
          eventDate: toRfc3339(datum),
        }),
      "Odgovor je evidentiran.",
    );
    setSubmitting(false);
    if (ok) {
      setAnswerText("");
    }
  }

  return (
    <form
      className="flex flex-col gap-3 rounded-md border border-border p-3"
      onSubmit={handleSubmit}
      aria-label="Unos odgovora"
    >
      <FieldGroup>
        {error ? <FieldError>{error}</FieldError> : null}
        <Field>
          <FieldLabel htmlFor="reklamacija-odgovor">
            Odgovor na reklamaciju
          </FieldLabel>
          <Textarea
            id="reklamacija-odgovor"
            required
            value={answerText}
            onChange={(event) => setAnswerText(event.target.value)}
          />
        </Field>
        {isNew ? (
          <>
            <Field>
              <FieldLabel htmlFor="reklamacija-upozorenje-obaveza">
                Obaveza izjašnjenja potrošača
              </FieldLabel>
              <Textarea
                id="reklamacija-upozorenje-obaveza"
                required
                value={duty}
                onChange={(event) => setDuty(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="reklamacija-upozorenje-posledice">
                Posledice propuštanja roka
              </FieldLabel>
              <Textarea
                id="reklamacija-upozorenje-posledice"
                required
                value={consequences}
                onChange={(event) => setConsequences(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="reklamacija-upozorenje-zastoj">
                Zastoj rokova
              </FieldLabel>
              <Textarea
                id="reklamacija-upozorenje-zastoj"
                required
                value={zastoj}
                onChange={(event) => setZastoj(event.target.value)}
              />
              <FieldDescription>
                Za nove reklamacije odgovor mora izričito obavestiti potrošača o
                obavezi izjašnjenja, posledicama i zastoju rokova (čl. 63 st.
                10).
              </FieldDescription>
            </Field>
          </>
        ) : null}
        <Field>
          <FieldLabel htmlFor="reklamacija-odgovor-datum">
            Datum odgovora
          </FieldLabel>
          <Input
            id="reklamacija-odgovor-datum"
            type="date"
            value={datum}
            onChange={(event) => setDatum(event.target.value)}
          />
        </Field>
      </FieldGroup>
      <div>
        <Button type="submit" disabled={submitting}>
          {submitting ? (
            <Spinner data-icon="inline-start" aria-hidden="true" />
          ) : null}
          Unesi odgovor
        </Button>
      </div>
    </form>
  );
}

function DateEventForm({
  inputId,
  dateLabel,
  buttonLabel,
  onSubmit,
}: {
  inputId: string;
  dateLabel: string;
  buttonLabel: string;
  onSubmit: (eventDate: string) => Promise<boolean>;
}) {
  const [datum, setDatum] = useState(() => today());
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!datum) {
      return;
    }
    setSubmitting(true);
    await onSubmit(toRfc3339(datum));
    setSubmitting(false);
  }

  return (
    <form
      className="flex flex-wrap items-end gap-3 rounded-md border border-border p-3"
      onSubmit={handleSubmit}
      aria-label={buttonLabel}
    >
      <Field className="w-auto">
        <FieldLabel htmlFor={inputId}>{dateLabel}</FieldLabel>
        <Input
          id={inputId}
          type="date"
          value={datum}
          onChange={(event) => setDatum(event.target.value)}
        />
      </Field>
      <Button type="submit" variant="outline" disabled={submitting}>
        {submitting ? (
          <Spinner data-icon="inline-start" aria-hidden="true" />
        ) : null}
        {buttonLabel}
      </Button>
    </form>
  );
}

function ExtensionForm({
  view,
  service,
  runAction,
}: {
  view: ReklamacijaView;
  service: ReklamacijeService;
  runAction: (
    action: () => Promise<ReklamacijaView>,
    successMessage: string,
  ) => Promise<boolean>;
}) {
  const used = view.deadlines.oneExtensionUsed;
  const [datum, setDatum] = useState(() => today());
  const [consent, setConsent] = useState(false);
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(undefined);
    // Only one extension, only with consent (čl. 55/63 st. 11).
    if (!consent) {
      setError("Produženje roka zahteva saglasnost potrošača.");
      return;
    }
    if (!datum) {
      setError("Novi rok je obavezan.");
      return;
    }
    setSubmitting(true);
    const ok = await runAction(
      () =>
        service.grantExtension(
          view.id,
          toRfc3339(datum),
          consent,
          reason.trim(),
          toRfc3339(today()),
        ),
      "Rok je produžen.",
    );
    setSubmitting(false);
    if (ok) {
      setReason("");
      setConsent(false);
    }
  }

  return (
    <form
      className="flex flex-col gap-3 rounded-md border border-border p-3"
      onSubmit={handleSubmit}
      aria-label="Produženje roka"
    >
      <FieldGroup>
        {error ? <FieldError>{error}</FieldError> : null}
        <Field>
          <FieldLabel htmlFor="reklamacija-novi-rok">Novi rok</FieldLabel>
          <Input
            id="reklamacija-novi-rok"
            type="date"
            value={datum}
            disabled={used}
            onChange={(event) => setDatum(event.target.value)}
          />
        </Field>
        <Field orientation="horizontal">
          <Checkbox
            id="reklamacija-saglasnost"
            checked={consent}
            disabled={used}
            onCheckedChange={(checked) => setConsent(Boolean(checked))}
          />
          <FieldLabel htmlFor="reklamacija-saglasnost">
            Potrošač je saglasan sa produženjem roka
          </FieldLabel>
        </Field>
        <Field>
          <FieldLabel htmlFor="reklamacija-razlog">
            Razlog produženja
          </FieldLabel>
          <Textarea
            id="reklamacija-razlog"
            value={reason}
            disabled={used}
            onChange={(event) => setReason(event.target.value)}
          />
        </Field>
      </FieldGroup>
      <div className="flex flex-col gap-1">
        <div>
          <Button type="submit" variant="outline" disabled={used || submitting}>
            {submitting ? (
              <Spinner data-icon="inline-start" aria-hidden="true" />
            ) : null}
            Produži rok
          </Button>
        </div>
        {used ? (
          <p className="text-xs text-muted-foreground">
            Rok je već jednom produžen — zakon dozvoljava samo jedno produženje
            (čl. 55/63 st. 11).
          </p>
        ) : null}
      </div>
    </form>
  );
}

function ResolveForm({
  view,
  service,
  runAction,
}: {
  view: ReklamacijaView;
  service: ReklamacijeService;
  runAction: (
    action: () => Promise<ReklamacijaView>,
    successMessage: string,
  ) => Promise<boolean>;
}) {
  const [nacin, setNacin] = useState("");
  const [datum, setDatum] = useState(() => today());
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(undefined);
    if (!nacin.trim()) {
      setError("Način rešavanja je obavezan.");
      return;
    }
    setSubmitting(true);
    const ok = await runAction(
      () => service.resolve(view.id, nacin.trim(), toRfc3339(datum)),
      "Reklamacija je rešena.",
    );
    setSubmitting(false);
    if (ok) {
      setNacin("");
    }
  }

  return (
    <form
      className="flex flex-col gap-3 rounded-md border border-border p-3"
      onSubmit={handleSubmit}
      aria-label="Rešavanje reklamacije"
    >
      <FieldGroup>
        {error ? <FieldError>{error}</FieldError> : null}
        <Field>
          <FieldLabel htmlFor="reklamacija-nacin">Način rešavanja</FieldLabel>
          <Textarea
            id="reklamacija-nacin"
            required
            value={nacin}
            onChange={(event) => setNacin(event.target.value)}
          />
        </Field>
        <Field>
          <FieldLabel htmlFor="reklamacija-resenje-datum">
            Datum rešavanja
          </FieldLabel>
          <Input
            id="reklamacija-resenje-datum"
            type="date"
            value={datum}
            onChange={(event) => setDatum(event.target.value)}
          />
        </Field>
      </FieldGroup>
      <div>
        <Button type="submit" disabled={submitting}>
          {submitting ? (
            <Spinner data-icon="inline-start" aria-hidden="true" />
          ) : null}
          Reši reklamaciju
        </Button>
      </div>
    </form>
  );
}

function InfoRow({
  label,
  value,
  className,
}: {
  label: string;
  value: ReactNode;
  className?: string;
}) {
  return (
    <div className={className}>
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="whitespace-pre-wrap text-sm">{value}</dd>
    </div>
  );
}

function resolutionDisplay(deadlines: DeadlineState): {
  text: string;
  overdue: boolean;
} {
  // A concrete resolution date only exists while the clock runs; when it is
  // null the record is paused/at impasse/resolved and we show the clock label.
  if (deadlines.resolutionDue) {
    return {
      text: formatDate(deadlines.resolutionDue),
      overdue: deadlines.resolutionOverdue,
    };
  }
  return { text: CLOCK_LABELS[deadlines.clock] ?? "—", overdue: false };
}

function DeadlineDate({
  value,
  overdue,
}: {
  value: string | null;
  overdue: boolean;
}) {
  if (value == null) {
    return <span className="text-muted-foreground">—</span>;
  }

  return (
    <span
      data-overdue={overdue ? "true" : undefined}
      className={overdue ? "font-medium text-destructive" : undefined}
    >
      {formatDate(value)}
    </span>
  );
}

function IntakeDialog({
  open,
  onOpenChange,
  onCreate,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreate: (input: ReklamacijaInput) => Promise<ReklamacijaView>;
  onCreated: (view: ReklamacijaView) => void;
}) {
  const [podnosilac, setPodnosilac] = useState("");
  const [kontakt, setKontakt] = useState("");
  const [podaciORobi, setPodaciORobi] = useState("");
  const [opis, setOpis] = useState("");
  const [zahtev, setZahtev] = useState("");
  const [robaKind, setRobaKind] = useState<RobaKind>("opsta");
  const [datum, setDatum] = useState(() => today());
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  // Mounting the dialog fresh each open resets the form, so the effect only has
  // to restore the two derived defaults and clear transient submit state.
  useEffect(() => {
    if (!open) {
      return;
    }
    setPodnosilac("");
    setKontakt("");
    setPodaciORobi("");
    setOpis("");
    setZahtev("");
    setRobaKind("opsta");
    setDatum(today());
    setError(undefined);
    setSubmitting(false);
  }, [open]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(undefined);

    if (!podnosilac.trim()) {
      setError("Ime i prezime podnosioca je obavezno.");
      return;
    }
    if (!podaciORobi.trim()) {
      setError("Podaci o robi su obavezni.");
      return;
    }
    if (!opis.trim()) {
      setError("Opis nesaobraznosti je obavezan.");
      return;
    }
    if (!zahtev.trim()) {
      setError("Zahtev potrošača je obavezan.");
      return;
    }
    if (!datum) {
      setError("Datum podnošenja je obavezan.");
      return;
    }

    setSubmitting(true);
    try {
      const view = await onCreate({
        podnosilacImePrezime: podnosilac.trim(),
        kontakt: kontakt.trim() || null,
        podaciORobi: podaciORobi.trim(),
        opisNesaobraznosti: opis.trim(),
        zahtev: zahtev.trim(),
        robaKind,
        filedAt: toRfc3339(datum),
      });
      onCreated(view);
    } catch (createError) {
      setError(errorMessage(createError, "Reklamacija nije evidentirana."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Nova reklamacija</DialogTitle>
          <DialogDescription>
            Podaci o podnosiocu čuvaju se radi zakonske obaveze vođenja
            evidencije.
          </DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <FieldGroup>
            {error ? <FieldError>{error}</FieldError> : null}
            <Field>
              <FieldLabel htmlFor="reklamacija-podnosilac">
                Ime i prezime podnosioca
              </FieldLabel>
              <Input
                id="reklamacija-podnosilac"
                value={podnosilac}
                onChange={(event) => setPodnosilac(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="reklamacija-kontakt">Kontakt</FieldLabel>
              <Input
                id="reklamacija-kontakt"
                value={kontakt}
                onChange={(event) => setKontakt(event.target.value)}
              />
              <FieldDescription>
                Telefon ili e-adresa za obaveštavanje. Nije obavezno.
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="reklamacija-roba">Podaci o robi</FieldLabel>
              <Textarea
                id="reklamacija-roba"
                value={podaciORobi}
                onChange={(event) => setPodaciORobi(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="reklamacija-opis">
                Opis nesaobraznosti
              </FieldLabel>
              <Textarea
                id="reklamacija-opis"
                value={opis}
                onChange={(event) => setOpis(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="reklamacija-zahtev">
                Zahtev potrošača
              </FieldLabel>
              <Textarea
                id="reklamacija-zahtev"
                value={zahtev}
                onChange={(event) => setZahtev(event.target.value)}
              />
            </Field>
            <Field>
              <div className="flex items-center gap-1.5">
                <FieldLabel htmlFor="reklamacija-roba-kind">
                  Vrsta robe
                </FieldLabel>
                <Tooltip>
                  <TooltipTrigger
                    type="button"
                    aria-label={ROBA_KIND_NOTE}
                    className="text-muted-foreground"
                  >
                    <InfoIcon className="size-4" aria-hidden="true" />
                  </TooltipTrigger>
                  <TooltipContent>{ROBA_KIND_NOTE}</TooltipContent>
                </Tooltip>
              </div>
              <NativeSelect
                id="reklamacija-roba-kind"
                value={robaKind}
                className="w-full"
                onChange={(event) =>
                  setRobaKind(event.target.value as RobaKind)
                }
              >
                <NativeSelectOption value="opsta">
                  {ROBA_KIND_LABELS.opsta}
                </NativeSelectOption>
                <NativeSelectOption value="tehnicka">
                  {ROBA_KIND_LABELS.tehnicka}
                </NativeSelectOption>
                <NativeSelectOption value="namestaj">
                  {ROBA_KIND_LABELS.namestaj}
                </NativeSelectOption>
              </NativeSelect>
              <FieldDescription>{ROBA_KIND_NOTE}</FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="reklamacija-datum">
                Datum podnošenja
              </FieldLabel>
              <Input
                id="reklamacija-datum"
                type="date"
                value={datum}
                onChange={(event) => setDatum(event.target.value)}
              />
              <FieldDescription>
                Datum podnošenja određuje pravni režim i pokreće rokove.
              </FieldDescription>
            </Field>
          </FieldGroup>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Odustani
            </Button>
            <Button type="submit" disabled={submitting}>
              {submitting ? (
                <Spinner data-icon="inline-start" aria-hidden="true" />
              ) : null}
              Evidentiraj reklamaciju
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function summaryFromView(view: ReklamacijaView): ReklamacijaSummary {
  return {
    id: view.id,
    registerNumber: view.registerNumber,
    regime: view.regime,
    status: view.status,
    podnosilacImePrezime: view.podnosilacImePrezime,
    filedAt: view.filedAt,
    answerDue: view.deadlines.answerDue,
    resolutionDue: view.deadlines.resolutionDue,
    answerOverdue: view.deadlines.answerOverdue,
    resolutionOverdue: view.deadlines.resolutionOverdue,
    purgeEligible: view.purgeEligible,
  };
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

/**
 * `<input type="date">` speaks `YYYY-MM-DD`; the command speaks RFC3339. The
 * statute counts calendar days, so midnight UTC carries no meaning of its own —
 * it is only the format `parse_rfc3339` accepts. Its calendar date fixes the
 * regime and starts the clocks.
 */
function toRfc3339(date: string): string {
  return date ? `${date}T00:00:00Z` : "";
}

/**
 * Formats the calendar-date component straight off the RFC3339 string. Parsing
 * to a `Date` first would re-project the instant into the viewer's timezone and
 * can shift a legally-meaningful deadline by a day.
 */
function formatDate(value: string): string {
  const [year, month, day] = value.slice(0, 10).split("-");

  if (!year || !month || !day) {
    return value;
  }

  return `${day}.${month}.${year}.`;
}

function errorMessage(error: unknown, fallback: string): string {
  if (isCommandError(error)) {
    return error.message;
  }

  if (
    error instanceof Error &&
    error.message &&
    !error.message.includes("Error:")
  ) {
    return error.message;
  }

  return fallback;
}

function isCommandError(error: unknown): error is { message: string } {
  return (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  );
}

import {
  InfoIcon,
  MessageSquareWarningIcon,
  PlusIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
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
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
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
import type { PosServices } from "@/services/ports";
import type {
  ReklamacijaInput,
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

export function ReklamacijeModule({ services }: ReklamacijeModuleProps) {
  const reklamacije = services.reklamacije;
  const [rows, setRows] = useState<ReklamacijaSummary[]>([]);
  const [listStatus, setListStatus] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [listError, setListError] = useState<string | undefined>();
  const [intakeOpen, setIntakeOpen] = useState(false);

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
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

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

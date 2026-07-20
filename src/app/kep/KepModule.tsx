import { AlertCircleIcon, BookIcon } from "lucide-react";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { KepService, PosServices } from "@/services/ports";
import type { KepLedger, KepStatus } from "@/services/types";

interface KepModuleProps {
  services: PosServices;
}

// How many prior book-years the selector offers alongside the current one. The
// ledger partitions strictly by book_year (SW-9a has no multi-store scope), so
// a small backward range covers the practical selection without guessing which
// years actually hold entries — the backend simply returns an empty ledger for
// a year with none.
const BOOK_YEAR_RANGE = 5;

export function KepModule({ services }: KepModuleProps) {
  const kep = services.kep;
  const [bookYear, setBookYear] = useState(() => new Date().getFullYear());
  const [ledger, setLedger] = useState<KepLedger | null>(null);
  const [ledgerStatus, setLedgerStatus] = useState<
    "loading" | "ready" | "error"
  >("loading");
  const [ledgerError, setLedgerError] = useState<string | undefined>();
  const [status, setStatus] = useState<KepStatus | null>(null);
  // Bumped after a successful posting to re-derive both the ledger and the
  // overdue/unbooked status from the append-only source of truth.
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    let cancelled = false;

    setLedgerStatus("loading");
    setLedgerError(undefined);

    kep
      .ledger(bookYear)
      .then((result) => {
        if (!cancelled) {
          setLedger(result);
          setLedgerStatus("ready");
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setLedgerError(errorMessage(error, "KEP nije učitan."));
          setLedgerStatus("error");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [kep, bookYear, reloadToken]);

  useEffect(() => {
    let cancelled = false;

    kep
      .status()
      .then((result) => {
        if (!cancelled) {
          setStatus(result);
        }
      })
      .catch(() => {
        // The status banner is advisory; a failed read must not block the
        // ledger, so it degrades silently to no warnings.
        if (!cancelled) {
          setStatus(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [kep, reloadToken]);

  const yearOptions = buildYearOptions(new Date().getFullYear());

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Knjiga evidencije prometa</h2>
          <p className="text-xs text-muted-foreground">
            Zaduženje se automatski knjiži iz prijema robe po maloprodajnoj
            vrednosti sa PDV-om; razduženje po danu prometa. Evidencija je
            hronološka i ne menja se (čl. 14).
          </p>
        </div>
        <Field className="w-auto">
          <FieldLabel htmlFor="kep-book-year">Poslovna godina</FieldLabel>
          <NativeSelect
            id="kep-book-year"
            value={String(bookYear)}
            onChange={(event) => setBookYear(Number(event.target.value))}
          >
            {yearOptions.map((year) => (
              <NativeSelectOption key={year} value={String(year)}>
                {year}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
      </div>

      <KepStatusWarnings status={status} />

      <DailyPostingForm
        kep={kep}
        onPosted={() => setReloadToken((token) => token + 1)}
      />

      {ledgerError ? (
        <Alert variant="destructive">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>KEP nije učitan</AlertTitle>
          <AlertDescription>{ledgerError}</AlertDescription>
        </Alert>
      ) : null}

      {ledgerStatus === "loading" ? (
        <Badge variant="outline">
          <Spinner data-icon="inline-start" aria-hidden="true" />
          Učitavanje evidencije
        </Badge>
      ) : ledger && ledger.entries.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <BookIcon aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>Nema knjiženja</EmptyTitle>
            <EmptyDescription>
              Za izabranu poslovnu godinu nema stavki u knjizi.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : ledger ? (
        <>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Red. br.</TableHead>
                  <TableHead>Datum</TableHead>
                  <TableHead>Opis</TableHead>
                  <TableHead className="text-right">Zaduženje</TableHead>
                  <TableHead className="text-right">Razduženje</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {ledger.entries.map((entry) => (
                  <TableRow key={entry.redniBroj}>
                    <TableCell>{entry.redniBroj}</TableCell>
                    <TableCell>{entry.datum}</TableCell>
                    <TableCell>{entry.opis}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      <AmountCell minor={entry.zaduzenjeMinor} />
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      <AmountCell minor={entry.razduzenjeMinor} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
          <div className="flex items-center justify-between gap-3 rounded-md border p-3">
            <span className="text-sm font-medium">Saldo</span>
            <span className="text-base font-semibold tabular-nums">
              {formatRsd(ledger.saldoMinor)}
            </span>
          </div>
        </>
      ) : null}
    </section>
  );
}

function KepStatusWarnings({ status }: { status: KepStatus | null }) {
  if (!status) {
    return null;
  }

  const hasOverdue = status.overdueSalesDays.length > 0;
  const hasUnbooked = status.unbookedReceiptCount > 0;

  if (!hasOverdue && !hasUnbooked) {
    return null;
  }

  return (
    <Alert variant="destructive">
      <AlertCircleIcon aria-hidden="true" />
      <AlertTitle>Neproknjižene stavke</AlertTitle>
      <AlertDescription>
        <div className="flex flex-col gap-1">
          {hasOverdue ? (
            <span>
              Nije proknjižen dnevni promet za:{" "}
              {status.overdueSalesDays.map(formatDay).join(", ")}
            </span>
          ) : null}
          {hasUnbooked ? (
            <span>
              Neproknjižen prijem robe: {status.unbookedReceiptCount}
            </span>
          ) : null}
        </div>
      </AlertDescription>
    </Alert>
  );
}

function DailyPostingForm({
  kep,
  onPosted,
}: {
  kep: KepService;
  onPosted: () => void;
}) {
  const [datum, setDatum] = useState(() => today());
  const [override, setOverride] = useState("");
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(undefined);

    if (!datum) {
      setError("Datum prometa je obavezan.");
      return;
    }

    // The override is optional: empty leaves the auto sales-total path; a value
    // records that the amount was set manually (entry_source='manual').
    let overrideMinor: number | null = null;
    if (override.trim()) {
      try {
        overrideMinor = parseRsdInput(override);
        if (overrideMinor < 0) {
          throw new Error("Iznos nije ispravan.");
        }
      } catch (parseError) {
        setError(errorMessage(parseError, "Iznos nije ispravan."));
        return;
      }
    }

    setSubmitting(true);
    try {
      await kep.postDailySales(toRfc3339(datum), overrideMinor);
      setOverride("");
      onPosted();
      toast.success("Dnevni promet je proknjižen.");
    } catch (postError) {
      setError(errorMessage(postError, "Dnevni promet nije proknjižen."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form
      className="flex flex-col gap-3 rounded-md border border-border p-3"
      onSubmit={handleSubmit}
      aria-label="Knjiženje dnevnog prometa"
    >
      <div className="flex flex-col gap-1">
        <h3 className="text-sm font-medium">Proknjiži dnevni promet</h3>
        <p className="text-xs text-muted-foreground">
          Razduženje se knjiži po danu prometa iz dnevnog izveštaja prodaje.
        </p>
      </div>
      <FieldGroup>
        {error ? <FieldError>{error}</FieldError> : null}
        <div className="flex flex-wrap items-end gap-3">
          <Field className="w-auto">
            <FieldLabel htmlFor="kep-daily-date">Datum prometa</FieldLabel>
            <Input
              id="kep-daily-date"
              type="date"
              value={datum}
              onChange={(event) => setDatum(event.target.value)}
            />
          </Field>
          <Field className="w-auto">
            <FieldLabel htmlFor="kep-daily-override">Ručni iznos</FieldLabel>
            <Input
              id="kep-daily-override"
              inputMode="decimal"
              value={override}
              onChange={(event) => setOverride(event.target.value)}
            />
            <FieldDescription>
              Opciono — unesite iznos sa fiskalnog dnevnog izveštaja ako se
              razlikuje.
            </FieldDescription>
          </Field>
          <Button type="submit" disabled={submitting}>
            {submitting ? (
              <Spinner data-icon="inline-start" aria-hidden="true" />
            ) : null}
            Proknjiži dnevni promet
          </Button>
        </div>
      </FieldGroup>
      <Separator />
      <p className="text-xs text-muted-foreground">
        Knjiženja se ne menjaju i ne brišu (čl. 14). Ispravke se evidentiraju
        posebnim knjiženjem.
      </p>
    </form>
  );
}

function AmountCell({ minor }: { minor: number | null }) {
  if (minor == null) {
    return <span className="text-muted-foreground">—</span>;
  }

  return <span>{formatRsd(minor)}</span>;
}

function buildYearOptions(currentYear: number): number[] {
  const years: number[] = [];
  for (let offset = 0; offset <= BOOK_YEAR_RANGE; offset += 1) {
    years.push(currentYear - offset);
  }
  return years;
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

/**
 * `<input type="date">` speaks `YYYY-MM-DD`; the command speaks RFC3339. The
 * daily razduženje keys idempotency on this calendar day (`document_date`), so
 * midnight UTC is only the format `parse_rfc3339` accepts — the date component
 * is what fixes the sales day.
 */
function toRfc3339(date: string): string {
  return date ? `${date}T00:00:00Z` : "";
}

/**
 * Formats a `YYYY-MM-DD` sales day straight off its calendar components. Parsing
 * to a `Date` first would re-project the instant into the viewer's timezone and
 * can shift a legally-meaningful day by one.
 */
function formatDay(value: string): string {
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

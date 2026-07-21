import { AlertCircleIcon, BookIcon, LockIcon, PrinterIcon } from "lucide-react";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { toast } from "sonner";

import { formatQuantity, parseQuantityInput } from "@/app/format";
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
import type {
  CatalogService,
  KepService,
  PosServices,
} from "@/services/ports";
import type {
  BasisDoc,
  ExportedFile,
  KalkulacijaSummary,
  KepClosePreview,
  KepClosureView,
  KepEntryView,
  KepLedger,
  KepStatus,
  ProductSummary,
  StornoCauseId,
} from "@/services/types";

interface KepModuleProps {
  services: PosServices;
}

// How many prior book-years the selector offers alongside the current one. The
// ledger partitions strictly by book_year (SW-9a has no multi-store scope), so
// a small backward range covers the practical selection without guessing which
// years actually hold entries — the backend simply returns an empty ledger for
// a year with none.
const BOOK_YEAR_RANGE = 5;

// The exact phrase the operator must type to close a year (mirrors
// `crate::kep_close::CLOSE_CONFIRMATION`). The backend re-validates it; this
// guard is only to prevent an accidental irreversible close.
const CLOSE_CONFIRMATION_PHRASE = "ZAKLJUČI KNJIGU";

export function KepModule({ services }: KepModuleProps) {
  const kep = services.kep;
  const [bookYear, setBookYear] = useState(() => new Date().getFullYear());
  const [ledger, setLedger] = useState<KepLedger | null>(null);
  const [ledgerStatus, setLedgerStatus] = useState<
    "loading" | "ready" | "error"
  >("loading");
  const [ledgerError, setLedgerError] = useState<string | undefined>();
  const [status, setStatus] = useState<KepStatus | null>(null);
  const [kalkulacije, setKalkulacije] = useState<KalkulacijaSummary[]>([]);
  // The ledger row currently targeted by „Ispravi stavku"; drives the
  // reversing-storno dialog. `null` closes it.
  const [correctionTarget, setCorrectionTarget] = useState<KepEntryView | null>(
    null,
  );
  // The year-end close preview (krajnji saldo + whether the year is already
  // closed) and the recorded closures list. `alreadyClosed` gates every
  // posting/adjustment control (čl. 18) and shows the closed-year badge.
  const [closePreview, setClosePreview] = useState<KepClosePreview | null>(null);
  const [closures, setClosures] = useState<KepClosureView[]>([]);
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);
  // Bumped after a successful posting to re-derive the ledger, the
  // overdue/unbooked status, and the kalkulacije list from the append-only
  // source of truth.
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

  useEffect(() => {
    let cancelled = false;

    kep
      .listKalkulacije(bookYear)
      .then((result) => {
        if (!cancelled) {
          setKalkulacije(result);
        }
      })
      .catch(() => {
        // The kalkulacija list is a print convenience; a failed read degrades
        // to an empty section rather than blocking the ledger.
        if (!cancelled) {
          setKalkulacije([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [kep, bookYear, reloadToken]);

  useEffect(() => {
    let cancelled = false;

    kep
      .closePreview(bookYear)
      .then((result) => {
        if (!cancelled) {
          setClosePreview(result);
        }
      })
      .catch(() => {
        // A failed preview must not block the ledger; it degrades to „open"
        // (no badge, controls enabled) — the backend gate is authoritative.
        if (!cancelled) {
          setClosePreview(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [kep, bookYear, reloadToken]);

  useEffect(() => {
    let cancelled = false;

    kep
      .listClosures()
      .then((result) => {
        if (!cancelled) {
          setClosures(result);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setClosures([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [kep, reloadToken]);

  const alreadyClosed = closePreview?.alreadyClosed ?? false;

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
      await services.print.openForPrint(exported.path);
      toast.success("Otvoreno za štampu", { description: exported.path });
    } catch {
      toast.warning("Dokument je sačuvan — otvorite ga ručno za štampu", {
        description: exported.path,
      });
    }
  }

  const yearOptions = buildYearOptions(new Date().getFullYear());

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <h2 className="text-lg font-semibold">Knjiga evidencije prometa</h2>
            {alreadyClosed ? (
              <Badge variant="secondary">
                <LockIcon data-icon="inline-start" aria-hidden="true" />
                Zaključena
              </Badge>
            ) : null}
          </div>
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

      {/* A closed year is frozen (čl. 18) — every posting/adjustment control is
          hidden, mirroring the backend `ensure_year_open` gate. */}
      {alreadyClosed ? null : (
        <>
          <DailyPostingForm
            kep={kep}
            onPosted={() => setReloadToken((token) => token + 1)}
          />

          <AdjustmentForm
            kep={kep}
            catalog={services.catalog}
            onPosted={() => setReloadToken((token) => token + 1)}
          />
        </>
      )}

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
      ) : ledger &&
        ledger.entries.length === 0 &&
        ledger.openingSaldoMinor === 0 ? (
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
                  <TableHead className="text-right">Radnje</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {/* The opening carry-in (§2): the prior year's krajnji saldo,
                    shown as the leading donos. Positive balances sit on the
                    zaduženje side (they add to the running saldo). */}
                {ledger.openingSaldoMinor !== 0 ? (
                  <TableRow className="font-medium">
                    <TableCell />
                    <TableCell />
                    <TableCell>Početno stanje (donos)</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {ledger.openingSaldoMinor > 0 ? (
                        formatRsd(ledger.openingSaldoMinor)
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {ledger.openingSaldoMinor < 0 ? (
                        formatRsd(-ledger.openingSaldoMinor)
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </TableCell>
                    <TableCell />
                  </TableRow>
                ) : null}
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
                    <TableCell className="text-right">
                      {alreadyClosed ? null : (
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          aria-label={`Ispravi stavku RB ${entry.redniBroj}`}
                          onClick={() => setCorrectionTarget(entry)}
                        >
                          Ispravi stavku
                        </Button>
                      )}
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

      <div className="flex flex-col gap-3 rounded-md border border-border p-3">
        <div className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">Zaključivanje godine</h3>
          <p className="text-xs text-muted-foreground">
            Zaključenjem se knjiga za poslovnu godinu nepovratno zaključava (čl.
            18). Krajnji saldo se prenosi kao početno stanje naredne godine.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {alreadyClosed ? null : (
            <Button type="button" onClick={() => setCloseDialogOpen(true)}>
              <LockIcon data-icon="inline-start" aria-hidden="true" />
              Zaključi godinu
            </Button>
          )}
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              runPrint(
                () => kep.exportClose(bookYear),
                "Zaključenje nije izvezeno.",
              )
            }
          >
            <PrinterIcon data-icon="inline-start" aria-hidden="true" />
            Štampaj zaključenje
          </Button>
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              runPrint(() => kep.exportBook(bookYear), "Knjiga nije izvezena.")
            }
          >
            <PrinterIcon data-icon="inline-start" aria-hidden="true" />
            Štampaj celu knjigu
          </Button>
        </div>
      </div>

      <Separator />

      <KalkulacijeSection
        items={kalkulacije}
        onPrint={(id) =>
          runPrint(() => kep.exportKalkulacija(id), "Kalkulacija nije izvezena.")
        }
      />

      <Separator />

      <ClosuresSection closures={closures} />

      <CorrectionDialog
        kep={kep}
        bookYear={bookYear}
        target={correctionTarget}
        onClose={() => setCorrectionTarget(null)}
        onCorrected={() => setReloadToken((token) => token + 1)}
      />

      <CloseYearDialog
        kep={kep}
        bookYear={bookYear}
        preview={closePreview}
        open={closeDialogOpen}
        onClose={() => setCloseDialogOpen(false)}
        onClosed={() => setReloadToken((token) => token + 1)}
      />
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

// The frontend mirror of the backend cause→{kolona, sign} hard map (see
// `crate::kep_storno::posting_for` / design §3), used ONLY to explain to the
// shop where a posting books. The backend remains authoritative — this text
// never decides the booking. Nivelacija naviše/naniže both route to the
// price-changing `nivelacija` method; the seven others route to
// `postAdjustment`, which fixes the kolona and sign backend-side.
type AdjustmentCauseId = StornoCauseId | "nivelacija_up" | "nivelacija_down";

interface CauseMeta {
  id: AdjustmentCauseId;
  label: string;
  kolona: 4 | 5;
  /** Booked as a negative crveni-storno amount (subtracts from the column). */
  storno: boolean;
  /** Routes to `nivelacija` (changes the product price) instead of `postAdjustment`. */
  isNivelacija: boolean;
}

const ADJUSTMENT_CAUSES: readonly CauseMeta[] = [
  {
    id: "nivelacija_up",
    label: "Nivelacija naviše",
    kolona: 4,
    storno: false,
    isNivelacija: true,
  },
  {
    id: "nivelacija_down",
    label: "Nivelacija naniže",
    kolona: 4,
    storno: true,
    isNivelacija: true,
  },
  {
    id: "supplier_return",
    label: "Povraćaj dobavljaču",
    kolona: 4,
    storno: true,
    isNivelacija: false,
  },
  {
    id: "customer_return",
    label: "Povraćaj kupca (raskid ugovora)",
    kolona: 5,
    storno: true,
    isNivelacija: false,
  },
  {
    id: "otpis",
    label: "Otpis",
    kolona: 4,
    storno: true,
    isNivelacija: false,
  },
  {
    id: "manjak_odluka",
    label: "Manjak po odluci",
    kolona: 4,
    storno: true,
    isNivelacija: false,
  },
  {
    id: "rashod",
    label: "Rashod",
    kolona: 4,
    storno: true,
    isNivelacija: false,
  },
  {
    id: "popis_visak",
    label: "Višak po popisu",
    kolona: 4,
    storno: false,
    isNivelacija: false,
  },
  {
    id: "popis_manjak",
    label: "Manjak po popisu",
    kolona: 5,
    storno: false,
    isNivelacija: false,
  },
];

const CAUSE_BY_ID: Record<AdjustmentCauseId, CauseMeta> = Object.fromEntries(
  ADJUSTMENT_CAUSES.map((cause) => [cause.id, cause]),
) as Record<AdjustmentCauseId, CauseMeta>;

const EMPTY_BASIS: BasisDoc = { naziv: "", broj: "", datum: "" };

const PRODUCT_SEARCH_LIMIT = 20;
const SEARCH_DEBOUNCE_MS = 150;

function kolonaExplanation(meta: CauseMeta): string {
  const kolonaWord = meta.kolona === 4 ? "zaduženje" : "razduženje";
  const base = `Knjiži se u kolonu ${meta.kolona} kao ${kolonaWord}`;
  return meta.storno ? `${base} (crveni storno).` : `${base}.`;
}

// Integer half-up division matching the backend `round_div` (b > 0, a >= 0
// here). Kept in integer minor units — never floats.
function roundDiv(a: number, b: number): number {
  return Math.floor((a + Math.floor(b / 2)) / b);
}

// The signed effect on the derived saldo (Σkolona4 − Σkolona5). A storno stores
// a negative amount; a kolona-5 posting subtracts from the saldo.
function saldoSign(meta: CauseMeta): number {
  const stored = meta.storno ? -1 : 1;
  const columnSign = meta.kolona === 4 ? 1 : -1;
  return stored * columnSign;
}

function computeSaldoDelta(
  meta: CauseMeta | undefined,
  product: ProductSummary | null,
  quantity: string,
  newPrice: string,
): number | null {
  if (!meta || !product) {
    return null;
  }

  if (meta.isNivelacija) {
    let priceMinor: number;
    try {
      priceMinor = parseRsdInput(newPrice);
    } catch {
      return null;
    }
    const diff = priceMinor - product.salePriceMinor;
    const magnitude = roundDiv(product.currentStockMilli * Math.abs(diff), 1000);
    return diff >= 0 ? magnitude : -magnitude;
  }

  let quantityMilli: number;
  try {
    quantityMilli = parseQuantityInput(quantity);
  } catch {
    return null;
  }
  const value = roundDiv(quantityMilli * product.salePriceMinor, 1000);
  return saldoSign(meta) * value;
}

function AdjustmentForm({
  kep,
  catalog,
  onPosted,
}: {
  kep: KepService;
  catalog: CatalogService;
  onPosted: () => void;
}) {
  const [causeId, setCauseId] = useState<AdjustmentCauseId | "">("");
  const [search, setSearch] = useState("");
  const [results, setResults] = useState<ProductSummary[]>([]);
  const [selected, setSelected] = useState<ProductSummary | null>(null);
  const [quantity, setQuantity] = useState("");
  const [newPrice, setNewPrice] = useState("");
  const [basis, setBasis] = useState<BasisDoc>(EMPTY_BASIS);
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  const meta = causeId ? CAUSE_BY_ID[causeId] : undefined;

  useEffect(() => {
    // Only search once a cause is chosen — an unopened form fetches nothing.
    if (!causeId) {
      return;
    }

    let cancelled = false;
    const term = search.trim();
    const timer = setTimeout(() => {
      catalog
        .searchProducts({
          search: term || undefined,
          active: true,
          limit: PRODUCT_SEARCH_LIMIT,
        })
        .then((result) => {
          if (!cancelled) {
            setResults(result.items);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setResults([]);
          }
        });
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [catalog, search, causeId]);

  function resetForm() {
    setCauseId("");
    setSearch("");
    setResults([]);
    setSelected(null);
    setQuantity("");
    setNewPrice("");
    setBasis(EMPTY_BASIS);
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(undefined);

    if (!meta) {
      setError("Izaberite vrstu izmene.");
      return;
    }
    if (!selected) {
      setError("Izaberite artikal.");
      return;
    }

    const naziv = basis.naziv.trim();
    const broj = basis.broj.trim();
    const datum = basis.datum.trim();
    if (!naziv || !broj || !datum) {
      setError("Popunite dokument osnova (naziv, broj i datum).");
      return;
    }
    const doc: BasisDoc = { naziv, broj, datum };

    let action: () => Promise<void>;
    if (meta.isNivelacija) {
      let priceMinor: number;
      try {
        priceMinor = parseRsdInput(newPrice);
      } catch (parseError) {
        setError(errorMessage(parseError, "Nova cena nije ispravna."));
        return;
      }
      action = () => kep.nivelacija(selected.id, priceMinor, doc);
    } else {
      let quantityMilli: number;
      try {
        quantityMilli = parseQuantityInput(quantity);
      } catch (parseError) {
        setError(errorMessage(parseError, "Količina nije ispravna."));
        return;
      }
      action = () =>
        kep.postAdjustment(meta.id as StornoCauseId, selected.id, quantityMilli, doc);
    }

    setSubmitting(true);
    try {
      await action();
      resetForm();
      onPosted();
      toast.success("Izmena je proknjižena.");
    } catch (postError) {
      setError(errorMessage(postError, "Izmena nije proknjižena."));
    } finally {
      setSubmitting(false);
    }
  }

  const saldoDelta = computeSaldoDelta(meta, selected, quantity, newPrice);

  return (
    <form
      className="flex flex-col gap-3 rounded-md border border-border p-3"
      onSubmit={handleSubmit}
      aria-label="Nova izmena"
    >
      <div className="flex flex-col gap-1">
        <h3 className="text-sm font-medium">Nova izmena</h3>
        <p className="text-xs text-muted-foreground">
          Vrsta izmene određuje kolonu i predznak knjiženja — to nije izbor
          korisnika. Nivelacija menja i prodajnu cenu artikla.
        </p>
      </div>
      <FieldGroup>
        {error ? <FieldError>{error}</FieldError> : null}
        <Field className="w-auto">
          <FieldLabel htmlFor="kep-adjustment-cause">Vrsta izmene</FieldLabel>
          <NativeSelect
            id="kep-adjustment-cause"
            value={causeId}
            onChange={(event) => {
              setCauseId(event.target.value as AdjustmentCauseId | "");
              setError(undefined);
            }}
          >
            <NativeSelectOption value="">— izaberite —</NativeSelectOption>
            {ADJUSTMENT_CAUSES.map((cause) => (
              <NativeSelectOption key={cause.id} value={cause.id}>
                {cause.label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>

        {meta ? (
          <>
            <p className="rounded-md bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
              {kolonaExplanation(meta)}
            </p>

            <Field className="w-auto">
              <FieldLabel htmlFor="kep-adjustment-product">Artikal</FieldLabel>
              <Input
                id="kep-adjustment-product"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Pretraga po nazivu ili šifri"
              />
            </Field>

            {selected ? (
              <div className="flex items-center justify-between gap-3 rounded-md border p-2 text-xs">
                <span>
                  Izabrano: {selected.name} —{" "}
                  {formatRsd(selected.salePriceMinor)}/{selected.unitOfMeasure}
                </span>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => setSelected(null)}
                >
                  Promeni
                </Button>
              </div>
            ) : results.length > 0 ? (
              <ul className="flex flex-col gap-1">
                {results.map((product) => (
                  <li key={product.id}>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      className="w-full justify-between"
                      aria-label={`Izaberi ${product.name}`}
                      onClick={() => {
                        setSelected(product);
                        setResults([]);
                        setSearch("");
                      }}
                    >
                      <span>Izaberi {product.name}</span>
                      <span className="text-muted-foreground">
                        {product.sku}
                      </span>
                    </Button>
                  </li>
                ))}
              </ul>
            ) : null}

            {meta.isNivelacija ? (
              <Field className="w-auto">
                <FieldLabel htmlFor="kep-adjustment-price">
                  Nova prodajna cena
                </FieldLabel>
                <Input
                  id="kep-adjustment-price"
                  inputMode="decimal"
                  value={newPrice}
                  onChange={(event) => setNewPrice(event.target.value)}
                />
                <FieldDescription>
                  Nova maloprodajna cena sa PDV-om po jedinici mere.
                </FieldDescription>
              </Field>
            ) : (
              <Field className="w-auto">
                <FieldLabel htmlFor="kep-adjustment-qty">Količina</FieldLabel>
                <Input
                  id="kep-adjustment-qty"
                  inputMode="decimal"
                  value={quantity}
                  onChange={(event) => setQuantity(event.target.value)}
                />
              </Field>
            )}

            <BasisFields
              idPrefix="kep-adj-basis"
              value={basis}
              onChange={setBasis}
            />

            {saldoDelta != null ? (
              <p className="text-xs text-muted-foreground">
                Efekat na saldo:{" "}
                <span className="font-medium tabular-nums">
                  {formatRsd(saldoDelta)}
                </span>
              </p>
            ) : null}

            <Button type="submit" className="w-auto" disabled={submitting}>
              {submitting ? (
                <Spinner data-icon="inline-start" aria-hidden="true" />
              ) : null}
              Proknjiži izmenu
            </Button>
          </>
        ) : null}
      </FieldGroup>
    </form>
  );
}

function CorrectionDialog({
  kep,
  bookYear,
  target,
  onClose,
  onCorrected,
}: {
  kep: KepService;
  bookYear: number;
  target: KepEntryView | null;
  onClose: () => void;
  onCorrected: () => void;
}) {
  const [amount, setAmount] = useState("");
  const [basis, setBasis] = useState<BasisDoc>(EMPTY_BASIS);
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  // Reset the form each time a different ledger row is targeted.
  useEffect(() => {
    setAmount("");
    setBasis(EMPTY_BASIS);
    setError(undefined);
  }, [target]);

  async function handleConfirm() {
    setError(undefined);
    if (!target) {
      return;
    }

    let amountMinor: number;
    try {
      amountMinor = parseRsdInput(amount);
    } catch (parseError) {
      setError(errorMessage(parseError, "Iznos nije ispravan."));
      return;
    }

    const naziv = basis.naziv.trim();
    const broj = basis.broj.trim();
    const datum = basis.datum.trim();
    if (!naziv || !broj || !datum) {
      setError("Popunite dokument osnova (naziv, broj i datum).");
      return;
    }

    setSubmitting(true);
    try {
      await kep.correctEntry(target.redniBroj, bookYear, amountMinor, {
        naziv,
        broj,
        datum,
      });
      onCorrected();
      onClose();
      toast.success("Ispravka je proknjižena.");
    } catch (correctError) {
      setError(errorMessage(correctError, "Ispravka nije proknjižena."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog
      open={target != null}
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Ispravi stavku</DialogTitle>
          <DialogDescription>
            Ispravka se knjiži kao storno originalne stavke i novo knjiženje sa
            tekućim datumom. Originalna stavka ostaje nepromenjena (čl. 14).
          </DialogDescription>
        </DialogHeader>
        <FieldGroup>
          {error ? <FieldError>{error}</FieldError> : null}
          {target ? (
            <p className="text-xs text-muted-foreground">
              Stavka RB {target.redniBroj} — {target.opis}
            </p>
          ) : null}
          <Field>
            <FieldLabel htmlFor="kep-correction-amount">
              Ispravan iznos
            </FieldLabel>
            <Input
              id="kep-correction-amount"
              inputMode="decimal"
              value={amount}
              onChange={(event) => setAmount(event.target.value)}
            />
            <FieldDescription>
              Tačan iznos koji je trebalo proknjižiti u istoj koloni.
            </FieldDescription>
          </Field>
          <BasisFields
            idPrefix="kep-corr-basis"
            value={basis}
            onChange={setBasis}
          />
        </FieldGroup>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={onClose}
            disabled={submitting}
          >
            Odustani
          </Button>
          <Button type="button" onClick={handleConfirm} disabled={submitting}>
            {submitting ? (
              <Spinner data-icon="inline-start" aria-hidden="true" />
            ) : null}
            Sačuvaj ispravku
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function CloseYearDialog({
  kep,
  bookYear,
  preview,
  open,
  onClose,
  onClosed,
}: {
  kep: KepService;
  bookYear: number;
  preview: KepClosePreview | null;
  open: boolean;
  onClose: () => void;
  onClosed: () => void;
}) {
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  // Clear the typed phrase and any error each time the dialog opens or closes,
  // so a reopened dialog never starts pre-confirmed.
  useEffect(() => {
    setConfirmation("");
    setError(undefined);
  }, [open]);

  async function handleConfirm() {
    setError(undefined);
    setSubmitting(true);
    try {
      await kep.closeYear(bookYear, confirmation);
      onClosed();
      onClose();
      toast.success("Knjiga je zaključena.");
    } catch (closeError) {
      setError(errorMessage(closeError, "Zaključenje nije uspelo."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          onClose();
        }
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Zaključivanje knjige za {bookYear}</DialogTitle>
          <DialogDescription>
            Zaključenje je nepovratno. Krajnji saldo se prenosi kao početno
            stanje naredne poslovne godine.
          </DialogDescription>
        </DialogHeader>
        <FieldGroup>
          {error ? <FieldError>{error}</FieldError> : null}
          <div className="flex flex-col gap-1 rounded-md border p-3 text-sm">
            <div className="flex items-center justify-between gap-3">
              <span className="text-muted-foreground">Krajnji saldo</span>
              <span className="font-semibold tabular-nums">
                {preview ? formatRsd(preview.krajnjiSaldoMinor) : "—"}
              </span>
            </div>
            <div className="flex items-center justify-between gap-3">
              <span className="text-muted-foreground">Broj stavki</span>
              <span className="tabular-nums">{preview?.entryCount ?? 0}</span>
            </div>
          </div>
          <Field>
            <FieldLabel htmlFor="kep-close-confirmation">Potvrda</FieldLabel>
            <Input
              id="kep-close-confirmation"
              value={confirmation}
              onChange={(event) => setConfirmation(event.target.value)}
              autoComplete="off"
            />
            <FieldDescription>
              Za potvrdu ukucajte tačno: ZAKLJUČI KNJIGU.
            </FieldDescription>
          </Field>
        </FieldGroup>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={onClose}
            disabled={submitting}
          >
            Odustani
          </Button>
          <Button
            type="button"
            onClick={handleConfirm}
            disabled={submitting || confirmation !== CLOSE_CONFIRMATION_PHRASE}
          >
            {submitting ? (
              <Spinner data-icon="inline-start" aria-hidden="true" />
            ) : null}
            Potvrdi zaključenje
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ClosuresSection({ closures }: { closures: KepClosureView[] }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-1">
        <h3 className="text-sm font-medium">Zaključene godine</h3>
        <p className="text-xs text-muted-foreground">
          Zaključena knjiga se čuva najmanje 5 godina. Ništa se ne briše
          automatski — arhiviranje je odluka trgovca.
        </p>
      </div>
      {closures.length === 0 ? (
        <p className="text-xs text-muted-foreground">
          Nema zaključenih godina.
        </p>
      ) : (
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Poslovna godina</TableHead>
                <TableHead className="text-right">Krajnji saldo</TableHead>
                <TableHead>Datum zaključenja</TableHead>
                <TableHead>Čuvanje</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {closures.map((closure) => (
                <TableRow key={closure.bookYear}>
                  <TableCell>{closure.bookYear}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatRsd(closure.krajnjiSaldoMinor)}
                  </TableCell>
                  <TableCell>{formatDay(closure.closedAt)}</TableCell>
                  <TableCell>
                    {closure.purgeEligible ? (
                      <Badge variant="outline">Može se arhivirati</Badge>
                    ) : (
                      <span className="text-xs text-muted-foreground">
                        Čuva se do {closure.bookYear + 5}.
                      </span>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

function KalkulacijeSection({
  items,
  onPrint,
}: {
  items: KalkulacijaSummary[];
  onPrint: (id: number) => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-1">
        <h3 className="text-sm font-medium">Kalkulacije</h3>
        <p className="text-xs text-muted-foreground">
          Kalkulacija cene se generiše iz prijema robe. Razlika u ceni (marža)
          se izvodi unazad iz maloprodajne cene i može biti negativna.
        </p>
      </div>
      {items.length === 0 ? (
        <p className="text-xs text-muted-foreground">
          Nema kalkulacija za izabranu poslovnu godinu.
        </p>
      ) : (
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Red. br.</TableHead>
                <TableHead>Trgovački naziv</TableHead>
                <TableHead className="text-right">Količina</TableHead>
                <TableHead className="text-right">
                  Razlika u ceni (marža)
                </TableHead>
                <TableHead className="text-right">
                  Prodajna vrednost sa PDV
                </TableHead>
                <TableHead className="text-right">Radnje</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {items.map((item) => (
                <TableRow key={item.id}>
                  <TableCell>{item.redniBroj}</TableCell>
                  <TableCell>{item.trgovackiNaziv}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatQuantity(item.kolicinaMilli)}
                  </TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatRsd(item.razlikaUCeniMinor)}
                  </TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatRsd(item.prodajnaVrednostSaPdvMinor)}
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      aria-label={`Štampaj kalkulaciju RB ${item.redniBroj}`}
                      onClick={() => onPrint(item.id)}
                    >
                      <PrinterIcon data-icon="inline-start" aria-hidden="true" />
                      Štampaj kalkulaciju
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

function BasisFields({
  idPrefix,
  value,
  onChange,
}: {
  idPrefix: string;
  value: BasisDoc;
  onChange: (value: BasisDoc) => void;
}) {
  return (
    <div className="flex flex-wrap items-end gap-3">
      <Field className="w-auto">
        <FieldLabel htmlFor={`${idPrefix}-naziv`}>Naziv dokumenta</FieldLabel>
        <Input
          id={`${idPrefix}-naziv`}
          value={value.naziv}
          onChange={(event) => onChange({ ...value, naziv: event.target.value })}
        />
      </Field>
      <Field className="w-auto">
        <FieldLabel htmlFor={`${idPrefix}-broj`}>Broj dokumenta</FieldLabel>
        <Input
          id={`${idPrefix}-broj`}
          value={value.broj}
          onChange={(event) => onChange({ ...value, broj: event.target.value })}
        />
      </Field>
      <Field className="w-auto">
        <FieldLabel htmlFor={`${idPrefix}-datum`}>Datum dokumenta</FieldLabel>
        <Input
          id={`${idPrefix}-datum`}
          type="date"
          value={value.datum}
          onChange={(event) => onChange({ ...value, datum: event.target.value })}
        />
      </Field>
    </div>
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

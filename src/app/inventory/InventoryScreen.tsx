import {
  ArchiveIcon,
  ClipboardListIcon,
  PackageMinusIcon,
  PackagePlusIcon,
  PencilIcon,
  SearchIcon,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";
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
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
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
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { declarationFieldList } from "@/lib/deklaracija";
import type { PosServices } from "@/services/ports";
import type {
  CommandErrorShape,
  DeclarationWarning,
  Dobavljac,
  InventoryAdjustmentRequest,
  IspravaWarning,
  PrimljenaIsprava,
  ProductLedger,
  StockListItem,
  StockListQuery,
  StockStateFilter,
} from "@/services/types";

interface InventoryScreenProps {
  services: PosServices;
  initialLedgerProductId?: number | null;
  onLedgerOpened?: () => void;
}

type AdjustmentMode = "receive" | "correction" | "write_off";

interface AdjustmentState {
  mode: AdjustmentMode;
  item: StockListItem;
}

const STOCK_FILTERS: Array<{
  value: StockStateFilter;
  label: string;
}> = [
  { value: "all", label: "Sve" },
  { value: "low", label: "Nizak" },
  { value: "zero", label: "Nula" },
  { value: "negative", label: "Negativno" },
];

export function InventoryScreen({
  services,
  initialLedgerProductId = null,
  onLedgerOpened,
}: InventoryScreenProps) {
  const [query, setQuery] = useState<StockListQuery>({ stockState: "all" });
  const [items, setItems] = useState<StockListItem[]>([]);
  const [allItems, setAllItems] = useState<StockListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [adjustment, setAdjustment] = useState<AdjustmentState | null>(null);
  const [ledger, setLedger] = useState<ProductLedger | null>(null);
  const [ledgerOpen, setLedgerOpen] = useState(false);
  const [ledgerLoading, setLedgerLoading] = useState(false);

  const categories = useMemo(() => {
    const categoryMap = new Map<number, string>();
    for (const item of allItems) {
      if (item.categoryId && item.categoryName) {
        categoryMap.set(item.categoryId, item.categoryName);
      }
    }
    return [...categoryMap.entries()].map(([id, name]) => ({ id, name }));
  }, [allItems]);

  async function refreshStock(nextQuery = query) {
    setLoading(true);
    setError(null);

    try {
      const [stock, completeStock] = await Promise.all([
        services.inventory.listStock(nextQuery),
        services.inventory.listStock({ stockState: "all" }),
      ]);
      setItems(stock.items);
      setAllItems(completeStock.items);
    } catch (unknownError) {
      setError(getCommandMessage(unknownError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refreshStock(query);
  }, [query]);

  async function openLedger(productId: number) {
    setLedgerOpen(true);
    setLedgerLoading(true);

    try {
      setLedger(await services.inventory.getProductLedger(productId));
    } catch (unknownError) {
      toast.error(getCommandMessage(unknownError));
    } finally {
      setLedgerLoading(false);
    }
  }

  useEffect(() => {
    if (initialLedgerProductId == null) {
      return;
    }

    void openLedger(initialLedgerProductId);
    onLedgerOpened?.();
  }, [initialLedgerProductId]);

  async function handleAdjustmentSaved(productId: number) {
    await refreshStock();
    if (ledgerOpen && ledger?.productId === productId) {
      setLedger(await services.inventory.getProductLedger(productId));
    }
  }

  function updateQuery(nextQuery: StockListQuery) {
    setQuery({
      ...nextQuery,
      search: nextQuery.search?.trim() ? nextQuery.search : undefined,
      stockState: nextQuery.stockState ?? "all",
    });
  }

  return (
    <div className="flex flex-1 flex-col gap-4 p-4">
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold">Stanje lagera</h2>
        <p className="text-xs text-muted-foreground">
          Prijem, korekcije, otpis i kartica artikla za lokalnu bazu.
        </p>
      </div>

      <div className="flex flex-col gap-3 lg:flex-row lg:items-end">
        <Field className="lg:max-w-sm">
          <FieldLabel htmlFor="inventory-search">Pretraga</FieldLabel>
          <InputGroup>
            <InputGroupAddon>
              <SearchIcon aria-hidden="true" />
            </InputGroupAddon>
            <InputGroupInput
              id="inventory-search"
              value={query.search ?? ""}
              placeholder="Naziv, SKU ili barkod"
              onChange={(event) =>
                updateQuery({ ...query, search: event.target.value })
              }
            />
          </InputGroup>
        </Field>

        <Field>
          <FieldLabel htmlFor="inventory-category">Kategorija</FieldLabel>
          <NativeSelect
            id="inventory-category"
            value={query.categoryId?.toString() ?? "all"}
            onChange={(event) =>
              updateQuery({
                ...query,
                categoryId:
                  event.target.value === "all"
                    ? null
                    : Number(event.target.value),
              })
            }
          >
            <NativeSelectOption value="all">Sve kategorije</NativeSelectOption>
            {categories.map((category) => (
              <NativeSelectOption key={category.id} value={category.id}>
                {category.name}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>

        <Field>
          <FieldLabel id="stock-state-filter">Stanje</FieldLabel>
          <ToggleGroup
            value={[query.stockState ?? "all"]}
            onValueChange={(value) => {
              const nextValue = (value as StockStateFilter[])[0] ?? "all";
              updateQuery({ ...query, stockState: nextValue });
            }}
            aria-labelledby="stock-state-filter"
            variant="outline"
            size="sm"
            spacing={1}
          >
            {STOCK_FILTERS.map((filter) => (
              <ToggleGroupItem key={filter.value} value={filter.value}>
                {filter.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </Field>
      </div>

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Lager nije učitan</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {loading ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-28 w-full" />
        </div>
      ) : items.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ArchiveIcon aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>Nema artikala za izabrani filter</EmptyTitle>
            <EmptyDescription>
              Promenite pretragu ili stanje lagera.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <StockTable
          items={items}
          onAdjust={(mode, item) => setAdjustment({ mode, item })}
          onLedger={openLedger}
        />
      )}

      <InventoryAdjustmentDialog
        adjustment={adjustment}
        services={services}
        onOpenChange={(open) => {
          if (!open) {
            setAdjustment(null);
          }
        }}
        onSaved={handleAdjustmentSaved}
      />

      <Sheet open={ledgerOpen} onOpenChange={setLedgerOpen}>
        <SheetContent className="w-full sm:max-w-xl">
          <SheetHeader>
            <SheetTitle>Kartica artikla</SheetTitle>
            <SheetDescription>
              Hronologija promena zaliha za izabrani artikal.
            </SheetDescription>
          </SheetHeader>
          <div className="flex flex-1 flex-col gap-4 overflow-auto px-6 pb-6">
            {ledgerLoading ? (
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <Spinner />
                Učitavanje kartice
              </div>
            ) : ledger ? (
              <ProductLedgerView ledger={ledger} />
            ) : (
              <Empty>
                <EmptyHeader>
                  <EmptyTitle>Kartica nije dostupna</EmptyTitle>
                  <EmptyDescription>
                    Izaberite artikal iz lager liste.
                  </EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
          </div>
        </SheetContent>
      </Sheet>
    </div>
  );
}

function StockTable({
  items,
  onAdjust,
  onLedger,
}: {
  items: StockListItem[];
  onAdjust: (mode: AdjustmentMode, item: StockListItem) => void;
  onLedger: (productId: number) => void;
}) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Artikal</TableHead>
          <TableHead>SKU</TableHead>
          <TableHead>Barkod</TableHead>
          <TableHead>Stanje</TableHead>
          <TableHead>Minimum</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Poslednja promena</TableHead>
          <TableHead>Akcije</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {items.map((item) => (
          <TableRow key={item.productId}>
            <TableCell>
              <span className="font-medium">{item.productName}</span>
            </TableCell>
            <TableCell>{item.sku}</TableCell>
            <TableCell>{item.barcode ?? "-"}</TableCell>
            <TableCell>
              {formatQuantity(item.currentQuantityMilli, item.unitOfMeasure)}
            </TableCell>
            <TableCell>
              {formatQuantity(item.minimumStockMilli, item.unitOfMeasure)}
            </TableCell>
            <TableCell>
              <StockBadge item={item} />
            </TableCell>
            <TableCell>{formatDate(item.lastMovementAt)}</TableCell>
            <TableCell>
              <div className="flex flex-wrap gap-1">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  aria-label={`Prijem robe za ${item.productName}`}
                  onClick={() => onAdjust("receive", item)}
                >
                  <PackagePlusIcon data-icon="inline-start" />
                  Prijem
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  aria-label={`Korekcija za ${item.productName}`}
                  onClick={() => onAdjust("correction", item)}
                >
                  <PencilIcon data-icon="inline-start" />
                  Korekcija
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  aria-label={`Otpis za ${item.productName}`}
                  onClick={() => onAdjust("write_off", item)}
                >
                  <PackageMinusIcon data-icon="inline-start" />
                  Otpis
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  aria-label={`Ledger za ${item.productName}`}
                  onClick={() => onLedger(item.productId)}
                >
                  <ClipboardListIcon data-icon="inline-start" />
                  Ledger
                </Button>
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function StockBadge({ item }: { item: StockListItem }) {
  if (item.currentQuantityMilli < 0) {
    return <Badge variant="destructive">Negativan lager</Badge>;
  }

  if (item.currentQuantityMilli === 0) {
    return <Badge variant="destructive">Nema zaliha</Badge>;
  }

  if (item.lowStock) {
    return <Badge variant="secondary">Nizak lager</Badge>;
  }

  return <Badge variant="outline">U redu</Badge>;
}

/** The picker value that means „create a new isprava from the fields below“. */
const NEW_ISPRAVA = "nova";

/**
 * The six kinds `crate::dobavljaci::VrstaIsprave` names, in the order
 * KEP-VERIFIED-RULES §3 gives them. **Offered, never enforced** — §3's list ends
 * in „ili druga odgovarajuća isprava za robu“, and neither the column nor the
 * parser refuses a kind outside this list.
 */
const VRSTE_ISPRAVE = [
  "faktura",
  "otpremnica",
  "faktura-otpremnica",
  "dostavnica",
  "interna prenosnica",
  "prijemnica",
] as const;

const EMPTY_NOVA_ISPRAVA = {
  dobavljacId: "",
  vrsta: "otpremnica",
  broj: "",
  datum: "",
  posedujeIspravu: false,
};

/**
 * A dinar amount typed by the operator → integer minor units (para). Never
 * floating point: the digits are read as written and scaled by string, so
 * `1234,5` is 123.450 para exactly rather than whatever a binary double makes
 * of it. Returns `null` for a blank input, meaning „do not send it“.
 */
function parsePriceInput(value: string): number | null {
  const trimmed = value.trim().replace(/\s/g, "");
  if (!trimmed) {
    return null;
  }
  const normalized = trimmed.replace(",", ".");
  if (!/^\d+(\.\d{0,2})?$/.test(normalized)) {
    throw new Error("Nabavna cena nije ispravna.");
  }
  const [whole, fraction = ""] = normalized.split(".");
  return Number(whole) * 100 + Number(fraction.padEnd(2, "0"));
}

function InventoryAdjustmentDialog({
  adjustment,
  services,
  onOpenChange,
  onSaved,
}: {
  adjustment: AdjustmentState | null;
  services: PosServices;
  onOpenChange: (open: boolean) => void;
  onSaved: (productId: number) => Promise<void>;
}) {
  const [quantity, setQuantity] = useState("1");
  const [reason, setReason] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Raised only *after* the movement is committed, so this state can never gate
  // the form — it replaces it (§3 req 25: warn, never hard-block).
  const [declarationWarnings, setDeclarationWarnings] = useState<
    DeclarationWarning[]
  >([]);
  const [markingChecked, setMarkingChecked] = useState(false);

  // ZoT čl. 29 st. 1 — the supplier's isprava o nabavci. Receive mode only: a
  // correction or a write-off is not a nabavka and has no dobavljač.
  const isReceive = adjustment?.mode === "receive";
  const [dobavljaci, setDobavljaci] = useState<Dobavljac[]>([]);
  const [isprave, setIsprave] = useState<PrimljenaIsprava[]>([]);
  const [ispravaChoice, setIspravaChoice] = useState<string>("");
  const [novaIsprava, setNovaIsprava] = useState(EMPTY_NOVA_ISPRAVA);
  const [nabavnaCena, setNabavnaCena] = useState("");
  const [ispravaWarning, setIspravaWarning] = useState<IspravaWarning | null>(
    null,
  );

  useEffect(() => {
    if (adjustment) {
      setQuantity("1");
      setReason("");
      setError(null);
      setDeclarationWarnings([]);
      setMarkingChecked(false);
      setIspravaChoice("");
      setNovaIsprava(EMPTY_NOVA_ISPRAVA);
      setNabavnaCena("");
      setIspravaWarning(null);
    }
  }, [adjustment]);

  // Loaded when the receive dialog opens. A failure here must not stop the
  // operator taking in stock: the pickers stay empty, the receipt still books,
  // and the čl. 29 advisory afterwards is what says the document is missing.
  useEffect(() => {
    if (!isReceive) {
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const [loadedDobavljaci, loadedIsprave] = await Promise.all([
          services.inventory.listDobavljaci(),
          services.inventory.listIsprave(50),
        ]);
        if (!cancelled) {
          setDobavljaci(loadedDobavljaci);
          setIsprave(loadedIsprave);
        }
      } catch {
        if (!cancelled) {
          setDobavljaci([]);
          setIsprave([]);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isReceive, services]);

  // Clearing on close, not on the next open: the effect above runs *after*
  // paint, so a dialog reopened for another article would render one frame
  // carrying the previous article's čl. 34 warning. The effect stays as
  // belt-and-braces for state arriving any other way.
  function handleOpenChange(open: boolean) {
    if (!open) {
      setDeclarationWarnings([]);
      setMarkingChecked(false);
      setIspravaWarning(null);
    }

    onOpenChange(open);
  }

  const copy = adjustment ? adjustmentCopy(adjustment.mode) : null;

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!adjustment || !copy) {
      return;
    }

    setSubmitting(true);
    setError(null);

    try {
      const quantityMilli = parseQuantityInput(quantity);
      const request: InventoryAdjustmentRequest = {
        productId: adjustment.item.productId,
        quantityMilli,
        reason,
      };

      if (adjustment.mode === "receive") {
        // A brand-new isprava is created first, so the receipt links to a row
        // that already exists. If this throws, nothing has been received yet —
        // the operator fixes the document and submits again.
        if (ispravaChoice === NEW_ISPRAVA) {
          request.ispravaId = await services.inventory.createIsprava({
            dobavljacId: Number(novaIsprava.dobavljacId),
            vrsta: novaIsprava.vrsta,
            broj: novaIsprava.broj,
            datum: novaIsprava.datum,
            posedujeIspravu: novaIsprava.posedujeIspravu,
          });
        } else if (ispravaChoice) {
          request.ispravaId = Number(ispravaChoice);
        }

        const parsedNabavna = parsePriceInput(nabavnaCena);
        if (parsedNabavna !== null) {
          request.purchasePriceMinor = parsedNabavna;
        }
      }

      const result =
        adjustment.mode === "receive"
          ? await services.inventory.receiveStock(request)
          : adjustment.mode === "correction"
            ? await services.inventory.correctStock(request)
            : await services.inventory.writeOffStock(request);

      toast.success(copy.success);
      const warnings = result.declarationWarnings ?? [];

      // The stock movement is already in the ledger either way. A warning keeps
      // the dialog open only so the operator can read it and stamp the check.
      if (warnings.length > 0) {
        setDeclarationWarnings(warnings);
      } else if (result.ispravaWarning) {
        setIspravaWarning(result.ispravaWarning);
      } else {
        handleOpenChange(false);
      }

      await onSaved(adjustment.item.productId);
    } catch (unknownError) {
      setError(getCommandMessage(unknownError));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleMarkChecked() {
    if (!adjustment) {
      return;
    }

    setMarkingChecked(true);

    try {
      await services.inventory.markDeclarationChecked(
        adjustment.item.productId,
      );
      toast.success("Provera deklaracije je evidentirana.");
      handleOpenChange(false);
    } catch (unknownError) {
      toast.error(getCommandMessage(unknownError));
    } finally {
      setMarkingChecked(false);
    }
  }

  return (
    <Dialog open={Boolean(adjustment)} onOpenChange={handleOpenChange}>
      <DialogContent>
        {declarationWarnings.length > 0 ? (
          <DeclarationWarningPanel
            warnings={declarationWarnings}
            marking={markingChecked}
            onMarkChecked={handleMarkChecked}
            onClose={() => handleOpenChange(false)}
          />
        ) : ispravaWarning ? (
          <IspravaWarningPanel
            warning={ispravaWarning}
            onClose={() => handleOpenChange(false)}
          />
        ) : copy && adjustment ? (
          <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
            <DialogHeader>
              <DialogTitle>{copy.title}</DialogTitle>
              <DialogDescription>
                {adjustment.item.productName} trenutno ima{" "}
                {formatQuantity(
                  adjustment.item.currentQuantityMilli,
                  adjustment.item.unitOfMeasure,
                )}
                .
              </DialogDescription>
            </DialogHeader>

            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="inventory-adjustment-quantity">
                  Količina
                </FieldLabel>
                <Input
                  id="inventory-adjustment-quantity"
                  inputMode="decimal"
                  value={quantity}
                  onChange={(event) => setQuantity(event.target.value)}
                  aria-invalid={error ? true : undefined}
                />
                <FieldDescription>
                  Unesite količinu u osnovnoj jedinici artikla.
                </FieldDescription>
              </Field>
              <Field>
                <FieldLabel htmlFor="inventory-adjustment-reason">
                  Razlog
                </FieldLabel>
                <Textarea
                  id="inventory-adjustment-reason"
                  value={reason}
                  onChange={(event) => setReason(event.target.value)}
                  aria-invalid={error ? true : undefined}
                />
              </Field>

              {adjustment.mode === "receive" ? (
                <>
                  <Field>
                    <FieldLabel htmlFor="inventory-adjustment-nabavna">
                      Nabavna cena po jedinici mere
                    </FieldLabel>
                    <Input
                      id="inventory-adjustment-nabavna"
                      inputMode="decimal"
                      value={nabavnaCena}
                      onChange={(event) => setNabavnaCena(event.target.value)}
                      placeholder="npr. 100,00"
                    />
                    <FieldDescription>
                      Cena sa ove isporuke. Ako je unesete, ažurira se i nabavna
                      cena artikla u šifarniku.
                    </FieldDescription>
                  </Field>

                  <Field>
                    <FieldLabel htmlFor="inventory-adjustment-isprava">
                      Isprava dobavljača
                    </FieldLabel>
                    <NativeSelect
                      id="inventory-adjustment-isprava"
                      value={ispravaChoice}
                      onChange={(event) =>
                        setIspravaChoice(event.target.value)
                      }
                    >
                      <NativeSelectOption value="">
                        Bez isprave
                      </NativeSelectOption>
                      {isprave.map((isprava) => (
                        <NativeSelectOption
                          key={isprava.id}
                          value={String(isprava.id)}
                        >
                          {`${isprava.vrsta} br. ${isprava.broj} od ${isprava.datum} — ${isprava.dobavljacPoslovnoIme}`}
                        </NativeSelectOption>
                      ))}
                      <NativeSelectOption value={NEW_ISPRAVA}>
                        Nova isprava…
                      </NativeSelectOption>
                    </NativeSelect>
                    <FieldDescription>
                      Aplikacija evidentira podatke o ispravi — samu ispravu
                      (papir ili PDF) ne čuva i ne zamenjuje je.
                    </FieldDescription>
                  </Field>

                  {ispravaChoice === NEW_ISPRAVA ? (
                    <>
                      <Field>
                        <FieldLabel htmlFor="inventory-isprava-dobavljac">
                          Dobavljač
                        </FieldLabel>
                        <NativeSelect
                          id="inventory-isprava-dobavljac"
                          value={novaIsprava.dobavljacId}
                          onChange={(event) =>
                            setNovaIsprava((previous) => ({
                              ...previous,
                              dobavljacId: event.target.value,
                            }))
                          }
                        >
                          <NativeSelectOption value="">
                            Izaberite dobavljača
                          </NativeSelectOption>
                          {dobavljaci.map((dobavljac) => (
                            <NativeSelectOption
                              key={dobavljac.id}
                              value={String(dobavljac.id)}
                            >
                              {dobavljac.poslovnoIme}
                            </NativeSelectOption>
                          ))}
                        </NativeSelect>
                      </Field>

                      <Field>
                        <FieldLabel htmlFor="inventory-isprava-vrsta">
                          Vrsta isprave
                        </FieldLabel>
                        <Input
                          id="inventory-isprava-vrsta"
                          list="inventory-isprava-vrste"
                          value={novaIsprava.vrsta}
                          onChange={(event) =>
                            setNovaIsprava((previous) => ({
                              ...previous,
                              vrsta: event.target.value,
                            }))
                          }
                        />
                        <datalist id="inventory-isprava-vrste">
                          {VRSTE_ISPRAVE.map((vrsta) => (
                            <option key={vrsta} value={vrsta} />
                          ))}
                        </datalist>
                        <FieldDescription>
                          Ponuđene vrste su predlog — možete upisati i drugu
                          odgovarajuću ispravu za robu.
                        </FieldDescription>
                      </Field>

                      <Field>
                        <FieldLabel htmlFor="inventory-isprava-broj">
                          Broj isprave
                        </FieldLabel>
                        <Input
                          id="inventory-isprava-broj"
                          value={novaIsprava.broj}
                          onChange={(event) =>
                            setNovaIsprava((previous) => ({
                              ...previous,
                              broj: event.target.value,
                            }))
                          }
                        />
                      </Field>

                      <Field>
                        <FieldLabel htmlFor="inventory-isprava-datum">
                          Datum isprave
                        </FieldLabel>
                        <Input
                          id="inventory-isprava-datum"
                          type="date"
                          value={novaIsprava.datum}
                          onChange={(event) =>
                            setNovaIsprava((previous) => ({
                              ...previous,
                              datum: event.target.value,
                            }))
                          }
                        />
                        <FieldDescription>
                          Datum sa same isprave, a ne datum knjiženja.
                        </FieldDescription>
                      </Field>

                      <Field orientation="horizontal">
                        <Checkbox
                          id="inventory-isprava-poseduje"
                          checked={novaIsprava.posedujeIspravu}
                          onCheckedChange={(checked) =>
                            setNovaIsprava((previous) => ({
                              ...previous,
                              posedujeIspravu: checked === true,
                            }))
                          }
                        />
                        <FieldLabel htmlFor="inventory-isprava-poseduje">
                          Ispravu posedujem u dokumentaciji
                        </FieldLabel>
                      </Field>
                    </>
                  ) : null}
                </>
              ) : null}

              {error ? <FieldError>{error}</FieldError> : null}
            </FieldGroup>

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => handleOpenChange(false)}
              >
                Odustani
              </Button>
              <Button type="submit" disabled={submitting}>
                {submitting ? <Spinner data-icon="inline-start" /> : copy.icon}
                {copy.submit}
              </Button>
            </DialogFooter>
          </form>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

/**
 * ZoT čl. 34. The receipt is already written when this renders — the title says
 * so first, because a warning that reads like a rejection sends the operator
 * looking for a movement that is in fact in the ledger.
 *
 * The advisory sentence is printed **with** the notice and never without it
 * (§3 req 26): the notice alone reads as a proven offence, when all the shop
 * actually knows is that its own records are blank. The stamp offered here is
 * the čl. 69a tač. 4 mitigating circumstance — it records who looked at the
 * physical label and when, and it is not a defence.
 */
function DeclarationWarningPanel({
  warnings,
  marking,
  onMarkChecked,
  onClose,
}: {
  warnings: DeclarationWarning[];
  marking: boolean;
  onMarkChecked: () => void;
  onClose: () => void;
}) {
  return (
    <div className="flex flex-col gap-4">
      <DialogHeader>
        <DialogTitle>Prijem je upisan — nedostaju podaci deklaracije</DialogTitle>
        <DialogDescription>
          Prijem robe je evidentiran u lageru. Podaci sa deklaracije se unose u
          kartici artikla.
        </DialogDescription>
      </DialogHeader>

      {warnings.map((warning) => (
        <Alert key={warning.productId}>
          <AlertTitle>{warning.productName}</AlertTitle>
          <AlertDescription>
            <div className="flex flex-col gap-2">
              <p>Nedostaje: {declarationFieldList(warning.missingFields)}</p>
              <p>{warning.advisory}</p>
              <p>{warning.notice.summary}</p>
              {warning.notice.penalty ? (
                <p>{warning.notice.penalty}</p>
              ) : (
                <p>
                  Unesite pravnu formu u Podešavanja → Profil da bi kazna bila
                  prikazana.
                </p>
              )}
              <p className="text-xs">{warning.notice.citation}</p>
            </div>
          </AlertDescription>
        </Alert>
      ))}

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          Zatvori
        </Button>
        <Button type="button" disabled={marking} onClick={onMarkChecked}>
          {marking ? <Spinner data-icon="inline-start" /> : null}
          Evidentiraj proveru deklaracije
        </Button>
      </DialogFooter>
    </div>
  );
}

/**
 * ZoT čl. 29 st. 1. Like the čl. 34 panel, the title says the receipt is already
 * written first — a warning that reads like a rejection sends the operator
 * looking for a movement that is in fact in the ledger.
 *
 * The advisory sentence is printed **with** the notice and never without it: the
 * notice alone reads as a proven offence, when all the shop actually knows is
 * that this receipt has no document linked to it. The paper may well be in the
 * folder — which is why the advisory says to record it, not that it is missing.
 *
 * There is no „mark as held“ button here on purpose. Asserting possession is an
 * act about a specific document, and there is no specific document on this
 * screen to assert it about; the operator records the isprava and links it.
 */
function IspravaWarningPanel({
  warning,
  onClose,
}: {
  warning: IspravaWarning;
  onClose: () => void;
}) {
  return (
    <div className="flex flex-col gap-4">
      <DialogHeader>
        <DialogTitle>Prijem je upisan — nema isprave dobavljača</DialogTitle>
        <DialogDescription>
          Prijem robe je evidentiran u lageru i u KEP-u. Isprava dobavljača nije
          vezana za ovaj prijem.
        </DialogDescription>
      </DialogHeader>

      <Alert>
        <AlertTitle>Isprava o nabavci</AlertTitle>
        <AlertDescription>
          <div className="flex flex-col gap-2">
            <p>{warning.advisory}</p>
            <p>{warning.notice.summary}</p>
            {warning.notice.penalty ? (
              <p>{warning.notice.penalty}</p>
            ) : (
              <p>
                Unesite pravnu formu u Podešavanja → Profil da bi kazna bila
                prikazana.
              </p>
            )}
            <p className="text-xs">{warning.notice.citation}</p>
          </div>
        </AlertDescription>
      </Alert>

      <DialogFooter>
        <Button type="button" onClick={onClose}>
          Zatvori
        </Button>
      </DialogFooter>
    </div>
  );
}

function ProductLedgerView({ ledger }: { ledger: ProductLedger }) {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <div className="text-sm font-medium">{ledger.product.productName}</div>
        <div className="text-xs text-muted-foreground">
          {ledger.product.sku} · trenutno{" "}
          {formatQuantity(
            ledger.product.currentQuantityMilli,
            ledger.product.unitOfMeasure,
          )}
        </div>
      </div>
      <Separator />
      {ledger.movements.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyTitle>Nema prometa za artikal</EmptyTitle>
            <EmptyDescription>
              Prva promena lagera ce otvoriti karticu.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Datum</TableHead>
              <TableHead>Tip</TableHead>
              <TableHead>Promena</TableHead>
              <TableHead>Stanje</TableHead>
              <TableHead>Razlog</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {ledger.movements.map((movement) => (
              <TableRow key={movement.id}>
                <TableCell>{formatDate(movement.createdAt)}</TableCell>
                <TableCell>{movementTypeLabel(movement.movementType)}</TableCell>
                <TableCell>
                  {formatSignedQuantity(
                    movement.quantityMilli,
                    ledger.product.unitOfMeasure,
                  )}
                </TableCell>
                <TableCell>
                  {formatQuantity(
                    movement.resultingQuantityMilli,
                    ledger.product.unitOfMeasure,
                  )}
                </TableCell>
                <TableCell>{movement.reason ?? "-"}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}

function adjustmentCopy(mode: AdjustmentMode) {
  switch (mode) {
    case "receive":
      return {
        title: "Prijem robe",
        submit: "Sačuvaj prijem",
        success: "Prijem robe je upisan.",
        icon: <PackagePlusIcon data-icon="inline-start" />,
      };
    case "correction":
      return {
        title: "Korekcija lagera",
        submit: "Sačuvaj korekciju",
        success: "Korekcija je upisana.",
        icon: <PencilIcon data-icon="inline-start" />,
      };
    case "write_off":
      return {
        title: "Otpis robe",
        submit: "Sačuvaj otpis",
        success: "Otpis je upisan.",
        icon: <PackageMinusIcon data-icon="inline-start" />,
      };
  }
}

function parseQuantityInput(input: string): number {
  const normalized = input.trim().replace(",", ".");
  if (!/^-?\d+(\.\d{1,3})?$/.test(normalized)) {
    throw new Error("Količina nije ispravna.");
  }

  const [whole, fractional = ""] = normalized.split(".");
  const sign = whole.startsWith("-") ? -1 : 1;
  const wholeDigits = whole.replace("-", "");
  const milli =
    Number(wholeDigits) * 1000 + Number(fractional.padEnd(3, "0"));

  if (!Number.isSafeInteger(milli)) {
    throw new Error("Količina nije ispravna.");
  }

  return sign * milli;
}

function formatQuantity(quantityMilli: number, unit: string): string {
  const sign = quantityMilli < 0 ? "-" : "";
  const absolute = Math.abs(quantityMilli);
  const whole = Math.floor(absolute / 1000);
  const fractional = absolute % 1000;

  if (fractional === 0) {
    return `${sign}${whole} ${unit}`;
  }

  return `${sign}${whole},${fractional
    .toString()
    .padStart(3, "0")
    .replace(/0+$/, "")} ${unit}`;
}

function formatSignedQuantity(quantityMilli: number, unit: string): string {
  return `${quantityMilli > 0 ? "+" : ""}${formatQuantity(quantityMilli, unit)}`;
}

function movementTypeLabel(type: string): string {
  switch (type) {
    case "receive":
      return "Prijem";
    case "correction":
      return "Korekcija";
    case "write_off":
      return "Otpis";
    case "sale":
      return "Prodaja";
    case "return":
      return "Povrat";
    case "void":
      return "Storno";
    default:
      return type;
  }
}

function formatDate(value: string | null): string {
  if (!value) {
    return "-";
  }

  return value.replace("T", " ").replace("Z", "");
}

function getCommandMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "object" && error && "message" in error) {
    return String((error as CommandErrorShape).message);
  }

  return "Operacija nije uspela.";
}

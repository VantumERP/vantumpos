import {
  FileTextIcon,
  RotateCcwIcon,
  SearchIcon,
  Undo2Icon,
} from "lucide-react";
import { useEffect, useState } from "react";

import { formatQuantity, parseQuantityInput } from "@/app/format";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
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
import type { ReceiptsService } from "@/services/ports";
import type {
  CommandErrorShape,
  PaymentMethod,
  ReceiptDetail,
  ReceiptSearchQuery,
  ReceiptSummary,
} from "@/services/types";

interface ReceiptsScreenProps {
  receipts: ReceiptsService;
}

interface ReceiptFilters {
  dateFrom: string;
  dateTo: string;
  receiptNumber: string;
  cashier: string;
  shiftId: string;
  paymentMethod: "" | PaymentMethod;
  product: string;
}

const emptyFilters: ReceiptFilters = {
  dateFrom: "",
  dateTo: "",
  receiptNumber: "",
  cashier: "",
  shiftId: "",
  paymentMethod: "",
  product: "",
};

const statusLabels = {
  completed: "Završen",
  voided: "Storniran",
  refunded: "Refundiran",
};

const documentLabels = {
  sale: "Prodaja",
  void: "Storno",
  return: "Povrat",
};

const fiscalLabels = {
  not_fiscalized: "Nije fiskalizovan",
  fiscalized: "Fiskalizovan",
  failed: "Fiskalizacija neuspešna",
};

const fiscalSummaryLabels = {
  not_fiscalized: "Bez fiskalizacije",
  fiscalized: "Fiskalizovan",
  failed: "Fiskalna greška",
};

// Exhaustive over PaymentMethod on purpose: an unmapped tender renders a
// blank cell on a settled receipt, which reads as "no payment recorded".
const paymentLabels: Record<PaymentMethod, string> = {
  cash: "Gotovina",
  card: "Kartica",
  bank_transfer: "Prenos na račun",
};

const paymentSummaryLabels: Record<PaymentMethod, string> = {
  cash: "Gotovinom",
  card: "Karticom",
  bank_transfer: "Prenosom na račun",
};

export function ReceiptsScreen({ receipts }: ReceiptsScreenProps) {
  const [filters, setFilters] = useState(emptyFilters);
  const [rows, setRows] = useState<ReceiptSummary[]>([]);
  const [selected, setSelected] = useState<ReceiptDetail | null>(null);
  const [voidOpen, setVoidOpen] = useState(false);
  const [voidReason, setVoidReason] = useState("");
  const [voidError, setVoidError] = useState<string | undefined>();
  const [returnOpen, setReturnOpen] = useState(false);
  const [returnReason, setReturnReason] = useState("");
  const [refundTender, setRefundTender] = useState<PaymentMethod>("cash");
  const [returnQuantities, setReturnQuantities] = useState<Record<number, string>>({});
  const [returnError, setReturnError] = useState<string | undefined>();
  const [listStatus, setListStatus] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [listError, setListError] = useState<string | undefined>();
  const [detailError, setDetailError] = useState<string | undefined>();

  async function runSearch(query: ReceiptSearchQuery = {}) {
    setListStatus("loading");
    setListError(undefined);

    try {
      const result = await receipts.searchReceipts(query);
      setRows(result.receipts);
      setListStatus("ready");
    } catch (error) {
      const commandError = error as CommandErrorShape;
      setListError(commandError.message ?? "Učitavanje računa nije uspelo.");
      setListStatus("error");
    }
  }

  useEffect(() => {
    let cancelled = false;

    setListStatus("loading");
    setListError(undefined);

    receipts
      .searchReceipts({})
      .then((result) => {
        if (!cancelled) {
          setRows(result.receipts);
          setListStatus("ready");
        }
      })
      .catch((error) => {
        if (!cancelled) {
          const commandError = error as CommandErrorShape;
          setListError(commandError.message ?? "Učitavanje računa nije uspelo.");
          setListStatus("error");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [receipts]);

  async function showDetail(id: number) {
    setDetailError(undefined);

    try {
      setSelected(await receipts.getReceipt(id));
    } catch (error) {
      const commandError = error as CommandErrorShape;
      setDetailError(commandError.message ?? "Učitavanje detalja nije uspelo.");
    }
  }

  async function submitSearch() {
    await runSearch(buildQuery(filters));
  }

  async function submitVoid() {
    if (!selected) {
      return;
    }

    if (!voidReason.trim()) {
      setVoidError("Unesite razlog storniranja.");
      return;
    }

    try {
      const detail = await receipts.voidReceipt({
        receiptId: selected.id,
        reason: voidReason,
      });
      setSelected(detail);
      setRows((current) =>
        current.map((row) =>
          row.id === detail.id
            ? {
                ...row,
                status: detail.status,
                linkedDocumentCount: detail.linkedDocuments.length,
              }
            : row,
        ),
      );
      setVoidOpen(false);
      setVoidReason("");
      setVoidError(undefined);
    } catch (error) {
      const commandError = error as CommandErrorShape;
      setVoidError(commandError.message ?? "Storniranje nije uspelo.");
    }
  }

  function openReturn() {
    if (!selected) {
      return;
    }

    const quantities = Object.fromEntries(
      selected.items.map((item) => [item.id, "0"]),
    );
    setReturnQuantities(quantities);
    setReturnReason("");
    setRefundTender("cash");
    setReturnError(undefined);
    setReturnOpen(true);
  }

  async function submitReturn() {
    if (!selected) {
      return;
    }

    if (!returnReason.trim()) {
      setReturnError("Unesite razlog povrata.");
      return;
    }

    try {
      const items = selected.items
        .map((item) => ({
          saleItemId: item.id,
          quantityMilli: parseQuantityInput(returnQuantities[item.id] ?? "0"),
        }))
        .filter((item) => item.quantityMilli > 0);

      if (items.length === 0) {
        setReturnError("Unesite količinu za bar jedan artikal.");
        return;
      }

      const detail = await receipts.returnItems({
        receiptId: selected.id,
        reason: returnReason,
        items,
        refundTender,
      });
      setSelected(detail);
      setRows((current) =>
        current.map((row) =>
          row.id === detail.id
            ? {
                ...row,
                status: detail.status,
                linkedDocumentCount: detail.linkedDocuments.length,
              }
            : row,
        ),
      );
      setReturnOpen(false);
    } catch (error) {
      const commandError = error as CommandErrorShape;
      setReturnError(commandError.message ?? "Povrat nije sačuvan.");
    }
  }

  return (
    <div className="grid flex-1 gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(28rem,0.9fr)]">
      <section className="flex min-w-0 flex-col gap-4">
        <div className="flex flex-col gap-2">
          <h2 className="text-base font-semibold">Pretraga računa</h2>
          <FieldGroup className="grid gap-3 md:grid-cols-4">
            <Field>
              <FieldLabel htmlFor="receipt-date-from">Od datuma</FieldLabel>
              <Input
                id="receipt-date-from"
                type="date"
                value={filters.dateFrom}
                onChange={(event) =>
                  setFilters((current) => ({
                    ...current,
                    dateFrom: event.target.value,
                  }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="receipt-date-to">Do datuma</FieldLabel>
              <Input
                id="receipt-date-to"
                type="date"
                value={filters.dateTo}
                onChange={(event) =>
                  setFilters((current) => ({
                    ...current,
                    dateTo: event.target.value,
                  }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="receipt-number">Broj računa</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="receipt-number"
                  value={filters.receiptNumber}
                  onChange={(event) =>
                    setFilters((current) => ({
                      ...current,
                      receiptNumber: event.target.value,
                    }))
                  }
                />
                <InputGroupAddon align="inline-end">
                  <SearchIcon aria-hidden="true" />
                </InputGroupAddon>
              </InputGroup>
            </Field>
            <Field>
              <FieldLabel htmlFor="receipt-cashier">Kasir</FieldLabel>
              <Input
                id="receipt-cashier"
                value={filters.cashier}
                onChange={(event) =>
                  setFilters((current) => ({
                    ...current,
                    cashier: event.target.value,
                  }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="receipt-shift">Smena</FieldLabel>
              <Input
                id="receipt-shift"
                inputMode="numeric"
                value={filters.shiftId}
                onChange={(event) =>
                  setFilters((current) => ({
                    ...current,
                    shiftId: event.target.value,
                  }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="receipt-payment-method">
                Način plaćanja
              </FieldLabel>
              <NativeSelect
                id="receipt-payment-method"
                value={filters.paymentMethod}
                onChange={(event) =>
                  setFilters((current) => ({
                    ...current,
                    paymentMethod: event.target.value as "" | PaymentMethod,
                  }))
                }
              >
                <NativeSelectOption value="">Svi</NativeSelectOption>
                <NativeSelectOption value="cash">
                  Gotovinsko plaćanje
                </NativeSelectOption>
                <NativeSelectOption value="card">Kartica</NativeSelectOption>
                <NativeSelectOption value="bank_transfer">
                  Prenos na račun
                </NativeSelectOption>
              </NativeSelect>
            </Field>
            <Field>
              <FieldLabel htmlFor="receipt-product">Artikal</FieldLabel>
              <Input
                id="receipt-product"
                value={filters.product}
                onChange={(event) =>
                  setFilters((current) => ({
                    ...current,
                    product: event.target.value,
                  }))
                }
              />
            </Field>
            <Field className="justify-end">
              <Button type="button" onClick={submitSearch}>
                <SearchIcon data-icon="inline-start" />
                Pretraži
              </Button>
            </Field>
          </FieldGroup>
        </div>

        {listStatus === "loading" ? (
          <div className="flex items-center gap-2 rounded-md border border-border p-4 text-sm text-muted-foreground">
            <Spinner aria-hidden="true" />
            Učitavanje računa...
          </div>
        ) : listStatus === "error" ? (
          <Alert variant="destructive">
            <AlertTitle>Računi nisu učitani</AlertTitle>
            <AlertDescription>{listError}</AlertDescription>
          </Alert>
        ) : rows.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <FileTextIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>Nema računa za izabrane filtere</EmptyTitle>
              <EmptyDescription>
                Promenite filtere ili napravite novu prodaju.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Broj</TableHead>
                <TableHead>Datum</TableHead>
                <TableHead>Kasir</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Fiskalno</TableHead>
                <TableHead>Plaćanje</TableHead>
                <TableHead>Ukupno</TableHead>
                <TableHead>Akcije</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.id}>
                  <TableCell>{row.receiptNumber}</TableCell>
                  <TableCell>{formatDateTime(row.createdAt)}</TableCell>
                  <TableCell>{row.cashierName}</TableCell>
                  <TableCell>
                    <Badge variant={row.status === "completed" ? "secondary" : "outline"}>
                      {statusLabels[row.status]}
                    </Badge>
                  </TableCell>
                  <TableCell>{fiscalSummaryLabels[row.fiscalStatus]}</TableCell>
                  <TableCell>
                    {row.paymentMethods
                      .map((method) => paymentSummaryLabels[method])
                      .join(", ")}
                  </TableCell>
                  <TableCell>{formatRsd(row.totalMinor)}</TableCell>
                  <TableCell>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => showDetail(row.id)}
                    >
                      <FileTextIcon data-icon="inline-start" />
                      Detalji za {row.receiptNumber}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </section>

      <section className="flex min-w-0 flex-col gap-4">
        {detailError ? (
          <Alert variant="destructive">
            <AlertTitle>Detalji nisu učitani</AlertTitle>
            <AlertDescription>{detailError}</AlertDescription>
          </Alert>
        ) : null}
        {selected ? (
          <ReceiptDetailPanel
            detail={selected}
            onVoid={() => {
              setVoidReason("");
              setVoidError(undefined);
              setVoidOpen(true);
            }}
            onReturn={openReturn}
          />
        ) : (
          <div className="rounded-md border border-border p-4 text-sm text-muted-foreground">
            Izaberite račun za detalje.
          </div>
        )}
      </section>

      <AlertDialog open={voidOpen} onOpenChange={setVoidOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Storniranje računa</AlertDialogTitle>
            <AlertDialogDescription>
              Storno pravi novi povezani dokument i vraća robu na lager.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <Field data-invalid={Boolean(voidError)}>
            <FieldLabel htmlFor="void-reason">Razlog</FieldLabel>
            <Textarea
              id="void-reason"
              value={voidReason}
              aria-invalid={Boolean(voidError)}
              onChange={(event) => setVoidReason(event.target.value)}
            />
            <FieldError>{voidError}</FieldError>
          </Field>
          <AlertDialogFooter>
            <AlertDialogCancel>Odustani</AlertDialogCancel>
            <AlertDialogAction type="button" onClick={submitVoid}>
              Potvrdi storniranje
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <Sheet open={returnOpen} onOpenChange={setReturnOpen}>
        <SheetContent>
          <SheetHeader>
            <SheetTitle>Povrat artikala</SheetTitle>
            <SheetDescription>{selected?.receiptNumber}</SheetDescription>
          </SheetHeader>
          <div className="flex flex-col gap-4 px-6">
            <FieldGroup>
              {(selected?.items ?? []).map((item) => (
                <Field key={item.id}>
                  <FieldLabel htmlFor={`return-item-${item.id}`}>
                    Količina za {item.productName}
                  </FieldLabel>
                  <Input
                    id={`return-item-${item.id}`}
                    inputMode="decimal"
                    value={returnQuantities[item.id] ?? "0"}
                    onChange={(event) =>
                      setReturnQuantities((current) => ({
                        ...current,
                        [item.id]: event.target.value,
                      }))
                    }
                  />
                </Field>
              ))}
              <Field>
                <FieldLabel htmlFor="return-tender">Način povrata</FieldLabel>
                <NativeSelect
                  id="return-tender"
                  aria-label="Način povrata"
                  value={refundTender}
                  onChange={(event) =>
                    setRefundTender(event.target.value as PaymentMethod)
                  }
                >
                  <NativeSelectOption value="cash">Gotovina</NativeSelectOption>
                  <NativeSelectOption value="card">Kartica</NativeSelectOption>
                  <NativeSelectOption value="bank_transfer">
                    Prenos na račun
                  </NativeSelectOption>
                </NativeSelect>
              </Field>
              <Field data-invalid={Boolean(returnError)}>
                <FieldLabel htmlFor="return-reason">Razlog povrata</FieldLabel>
                <Textarea
                  id="return-reason"
                  value={returnReason}
                  aria-invalid={Boolean(returnError)}
                  onChange={(event) => setReturnReason(event.target.value)}
                />
                <FieldError>{returnError}</FieldError>
              </Field>
            </FieldGroup>
          </div>
          <SheetFooter>
            <Button type="button" onClick={submitReturn}>
              Sačuvaj povrat
            </Button>
          </SheetFooter>
        </SheetContent>
      </Sheet>
    </div>
  );
}

interface ReceiptDetailPanelProps {
  detail: ReceiptDetail;
  onVoid(): void;
  onReturn(): void;
}

function ReceiptDetailPanel({ detail, onVoid, onReturn }: ReceiptDetailPanelProps) {
  return (
    <div className="flex flex-col gap-4 rounded-md border border-border p-4">
      {/*
        Legal: PVFR cl. 2 st. 8-10 - the banner must be at least twice the
        line-item font, and that is arithmetic rather than a look. The stavke
        below are a <Table>, whose root is text-xs (0.75rem), so text-2xl
        (1.5rem) is exactly 2x. This was text-xl until 07.08.2026, which is
        1.6x and measurably short;
        docs_guard::the_non_fiscal_banner_is_at_least_twice_the_line_item_font
        computes the ratio from these class names, so lowering either one fails
        the crate rather than quietly breaking the Pravilnik.
      */}
      <div
        role="note"
        className="rounded-md border-2 border-destructive bg-destructive/10 px-3 py-2 text-center text-2xl font-bold uppercase tracking-wide text-destructive"
      >
        OVO NIJE FISKALNI RAČUN
      </div>
      <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
        <div className="min-w-0">
          <h2 className="text-base font-semibold">Račun {detail.receiptNumber}</h2>
          <div className="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
            <span>{formatDateTime(detail.createdAt)}</span>
            <span>{detail.cashierName}</span>
            <span>Smena {detail.shiftId}</span>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Badge variant="secondary">{documentLabels[detail.documentType]}</Badge>
          <Badge variant="outline">{statusLabels[detail.status]}</Badge>
          <Badge variant="outline">{fiscalLabels[detail.fiscalStatus]}</Badge>
          {detail.esirReceiptNumber ? (
            <Badge variant="outline">ESIR: {detail.esirReceiptNumber}</Badge>
          ) : null}
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="destructive" disabled={!detail.canVoid} onClick={onVoid}>
          <Undo2Icon data-icon="inline-start" />
          Storniraj račun
        </Button>
        <Button type="button" variant="outline" disabled={!detail.canReturn} onClick={onReturn}>
          <RotateCcwIcon data-icon="inline-start" />
          Povrat artikala
        </Button>
      </div>

      <Separator />

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Artikal</TableHead>
            <TableHead>SKU</TableHead>
            <TableHead>Količina</TableHead>
            <TableHead>Vraćao</TableHead>
            <TableHead>Cena</TableHead>
            <TableHead>PDV</TableHead>
            <TableHead>Ukupno</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {detail.items.map((item) => (
            <TableRow key={item.id}>
              <TableCell>{item.productName}</TableCell>
              <TableCell>{item.productSku}</TableCell>
              <TableCell>{formatQuantity(item.quantityMilli)}</TableCell>
              <TableCell>
                {item.returnedQuantityMilli > 0
                  ? `Vraćao ${formatQuantity(item.returnedQuantityMilli)}`
                  : "-"}
              </TableCell>
              <TableCell>{formatRsd(item.unitPriceMinor)}</TableCell>
              <TableCell>{formatRsd(item.taxMinor)}</TableCell>
              <TableCell>{formatRsd(item.totalMinor)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      <div className="grid gap-4 md:grid-cols-2">
        <div className="flex flex-col gap-2">
          <h3 className="text-sm font-medium">Plaćanja</h3>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Način</TableHead>
                <TableHead>Iznos</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {detail.payments.map((payment) => (
                <TableRow key={payment.id}>
                  <TableCell>{paymentLabels[payment.paymentMethod]}</TableCell>
                  <TableCell>{formatRsd(payment.amountMinor)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>

        <div className="flex flex-col gap-2">
          <h3 className="text-sm font-medium">Ukupno</h3>
          <dl className="grid grid-cols-2 gap-2 text-sm">
            <dt className="text-muted-foreground">Osnovica</dt>
            <dd className="text-end">{formatRsd(detail.subtotalMinor)}</dd>
            <dt className="text-muted-foreground">Popust</dt>
            <dd className="text-end">{formatRsd(detail.discountMinor)}</dd>
            <dt className="text-muted-foreground">PDV</dt>
            <dd className="text-end">{formatRsd(detail.taxMinor)}</dd>
            <dt className="font-medium">Ukupno</dt>
            <dd className="text-end font-medium">{formatRsd(detail.totalMinor)}</dd>
          </dl>
        </div>
      </div>

      {detail.voidReason ? (
        <div className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">Razlog storniranja</h3>
          <p className="text-sm text-muted-foreground">{detail.voidReason}</p>
        </div>
      ) : null}

      {detail.returnReason ? (
        <div className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">Razlog povrata</h3>
          <p className="text-sm text-muted-foreground">{detail.returnReason}</p>
        </div>
      ) : null}

      <div className="flex flex-col gap-2">
        <h3 className="text-sm font-medium">Povezani dokumenti</h3>
        {detail.linkedDocuments.length > 0 ? (
          <div className="flex flex-col gap-2">
            {detail.linkedDocuments.map((document) => (
              <div
                key={document.id}
                className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border p-2 text-sm"
              >
                <span>{document.receiptNumber}</span>
                <span>{documentLabels[document.documentType]}</span>
                <span>{formatRsd(document.totalMinor)}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">Nema povezanih dokumenata.</p>
        )}
      </div>
    </div>
  );
}

function buildQuery(filters: ReceiptFilters): ReceiptSearchQuery {
  return {
    dateFrom: filters.dateFrom || undefined,
    dateTo: filters.dateTo || undefined,
    receiptNumber: filters.receiptNumber || undefined,
    cashier: filters.cashier || undefined,
    shiftId: filters.shiftId ? Number(filters.shiftId) : undefined,
    paymentMethod: filters.paymentMethod || undefined,
    product: filters.product || undefined,
  };
}

function formatDateTime(value: string): string {
  return new Intl.DateTimeFormat("sr-Latn-RS", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value));
}

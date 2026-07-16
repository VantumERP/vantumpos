import {
  MinusIcon,
  PlusIcon,
  ReceiptTextIcon,
  SearchIcon,
  Trash2Icon,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

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
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Empty, EmptyContent, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
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
import type { PosServices } from "@/services/ports";
import type {
  CommandError,
  CompletedSale,
  DiscountDraft,
  ProductSummary,
  SaleDraftItem,
  SalePreview,
} from "@/services/types";

interface RegisterScreenProps {
  services: PosServices;
}

interface CartItem {
  product: ProductSummary;
  quantityInput: string;
  quantityMilli: number;
  discountInput: string;
  discount: DiscountDraft | null;
}

type PreviewState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; preview: SalePreview }
  | { status: "error"; message: string };

export function RegisterScreen({ services }: RegisterScreenProps) {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [search, setSearch] = useState("");
  const [matches, setMatches] = useState<ProductSummary[]>([]);
  const [cart, setCart] = useState<CartItem[]>([]);
  const [previewState, setPreviewState] = useState<PreviewState>({
    status: "idle",
  });
  const [receiptDiscountInput, setReceiptDiscountInput] = useState("");
  const [cashInput, setCashInput] = useState("");
  const [cashTouched, setCashTouched] = useState(false);
  const [cardInput, setCardInput] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [isCompleting, setIsCompleting] = useState(false);
  const [overrideOpen, setOverrideOpen] = useState(false);
  const [completedSale, setCompletedSale] = useState<CompletedSale | null>(null);

  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  const receiptDiscount = useMemo(
    () => moneyDiscountFromInput(receiptDiscountInput),
    [receiptDiscountInput],
  );
  const draft = useMemo(
    () => ({
      items: cart.map(toSaleDraftItem),
      receiptDiscount,
    }),
    [cart, receiptDiscount],
  );

  useEffect(() => {
    if (draft.items.length === 0) {
      setPreviewState({ status: "idle" });
      return;
    }

    let active = true;
    setPreviewState({ status: "loading" });

    services.sales
      .createSalePreview(draft)
      .then((preview) => {
        if (active) {
          setPreviewState({ status: "ready", preview });
        }
      })
      .catch((error: unknown) => {
        if (active) {
          setPreviewState({
            status: "error",
            message: errorMessage(error),
          });
        }
      });

    return () => {
      active = false;
    };
  }, [draft, services]);

  const preview =
    previewState.status === "ready" ? previewState.preview : undefined;
  const previewTotalMinor = preview?.totalMinor;
  const cashMinor = parseOptionalMoney(cashInput);
  const cardMinor = parseOptionalMoney(cardInput);
  const changeMinor =
    preview && cashMinor + cardMinor > preview.totalMinor
      ? cashMinor + cardMinor - preview.totalMinor
      : 0;

  useEffect(() => {
    if (!cashTouched && previewTotalMinor !== undefined) {
      // Prefill only the cash still due after any card amount, so a card-only
      // sale prefills 0 and never shows phantom change.
      const cashDueMinor = Math.max(0, previewTotalMinor - cardMinor);
      setCashInput((cashDueMinor / 100).toFixed(2));
    }
  }, [previewTotalMinor, cardMinor, cashTouched]);

  async function handleSearchSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = search.trim();

    if (!query) {
      return;
    }

    setMessage(null);
    const result = await services.catalog.searchProducts({
      search: query,
      active: true,
      limit: 20,
    });
    const items = result.items;

    if (items.length === 0) {
      setMatches([]);
      setMessage("Artikal nije pronađen.");
      return;
    }

    const exact = items.find(
      (item) =>
        (item.barcode && item.barcode.toLowerCase() === query.toLowerCase()) ||
        item.sku.toLowerCase() === query.toLowerCase(),
    );

    if (exact) {
      addProduct(exact);
      setSearch("");
      setMatches([]);
      return;
    }

    if (items.length === 1) {
      addProduct(items[0]);
      setSearch("");
      setMatches([]);
      return;
    }

    setMatches(items);
  }

  function chooseMatch(item: ProductSummary) {
    addProduct(item);
    setSearch("");
    setMatches([]);
  }

  function addProduct(product: ProductSummary) {
    setCart((current) => {
      const existing = current.find((item) => item.product.id === product.id);

      if (!existing) {
        return [
          ...current,
          {
            product,
            quantityInput: "1",
            quantityMilli: 1000,
            discountInput: "",
            discount: null,
          },
        ];
      }

      return current.map((item) =>
        item.product.id === product.id
          ? {
              ...item,
              quantityInput: formatQuantity(item.quantityMilli + 1000),
              quantityMilli: item.quantityMilli + 1000,
            }
          : item,
      );
    });
  }

  function updateQuantity(productId: number, value: string) {
    const quantityMilli = parseQuantityInput(value);

    setCart((current) =>
      current.map((item) =>
        item.product.id === productId
          ? {
              ...item,
              quantityInput: value,
              quantityMilli,
            }
          : item,
      ),
    );
  }

  function updateDiscount(productId: number, value: string) {
    setCart((current) =>
      current.map((item) =>
        item.product.id === productId
          ? {
              ...item,
              discountInput: value,
              discount: moneyDiscountFromInput(value),
            }
          : item,
      ),
    );
  }

  function removeItem(productId: number) {
    setCart((current) =>
      current.filter((item) => item.product.id !== productId),
    );
  }

  async function completeSale(allowStockOverride = false) {
    if (!preview) {
      return;
    }

    setMessage(null);
    setIsCompleting(true);

    try {
      const sale = await services.sales.completeSale({
        ...draft,
        payments: [
          ...(cashMinor > 0
            ? [{ method: "cash" as const, amountMinor: cashMinor }]
            : []),
          ...(cardMinor > 0
            ? [{ method: "card" as const, amountMinor: cardMinor }]
            : []),
        ],
        ...(allowStockOverride ? { allowStockOverride: true } : {}),
      });
      setCompletedSale(sale);
      setCart([]);
      setPreviewState({ status: "idle" });
      setCashInput("");
      setCashTouched(false);
      setCardInput("");
      setReceiptDiscountInput("");
      setOverrideOpen(false);
    } catch (error) {
      if ((error as CommandError).code === "insufficient_stock") {
        setOverrideOpen(true);
      } else {
        setMessage(errorMessage(error));
      }
    } finally {
      setIsCompleting(false);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 p-4">
      <form onSubmit={handleSearchSubmit}>
        <InputGroup className="h-11">
          <InputGroupAddon>
            <SearchIcon aria-hidden="true" />
          </InputGroupAddon>
          <InputGroupInput
            ref={searchInputRef}
            type="search"
            role="searchbox"
            aria-label="Skeniraj barkod ili pretraži artikal"
            placeholder="Skeniraj barkod ili pretraži artikal"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
          <InputGroupAddon align="inline-end">
            <InputGroupButton type="submit">Dodaj</InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      </form>

      {matches.length > 0 && (
        <div
          role="listbox"
          aria-label="Rezultati pretrage"
          className="divide-y rounded-md border bg-card"
        >
          {matches.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => chooseMatch(item)}
              className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-sm hover:bg-accent"
            >
              <span className="min-w-0 truncate font-medium">{item.name}</span>
              <span className="shrink-0 tabular-nums text-muted-foreground">
                {formatRsd(item.salePriceMinor)} ·{" "}
                {formatQuantity(item.currentStockMilli)} {item.unitOfMeasure}
              </span>
            </button>
          ))}
        </div>
      )}

      {message && (
        <Alert variant="destructive">
          <AlertTitle>Prodaja nije završena</AlertTitle>
          <AlertDescription>{message}</AlertDescription>
        </Alert>
      )}

      {previewState.status === "error" && (
        <Alert variant="destructive">
          <AlertTitle>Pregled računa nije moguć</AlertTitle>
          <AlertDescription>{previewState.message}</AlertDescription>
        </Alert>
      )}

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <section className="min-h-0 rounded-md border bg-card">
          {cart.length === 0 ? (
            <Empty className="min-h-96">
              <EmptyHeader>
                <EmptyTitle>Korpa je prazna</EmptyTitle>
              </EmptyHeader>
              <EmptyContent>
                <p className="text-xs text-muted-foreground">
                  Skenirajte barkod ili pretražite artikal.
                </p>
              </EmptyContent>
            </Empty>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Artikal</TableHead>
                  <TableHead>SKU/barcode</TableHead>
                  <TableHead>Količina</TableHead>
                  <TableHead>Cena</TableHead>
                  <TableHead>Popust</TableHead>
                  <TableHead>Ukupno</TableHead>
                  <TableHead>
                    <span className="sr-only">Akcije</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {cart.map((item) => {
                  const previewItem = preview?.items.find(
                    (candidate) => candidate.productId === item.product.id,
                  );

                  return (
                    <TableRow key={item.product.id}>
                      <TableCell>
                        <div className="flex flex-col gap-1">
                          <span className="font-medium">{item.product.name}</span>
                          <Badge variant="outline">{item.product.unitOfMeasure}</Badge>
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-col gap-1">
                          <span>{item.product.sku}</span>
                          <span className="text-muted-foreground">
                            {item.product.barcode}
                          </span>
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-2">
                          <Button
                            type="button"
                            variant="outline"
                            size="icon-sm"
                            aria-label={`Smanji količinu za ${item.product.name}`}
                            onClick={() =>
                              updateQuantity(
                                item.product.id,
                                formatQuantity(
                                  Math.max(1000, item.quantityMilli - 1000),
                                ),
                              )
                            }
                          >
                            <MinusIcon />
                          </Button>
                          <input
                            aria-label={`Količina za ${item.product.name}`}
                            className="h-8 w-20 rounded-md border bg-background px-2 text-center text-sm"
                            inputMode="decimal"
                            value={item.quantityInput}
                            onChange={(event) =>
                              updateQuantity(item.product.id, event.target.value)
                            }
                          />
                          <Button
                            type="button"
                            variant="outline"
                            size="icon-sm"
                            aria-label={`Povećaj količinu za ${item.product.name}`}
                            onClick={() =>
                              updateQuantity(
                                item.product.id,
                                formatQuantity(item.quantityMilli + 1000),
                              )
                            }
                          >
                            <PlusIcon />
                          </Button>
                        </div>
                      </TableCell>
                      <TableCell>{formatRsd(item.product.salePriceMinor)}</TableCell>
                      <TableCell>
                        <input
                          aria-label={`Popust za ${item.product.name}`}
                          className="h-8 w-24 rounded-md border bg-background px-2 text-sm"
                          inputMode="decimal"
                          placeholder="20 ili 20%"
                          value={item.discountInput}
                          onChange={(event) =>
                            updateDiscount(item.product.id, event.target.value)
                          }
                        />
                      </TableCell>
                      <TableCell>
                        {previewItem ? formatRsd(previewItem.totalMinor) : "-"}
                      </TableCell>
                      <TableCell>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon-sm"
                          aria-label={`Ukloni ${item.product.name}`}
                          onClick={() => removeItem(item.product.id)}
                        >
                          <Trash2Icon />
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          )}
        </section>

        <aside className="flex flex-col gap-4 rounded-md border bg-card p-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold">Naplata</h2>
            {previewState.status === "loading" && <Spinner />}
          </div>
          <Totals preview={preview} />
          <Separator />
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="receipt-discount">Popust na račun</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="receipt-discount"
                  inputMode="decimal"
                  placeholder="20 ili 20%"
                  value={receiptDiscountInput}
                  onChange={(event) => setReceiptDiscountInput(event.target.value)}
                />
                <InputGroupAddon align="inline-end">RSD</InputGroupAddon>
              </InputGroup>
            </Field>
            <Field>
              <FieldLabel htmlFor="cash-received">Gotovina primljeno</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="cash-received"
                  inputMode="decimal"
                  value={cashInput}
                  onChange={(event) => {
                    setCashTouched(true);
                    setCashInput(event.target.value);
                  }}
                />
                <InputGroupAddon align="inline-end">RSD</InputGroupAddon>
              </InputGroup>
            </Field>
            <Field>
              <FieldLabel htmlFor="card-amount">Kartica</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="card-amount"
                  inputMode="decimal"
                  value={cardInput}
                  onChange={(event) => setCardInput(event.target.value)}
                />
                <InputGroupAddon align="inline-end">RSD</InputGroupAddon>
              </InputGroup>
            </Field>
          </FieldGroup>
          <div className="rounded-md bg-muted p-3">
            <div className="flex items-center justify-between text-sm">
              <span>Kusur</span>
              <span className="font-semibold">{formatRsd(changeMinor)}</span>
            </div>
          </div>
          <div className="mt-auto flex gap-2">
            <AlertDialog>
              <AlertDialogTrigger render={<Button variant="outline" type="button" />}>
                Isprazni
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Isprazniti korpu?</AlertDialogTitle>
                  <AlertDialogDescription>
                    Stavke u korpi će biti uklonjene.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Odustani</AlertDialogCancel>
                  <AlertDialogAction
                    type="button"
                    onClick={() => {
                      setCart([]);
                      setMessage(null);
                    }}
                  >
                    Isprazni
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
            <Button
              className="flex-1"
              type="button"
              disabled={!preview || isCompleting}
              onClick={() => completeSale()}
            >
              {isCompleting ? (
                <Spinner data-icon="inline-start" />
              ) : (
                <ReceiptTextIcon data-icon="inline-start" />
              )}
              Završi prodaju
            </Button>
          </div>
        </aside>
      </div>

      <AlertDialog open={overrideOpen} onOpenChange={setOverrideOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Nema dovoljno zaliha</AlertDialogTitle>
            <AlertDialogDescription>
              Stanje na kartici je manje od količine na računu. Želite li ipak da
              prodate i pustite stanje u minus?
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Odustani</AlertDialogCancel>
            <AlertDialogAction onClick={() => completeSale(true)}>
              Ipak prodaj
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <Dialog
        open={completedSale !== null}
        onOpenChange={(open) => {
          if (!open) {
            setCompletedSale(null);
          }
        }}
      >
        <DialogContent>
          {completedSale && (
            <>
              <DialogHeader>
                <DialogTitle>Račun {completedSale.localReceiptNumber}</DialogTitle>
                <DialogDescription>
                  Interni pregled prodaje — nije fiskalni račun
                </DialogDescription>
              </DialogHeader>
              <NonFiscalBanner />
              <div className="flex flex-col gap-3">
                <div className="flex justify-between">
                  <span>Kasir</span>
                  <span>{completedSale.cashierName}</span>
                </div>
                <Separator />
                {completedSale.items.map((item) => (
                  <div
                    className="flex items-center justify-between gap-3"
                    key={item.productId}
                  >
                    <div>
                      <div className="font-medium">{item.productName}</div>
                      <div className="text-muted-foreground">
                        {item.quantityLabel}
                      </div>
                    </div>
                    <span>{formatRsd(item.totalMinor)}</span>
                  </div>
                ))}
                <Separator />
                <div className="flex justify-between text-sm font-semibold">
                  <span>Ukupno</span>
                  <span>{formatRsd(completedSale.totalMinor)}</span>
                </div>
                <div className="flex justify-between text-sm">
                  <span>Kusur</span>
                  <span>{formatRsd(completedSale.changeDueMinor)}</span>
                </div>
              </div>
              <NonFiscalBanner />
            </>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function Totals({ preview }: { preview?: SalePreview }) {
  return (
    <div className="flex flex-col gap-2 text-sm">
      <TotalRow label="Međuzbir" value={preview?.subtotalMinor ?? 0} />
      <TotalRow label="Popust" value={preview?.discountMinor ?? 0} />
      <TotalRow label="PDV" value={preview?.taxMinor ?? 0} />
      <Separator />
      <div className="flex items-center justify-between text-base font-semibold">
        <span>Ukupno</span>
        <span>{formatRsd(preview?.totalMinor ?? 0)}</span>
      </div>
    </div>
  );
}

function TotalRow({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center justify-between">
      <span>{label}</span>
      <span>{formatRsd(value)}</span>
    </div>
  );
}

function toSaleDraftItem(item: CartItem): SaleDraftItem {
  return {
    productId: item.product.id,
    quantityMilli: item.quantityMilli,
    discount: item.discount,
  };
}

function parseOptionalMoney(input: string) {
  if (!input.trim()) {
    return 0;
  }

  try {
    return parseRsdInput(input);
  } catch {
    return 0;
  }
}

function moneyDiscountFromInput(input: string): DiscountDraft | null {
  const trimmed = input.trim();

  if (trimmed.endsWith("%")) {
    const percent = Number.parseFloat(
      trimmed.slice(0, -1).replace(",", ".").trim(),
    );

    if (!Number.isFinite(percent) || percent <= 0) {
      return null;
    }

    return { type: "percent", basisPoints: Math.round(percent * 100) };
  }

  const amountMinor = parseOptionalMoney(input);

  return amountMinor > 0 ? { type: "amount", amountMinor } : null;
}

function parseQuantityInput(input: string) {
  const normalized = input.replace(",", ".").trim();
  const value = Number.parseFloat(normalized);

  if (!Number.isFinite(value) || value <= 0) {
    return 0;
  }

  return Math.round(value * 1000);
}

function formatQuantity(quantityMilli: number) {
  return (quantityMilli / 1000).toString().replace(".", ",");
}

function errorMessage(error: unknown) {
  if (isCommandError(error)) {
    return error.message;
  }

  if (error instanceof Error) {
    return error.message;
  }

  return "Operacija nije uspela.";
}

function isCommandError(error: unknown): error is CommandError {
  return (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as CommandError).message === "string"
  );
}

function NonFiscalBanner() {
  // Legal: PVFR čl. 2 st. 8–10 — any sale-itemizing surface must be
  // unmistakably NON-fiscal. Never add a QR code, "FISKALNI RAČUN" heading,
  // or PIB/PFR/brojač block to this or any future receipt export.
  return (
    <div
      role="note"
      className="rounded-md border-2 border-destructive bg-destructive/10 px-3 py-2 text-center text-2xl font-bold uppercase tracking-wide text-destructive"
    >
      OVO NIJE FISKALNI RAČUN
    </div>
  );
}

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
import { Textarea } from "@/components/ui/textarea";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { PosServices } from "@/services/ports";
import type {
  AmlAssessment,
  CommandError,
  CompletedSale,
  DiscountDraft,
  SalePaymentDraft,
  ProductSummary,
  SaleDraftItem,
  SalePreview,
} from "@/services/types";

/**
 * The till asks the backend for an AML verdict as the operator types, so the
 * debounce has to be short enough that the warning is on screen before the
 * money changes hands.
 */
const AML_ASSESS_DEBOUNCE_MS = 200;

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
  const [bankTransferInput, setBankTransferInput] = useState("");
  const [assessment, setAssessment] = useState<AmlAssessment | null>(null);
  const [assessFailed, setAssessFailed] = useState(false);
  // The cash line `assessment`/`assessFailed` actually describe. The verdict is
  // debounced, so without this the till cannot tell a verdict for the tender in
  // the field from one for the tender before last — and a soft block that
  // consults a stale verdict is a soft block a fast operator walks straight
  // through.
  const [assessedCashMinor, setAssessedCashMinor] = useState<number | null>(
    null,
  );
  const [amlReason, setAmlReason] = useState("");
  const [amlReasonError, setAmlReasonError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [isCompleting, setIsCompleting] = useState(false);
  const [overrideOpen, setOverrideOpen] = useState(false);
  const [completedSale, setCompletedSale] = useState<CompletedSale | null>(null);
  const [esirNumber, setEsirNumberValue] = useState("");

  useEffect(() => {
    searchInputRef.current?.focus();
  }, []);

  useEffect(() => {
    setEsirNumberValue("");
  }, [completedSale]);

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
  const bankTransferMinor = parseOptionalMoney(bankTransferInput);
  const tenderedMinor = cashMinor + cardMinor + bankTransferMinor;
  const changeMinor =
    preview && tenderedMinor > preview.totalMinor
      ? tenderedMinor - preview.totalMinor
      : 0;
  // The cash the shop actually KEEPS — the tender minus the change handed
  // back. `complete_sale_transaction` assesses exactly this (its
  // `stored_cash_minor` = total - card - transfer), so assessing the raw
  // tender here would warn about sales the record shows as lawful.
  const retainedCashMinor = preview
    ? Math.max(
        0,
        Math.min(cashMinor, preview.totalMinor - cardMinor - bankTransferMinor),
      )
    : 0;

  useEffect(() => {
    if (!cashTouched && previewTotalMinor !== undefined) {
      // Prefill only the cash still due after any cashless amount, so a
      // card- or transfer-only sale prefills 0 and never shows phantom change.
      // It is also what lets the operator clear an AML breach by moving the
      // money to the bank-transfer line (čl. 46 st. 1's lawful alternative).
      const cashDueMinor = Math.max(
        0,
        previewTotalMinor - cardMinor - bankTransferMinor,
      );
      setCashInput((cashDueMinor / 100).toFixed(2));
    }
  }, [previewTotalMinor, cardMinor, bankTransferMinor, cashTouched]);

  // AML čl. 46 st. 1 keys to the CASH line, never the invoice total. The
  // verdict is advisory: a failed or unavailable check degrades to a note and
  // never blocks the till.
  useEffect(() => {
    if (retainedCashMinor <= 0) {
      setAssessment(null);
      setAssessFailed(false);
      setAssessedCashMinor(null);
      return;
    }

    let active = true;
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const verdict =
            await services.sales.assessCashPayment(retainedCashMinor);

          if (active) {
            setAssessment(verdict);
            setAssessFailed(false);
            setAssessedCashMinor(retainedCashMinor);
          }
        } catch {
          if (active) {
            // A check that could not run is NOT a check that passed. `aml.rs`
            // and `sales.rs` both degrade to a visible verdict rather than to
            // silence; rendering nothing here would put the silent pass back
            // one layer up, where the operator reads it as an all-clear.
            //
            // Unlike a missing rate this is never a steady state — the backend
            // answers even a corrupt settings row — so it is worth saying at
            // any amount rather than only past the stand-in cap.
            setAssessment(null);
            setAssessFailed(true);
            setAssessedCashMinor(retainedCashMinor);
          }
        }
      })();
    }, AML_ASSESS_DEBOUNCE_MS);

    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [retainedCashMinor, services]);

  // True from the keystroke until the verdict for THAT cash line lands: the
  // debounce timer, the in-flight call, and the stretch where a settled verdict
  // still belongs to an older tender. Nothing may read `amlBreached` as final
  // while this holds.
  const amlAssessmentPending =
    retainedCashMinor > 0 && assessedCashMinor !== retainedCashMinor;
  const amlBreached = assessment?.breached ?? false;
  const amlWarns = amlBreached || (assessment?.nearThreshold ?? false);
  // A cached rate is missing on every fresh install, so saying so on a 200 RSD
  // bread sale would fire on every transaction forever and train the cashier
  // to dismiss the AML alert on sight. The stand-in cap comes from `aml.rs`;
  // it sits below any real cap, so a genuine breach is still unmissable.
  const amlCheckUnavailable =
    assessment !== null &&
    assessment.rateUnavailable &&
    retainedCashMinor >= assessment.fallbackThresholdMinor;

  useEffect(() => {
    if (!amlBreached) {
      setAmlReasonError(null);
      // The acknowledgement belongs to the tender it was typed for. Keeping it
      // would attach a breach explanation to a sale whose breach the operator
      // already cleared — and never saw acknowledged.
      setAmlReason("");
    }
  }, [amlBreached]);

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

    // The rendered verdict is debounced, so between the last keystroke and the
    // assessment landing it describes the tender before this one. Deciding the
    // soft block on it would let a breach through to anyone who types and
    // clicks inside that window — settle the verdict for the cash line that is
    // actually about to be booked before deciding anything.
    let verdict = assessment;

    if (amlAssessmentPending) {
      setIsCompleting(true);

      try {
        verdict = await services.sales.assessCashPayment(retainedCashMinor);
        setAssessment(verdict);
        setAssessFailed(false);
      } catch {
        // Same degradation as the debounced path: a check that could not run is
        // not a check that passed, but it must not hold the till hostage
        // either.
        verdict = null;
        setAssessment(null);
        setAssessFailed(true);
      }

      setAssessedCashMinor(retainedCashMinor);
    }

    const breached = verdict?.breached ?? false;
    const reason = amlReason.trim();

    // A soft block: the sale is lawful to record either way, but a breach of
    // čl. 46 st. 1 must not go into the books unexplained.
    if (breached && !reason) {
      setAmlReasonError(
        "Unesite razlog prijema gotovine pre nego što završite prodaju.",
      );
      setIsCompleting(false);
      return;
    }

    setAmlReasonError(null);
    setIsCompleting(true);

    const payments: SalePaymentDraft[] = [
      ...(cashMinor > 0
        ? [{ method: "cash" as const, amountMinor: cashMinor }]
        : []),
      ...(cardMinor > 0
        ? [{ method: "card" as const, amountMinor: cardMinor }]
        : []),
      ...(bankTransferMinor > 0
        ? [{ method: "bank_transfer" as const, amountMinor: bankTransferMinor }]
        : []),
    ];

    try {
      const sale = await services.sales.completeSale({
        ...draft,
        payments,
        ...(allowStockOverride ? { allowStockOverride: true } : {}),
        // Only a breach is ever acknowledged — the reason field is the only
        // place a reason can be typed, and it only renders on a breach.
        ...(breached && reason ? { amlAckReason: reason } : {}),
      });
      setCompletedSale(sale);
      setCart([]);
      setPreviewState({ status: "idle" });
      setCashInput("");
      setCashTouched(false);
      setCardInput("");
      setBankTransferInput("");
      setAssessment(null);
      setAssessFailed(false);
      // Without this the NEXT sale for the same cash figure would look already
      // assessed while `assessment` is null — the bypass again, one sale later.
      setAssessedCashMinor(null);
      setAmlReason("");
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
            <Field>
              {/* The lawful alternative čl. 46 st. 1 commands when the cash
                  cap is reached: the amount goes to the shop's bank account. */}
              <FieldLabel htmlFor="bank-transfer-amount">
                Prenos na račun — uplata na tekući račun prodavnice
              </FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="bank-transfer-amount"
                  inputMode="decimal"
                  value={bankTransferInput}
                  onChange={(event) => setBankTransferInput(event.target.value)}
                />
                <InputGroupAddon align="inline-end">RSD</InputGroupAddon>
              </InputGroup>
            </Field>
          </FieldGroup>

          {assessFailed && (
            <Alert>
              <AlertTitle>Provera limita gotovine nije izvršena</AlertTitle>
              <AlertDescription>
                Provera nije uspela zbog greške u sistemu. Prodaja se može
                završiti.
              </AlertDescription>
            </Alert>
          )}
          {assessment && (amlCheckUnavailable || amlWarns) && (
            <AmlNotice
              assessment={assessment}
              reason={amlReason}
              reasonError={amlReasonError}
              onReasonChange={(value) => {
                setAmlReason(value);
                setAmlReasonError(null);
              }}
            />
          )}
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
              <div className="rounded-md bg-muted p-3 text-sm">
                <p className="font-medium">Izdajte fiskalni račun na ESIR-u</p>
                <p className="text-muted-foreground">
                  Ovaj interni račun ne zamenjuje fiskalni račun.
                </p>
                <div className="mt-2 flex items-end gap-2">
                  <div className="flex flex-col gap-1">
                    <label htmlFor="esir-number" className="text-xs">
                      Broj fiskalnog računa (ESIR)
                    </label>
                    <Input
                      id="esir-number"
                      value={esirNumber}
                      onChange={(e) => setEsirNumberValue(e.target.value)}
                    />
                  </div>
                  <Button
                    type="button"
                    variant="secondary"
                    onClick={() => {
                      void services.receipts.setEsirNumber(
                        completedSale.id,
                        esirNumber,
                      );
                    }}
                  >
                    Sačuvaj broj
                  </Button>
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

interface AmlNoticeProps {
  assessment: AmlAssessment;
  reason: string;
  reasonError: string | null;
  onReasonChange: (value: string) => void;
}

/**
 * The till-side rendering of the AML čl. 46 st. 1 verdict.
 *
 * Legal: every figure and every word of the penalty comes from the backend
 * (`legal.rs`), never from this file — a preduzetnik must never be shown the
 * pravno-lice "privredni prestup" tier. When the legal form is unanswered the
 * penalty is `null` and we point at Podešavanja → Profil instead of guessing.
 */
function AmlNotice({
  assessment,
  reason,
  reasonError,
  onReasonChange,
}: AmlNoticeProps) {
  if (assessment.rateUnavailable) {
    return (
      <Alert>
        <AlertTitle>Provera limita gotovine nije izvršena</AlertTitle>
        <AlertDescription>
          Kurs evra nije poznat, pa provera nije mogla da se izvrši. Unesite ili
          osvežite kurs u Podešavanja → Kurs. Prodaja se može završiti.
        </AlertDescription>
      </Alert>
    );
  }

  const stale =
    assessment.rate !== null && assessment.rate.rateDate !== todayIsoDate();

  return (
    <Alert variant={assessment.breached ? "destructive" : "default"}>
      <AlertTitle>
        {assessment.breached
          ? "Prekoračen limit za gotovinu"
          : "Blizu limita za gotovinu"}
      </AlertTitle>
      <AlertDescription>
        <div className="flex flex-col gap-2">
          <p>{assessment.notice.summary}</p>
          <p>
            Limit: {formatRsd(assessment.thresholdMinor)} · Primljeno u
            gotovini: {formatRsd(assessment.cashMinor)}
          </p>
          {assessment.notice.penalty ? (
            <p>{assessment.notice.penalty}</p>
          ) : (
            <p>
              Unesite pravnu formu u Podešavanja → Profil da bi kazna bila
              prikazana.
            </p>
          )}
          <p className="text-xs">{assessment.notice.citation}</p>
          {stale && assessment.rate && (
            <p className="text-xs">
              Upozorenje: primenjen je kurs od{" "}
              {formatRateDate(assessment.rate.rateDate)}, a ne današnji.
              Osvežite kurs u Podešavanja → Kurs.
            </p>
          )}
          {assessment.breached && (
            <Field data-invalid={Boolean(reasonError)}>
              <FieldLabel htmlFor="aml-reason">
                Razlog prijema gotovine
              </FieldLabel>
              <Textarea
                id="aml-reason"
                value={reason}
                aria-invalid={Boolean(reasonError)}
                onChange={(event) => onReasonChange(event.target.value)}
              />
              <FieldDescription>
                Umesto gotovine ponudite kupcu uplatu na tekući račun
                prodavnice.
              </FieldDescription>
              <FieldError>{reasonError}</FieldError>
            </Field>
          )}
        </div>
      </AlertDescription>
    </Alert>
  );
}

function todayIsoDate(): string {
  const now = new Date();
  const month = `${now.getMonth() + 1}`.padStart(2, "0");
  const day = `${now.getDate()}`.padStart(2, "0");

  return `${now.getFullYear()}-${month}-${day}`;
}

/** `2026-07-01` -> `01.07.2026`, without going through a Date (no TZ shift). */
function formatRateDate(value: string): string {
  const [year, month, day] = value.split("-");

  return year && month && day ? `${day}.${month}.${year}` : value;
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

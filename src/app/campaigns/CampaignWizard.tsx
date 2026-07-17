import { TriangleAlertIcon } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import {
  ANCHOR_TRUNCATED_NOTE,
  displayModeLabels,
  groundLabels,
  STOCK_LABEL,
  typeLabels,
} from "./campaign-copy";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Separator } from "@/components/ui/separator";
import { Textarea } from "@/components/ui/textarea";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { PosServices } from "@/services/ports";
import type {
  CampaignDisplayMode,
  CampaignInput,
  CampaignItemAnchor,
  CampaignType,
  CampaignValidationReport,
  CampaignView,
  CampaignViolation,
  ProductSummary,
  RasprodajaGround,
} from "@/services/types";

interface CampaignWizardProps {
  services: PosServices;
  /** An existing DRAFT to edit, or `null` to start a new one. Never an
   *  activated campaign: its anchor is frozen (čl. 37 st. 5) and
   *  `campaigns_update` is draft-only. */
  campaign: CampaignView | null;
  onClose: () => void;
  onSaved: (view: CampaignView) => void;
}

interface WizardItem {
  productId: number;
  productName: string;
  sku: string;
  price: string;
  manualPrethodna: string;
  justification: string;
  futurePrice: string;
}

/**
 * The plain-language notes are legal summaries, not legal advice, and they are
 * the wizard's only explanation of why a type is refused later. They are
 * reproduced from the plan character-for-character.
 */
const typeOptions: { value: CampaignType; note: string }[] = [
  {
    value: "sezonsko_snizenje",
    note: "Najviše dva puta godišnje, počinje 25.12–10.01. ili 01–15.07, do 60 dana. Brojimo po datumu početka u kalendarskoj godini.",
  },
  {
    value: "akcijska_prodaja",
    note: "Do 31 dan. Do 3 dana može samo procenat umesto dve cene.",
  },
  {
    value: "promotivna_prodaja",
    note: "Samo roba koja se prvi put uvodi u ponudu; do 60 dana; unosi se buduća redovna cena.",
  },
  {
    value: "rasprodaja",
    note: "Samo uz zakonski osnov; roba se fizički izdvaja; bez prijema novih količina.",
  },
];

const SEASON_ATTESTATION = "Potvrđujem da je sezona protekla (čl. 37 st. 8).";
const SEPARATION_ATTESTATION =
  "Potvrđujem da je roba na rasprodaji fizički izdvojena (čl. 37 st. 7).";
const WARNINGS_DO_NOT_BLOCK = "Upozorenja ne blokiraju čuvanje.";

const DEBOUNCE_MS = 200;
const SEARCH_LIMIT = 10;

export function CampaignWizard({
  services,
  campaign,
  onClose,
  onSaved,
}: CampaignWizardProps) {
  const campaignsService = services.campaigns;
  const catalogService = services.catalog;

  const [campaignType, setCampaignType] = useState<CampaignType>(
    campaign?.campaignType ?? "sezonsko_snizenje",
  );
  const [startsOn, setStartsOn] = useState(() => toDateInput(campaign?.startsOn));
  const [endsOn, setEndsOn] = useState(() => toDateInput(campaign?.endsOn));
  const [displayMode, setDisplayMode] = useState<CampaignDisplayMode>(
    campaign?.displayMode ?? "two_prices",
  );
  const [headlinePercent, setHeadlinePercent] = useState(
    campaign?.headlinePercent == null ? "" : String(campaign.headlinePercent),
  );
  const [rasprodajaGround, setRasprodajaGround] = useState<RasprodajaGround | "">(
    campaign?.rasprodajaGround ?? "",
  );
  const [marketingLabel, setMarketingLabel] = useState(
    campaign?.marketingLabel ?? "",
  );
  const [specialConditions, setSpecialConditions] = useState(
    campaign?.specialConditions ?? "",
  );
  const [reducedUtilityReason, setReducedUtilityReason] = useState(
    campaign?.reducedUtilityReason ?? "",
  );
  const [seasonAttested, setSeasonAttested] = useState(
    campaign?.seasonAttested ?? false,
  );
  const [separationAttested, setSeparationAttested] = useState(
    campaign?.separationAttested ?? false,
  );
  const [items, setItems] = useState<WizardItem[]>(() => initialItems(campaign));

  const [search, setSearch] = useState("");
  const [results, setResults] = useState<ProductSummary[]>([]);
  const [searchError, setSearchError] = useState<string | undefined>();

  const [report, setReport] = useState<CampaignValidationReport | null>(null);
  const [validating, setValidating] = useState(false);
  const [validateError, setValidateError] = useState<string | undefined>();
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | undefined>();

  const isPromotivna = campaignType === "promotivna_prodaja";
  const isRasprodaja = campaignType === "rasprodaja";
  const isSezonsko = campaignType === "sezonsko_snizenje";

  /**
   * Mirrors h6 (čl. 37 st. 11) so the option is not offered where it cannot be
   * used. It decides nothing: the backend re-checks h6 on validate and on
   * create, and its verdict is the one that counts.
   */
  const durationDays = declaredDurationDays(startsOn, endsOn);
  const percentageAllowed =
    campaignType === "akcijska_prodaja" &&
    durationDays != null &&
    durationDays >= 1 &&
    durationDays <= 3;

  // Items whose price cannot be read at all: there is no input to validate, so
  // the wizard says so itself rather than sending a fabricated number.
  const unreadablePrices: CampaignViolation[] = items
    .filter((item) => parseMoney(item.price) == null)
    .map((item) => ({
      code: "ui-price",
      message: `Unesite ispravnu cenu u kampanji za ${item.productName}.`,
      productId: item.productId,
    }));

  const input = useMemo(
    () =>
      buildInput({
        campaignType,
        startsOn,
        endsOn,
        displayMode,
        headlinePercent,
        rasprodajaGround,
        marketingLabel,
        specialConditions,
        reducedUtilityReason,
        seasonAttested,
        separationAttested,
        items,
      }),
    [
      campaignType,
      startsOn,
      endsOn,
      displayMode,
      headlinePercent,
      rasprodajaGround,
      marketingLabel,
      specialConditions,
      reducedUtilityReason,
      seasonAttested,
      separationAttested,
      items,
    ],
  );
  const inputKey = input == null ? null : JSON.stringify(input);

  useEffect(() => {
    let cancelled = false;
    const term = search.trim();
    const timer = setTimeout(() => {
      catalogService
        .searchProducts({
          search: term || undefined,
          // Promotivna prodaja is for goods not yet in the offer, so its items
          // are precisely the inactive ones (h7c); every sniženje type needs an
          // active item (h13b). Searching the wrong half would hide every
          // product the user is allowed to pick.
          active: !isPromotivna,
          limit: SEARCH_LIMIT,
        })
        .then((result) => {
          if (!cancelled) {
            setResults(result.items);
            setSearchError(undefined);
          }
        })
        .catch((error) => {
          if (!cancelled) {
            setResults([]);
            setSearchError(errorMessage(error, "Pretraga artikala nije uspela."));
          }
        });
    }, DEBOUNCE_MS);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [catalogService, search, isPromotivna]);

  useEffect(() => {
    if (inputKey == null) {
      setReport(null);
      setValidating(false);
      setValidateError(undefined);
      return;
    }

    let cancelled = false;
    setValidating(true);

    const timer = setTimeout(() => {
      campaignsService
        .validateCampaign(JSON.parse(inputKey) as CampaignInput)
        .then((result) => {
          if (!cancelled) {
            setReport(result);
            setValidateError(undefined);
            setValidating(false);
          }
        })
        .catch((error) => {
          if (!cancelled) {
            setReport(null);
            setValidateError(errorMessage(error, "Provera kampanje nije uspela."));
            setValidating(false);
          }
        });
    }, DEBOUNCE_MS);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [campaignsService, inputKey]);

  const anchors = useMemo(
    () =>
      new Map<number, CampaignItemAnchor>(
        (report?.anchors ?? []).map((anchor) => [anchor.productId, anchor]),
      ),
    [report],
  );
  const hard = input == null ? unreadablePrices : (report?.hard ?? []);
  const warnings = report?.warnings ?? [];
  // Never savable on an unread report: an empty `hard` we have not received is
  // not the same thing as no hard violations.
  const canSave =
    input != null && report != null && report.hard.length === 0 && !validating && !saving;

  function updateItem(productId: number, patch: Partial<WizardItem>) {
    setItems((current) =>
      current.map((item) =>
        item.productId === productId ? { ...item, ...patch } : item,
      ),
    );
  }

  function addItem(product: ProductSummary) {
    setItems((current) =>
      current.some((item) => item.productId === product.id)
        ? current
        : [
            ...current,
            {
              productId: product.id,
              productName: product.name,
              sku: product.sku,
              price: "",
              manualPrethodna: "",
              justification: "",
              futurePrice: "",
            },
          ],
    );
  }

  function removeItem(productId: number) {
    setItems((current) => current.filter((item) => item.productId !== productId));
  }

  async function save() {
    if (input == null) {
      return;
    }

    setSaving(true);
    setSaveError(undefined);

    try {
      const view =
        campaign == null
          ? await campaignsService.createCampaign(input)
          : await campaignsService.updateCampaign(campaign.id, input);

      onSaved(view);
    } catch (error) {
      setSaveError(errorMessage(error, "Nacrt kampanje nije sačuvan."));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
    >
      <DialogContent className="max-h-[85vh] gap-4 overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>
            {campaign == null ? "Nova kampanja" : "Izmena nacrta kampanje"}
          </DialogTitle>
          <DialogDescription>
            Nacrt se ne objavljuje. Cene na kasi menja tek aktivacija, koja
            zamrzava prethodnu cenu (čl. 37 st. 5).
          </DialogDescription>
        </DialogHeader>

        <section className="flex flex-col gap-3" aria-label="Vrsta i osnovni podaci">
          <h3 className="text-sm font-semibold">Vrsta kampanje</h3>
          <RadioGroup
            aria-label="Vrsta kampanje"
            value={campaignType}
            onValueChange={(value) => setCampaignType(value as CampaignType)}
          >
            {typeOptions.map((option) => (
              <Field key={option.value} orientation="horizontal">
                <RadioGroupItem
                  id={`campaign-type-${option.value}`}
                  value={option.value}
                />
                <FieldContent>
                  <FieldLabel
                    htmlFor={`campaign-type-${option.value}`}
                    className="font-normal"
                  >
                    {typeLabels[option.value]}
                  </FieldLabel>
                  <FieldDescription>{option.note}</FieldDescription>
                </FieldContent>
              </Field>
            ))}
          </RadioGroup>

          <Separator />

          <FieldGroup className="grid gap-3 sm:grid-cols-2">
            <Field>
              <FieldLabel htmlFor="campaign-starts-on">Datum početka</FieldLabel>
              <Input
                id="campaign-starts-on"
                type="date"
                value={startsOn}
                onChange={(event) => setStartsOn(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="campaign-ends-on">Datum isteka</FieldLabel>
              <Input
                id="campaign-ends-on"
                type="date"
                value={endsOn}
                placeholder={isRasprodaja ? STOCK_LABEL : undefined}
                onChange={(event) => setEndsOn(event.target.value)}
              />
              {isRasprodaja ? (
                <FieldDescription>
                  Ostavite prazno ako rasprodaja traje {STOCK_LABEL}.
                </FieldDescription>
              ) : null}
            </Field>
            <Field>
              <FieldLabel htmlFor="campaign-display-mode">
                Način isticanja cene
              </FieldLabel>
              <NativeSelect
                id="campaign-display-mode"
                className="w-full"
                value={displayMode}
                onChange={(event) =>
                  setDisplayMode(event.target.value as CampaignDisplayMode)
                }
              >
                <NativeSelectOption value="two_prices">
                  {displayModeLabels.two_prices}
                </NativeSelectOption>
                <NativeSelectOption value="percentage" disabled={!percentageAllowed}>
                  {displayModeLabels.percentage}
                </NativeSelectOption>
              </NativeSelect>
              <FieldDescription>
                Isticanje samo procenta je moguće jedino za akcijsku prodaju do 3
                dana (čl. 37 st. 11).
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="campaign-headline-percent">
                Istaknuti procenat (%)
              </FieldLabel>
              <Input
                id="campaign-headline-percent"
                inputMode="numeric"
                value={headlinePercent}
                onChange={(event) => setHeadlinePercent(event.target.value)}
              />
            </Field>
            {isRasprodaja ? (
              <Field>
                <FieldLabel htmlFor="campaign-ground">
                  Zakonski osnov rasprodaje
                </FieldLabel>
                <NativeSelect
                  id="campaign-ground"
                  className="w-full"
                  value={rasprodajaGround}
                  onChange={(event) =>
                    setRasprodajaGround(event.target.value as RasprodajaGround | "")
                  }
                >
                  <NativeSelectOption value="">Izaberite osnov</NativeSelectOption>
                  {(
                    Object.keys(groundLabels) as RasprodajaGround[]
                  ).map((ground) => (
                    <NativeSelectOption key={ground} value={ground}>
                      {groundLabels[ground]}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </Field>
            ) : null}
            <Field>
              <FieldLabel htmlFor="campaign-marketing-label">
                Marketinška oznaka
              </FieldLabel>
              <Input
                id="campaign-marketing-label"
                value={marketingLabel}
                onChange={(event) => setMarketingLabel(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="campaign-special-conditions">
                Posebni uslovi
              </FieldLabel>
              <Textarea
                id="campaign-special-conditions"
                value={specialConditions}
                onChange={(event) => setSpecialConditions(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="campaign-reduced-utility">
                Razlog umanjene upotrebljivosti
              </FieldLabel>
              <Textarea
                id="campaign-reduced-utility"
                value={reducedUtilityReason}
                onChange={(event) => setReducedUtilityReason(event.target.value)}
              />
            </Field>
          </FieldGroup>

          {isSezonsko ? (
            <Field orientation="horizontal">
              <Checkbox
                id="campaign-season-attested"
                checked={seasonAttested}
                onCheckedChange={(checked) => setSeasonAttested(Boolean(checked))}
              />
              <FieldLabel htmlFor="campaign-season-attested" className="font-normal">
                {SEASON_ATTESTATION}
              </FieldLabel>
            </Field>
          ) : null}
          {isRasprodaja ? (
            <Field orientation="horizontal">
              <Checkbox
                id="campaign-separation-attested"
                checked={separationAttested}
                onCheckedChange={(checked) =>
                  setSeparationAttested(Boolean(checked))
                }
              />
              <FieldLabel
                htmlFor="campaign-separation-attested"
                className="font-normal"
              >
                {SEPARATION_ATTESTATION}
              </FieldLabel>
            </Field>
          ) : null}
        </section>

        <Separator />

        <section className="flex flex-col gap-3" aria-label="Artikli">
          <h3 className="text-sm font-semibold">Artikli</h3>
          <Field>
            <FieldLabel htmlFor="campaign-item-search">Pretraga artikala</FieldLabel>
            <Input
              id="campaign-item-search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
            {isPromotivna ? (
              <FieldDescription>
                Nudi se samo roba koja još nije u ponudi — promotivna prodaja je
                prvo uvođenje (čl. 36 st. 9).
              </FieldDescription>
            ) : null}
          </Field>

          {searchError ? (
            <Alert variant="destructive">
              <AlertDescription>{searchError}</AlertDescription>
            </Alert>
          ) : null}

          {results.length > 0 ? (
            <ul aria-label="Rezultati pretrage" className="flex flex-col gap-1">
              {results.map((product) => (
                <li
                  key={product.id}
                  className="flex items-center justify-between gap-2 rounded-md border border-border px-3 py-2"
                >
                  <div className="flex flex-col">
                    <span className="text-sm">{product.name}</span>
                    <span className="text-xs text-muted-foreground">
                      {product.sku}
                    </span>
                  </div>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={items.some((item) => item.productId === product.id)}
                    onClick={() => addItem(product)}
                  >
                    Dodaj {product.name}
                  </Button>
                </li>
              ))}
            </ul>
          ) : null}

          {items.map((item) => (
            <div
              key={item.productId}
              className="flex flex-col gap-3 rounded-md border border-border p-3"
            >
              <div className="flex items-start justify-between gap-2">
                <div className="flex flex-col">
                  <span className="text-sm font-medium">{item.productName}</span>
                  <span className="text-xs text-muted-foreground">{item.sku}</span>
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => removeItem(item.productId)}
                >
                  Ukloni {item.productName}
                </Button>
              </div>

              <div className="grid gap-3 sm:grid-cols-2">
                <Field>
                  <FieldLabel htmlFor={`campaign-item-price-${item.productId}`}>
                    Cena u kampanji za {item.productName}
                  </FieldLabel>
                  <Input
                    id={`campaign-item-price-${item.productId}`}
                    value={item.price}
                    onChange={(event) =>
                      updateItem(item.productId, { price: event.target.value })
                    }
                  />
                </Field>

                <ItemAnchorFields
                  item={item}
                  isPromotivna={isPromotivna}
                  anchor={anchors.get(item.productId)}
                  onChange={(patch) => updateItem(item.productId, patch)}
                />
              </div>
            </div>
          ))}
        </section>

        <Separator />

        <section className="flex flex-col gap-3" aria-label="Nalaz provere">
          {validateError ? (
            <Alert variant="destructive">
              <AlertTitle>Provera nije izvršena</AlertTitle>
              <AlertDescription>{validateError}</AlertDescription>
            </Alert>
          ) : null}

          {hard.length > 0 ? (
            <div className="flex flex-col gap-2 rounded-md border border-destructive/50 bg-destructive/5 p-3">
              <div className="text-sm font-medium text-destructive">
                Prepreke za čuvanje
              </div>
              <ul aria-label="Prepreke" className="flex flex-col gap-1">
                {hard.map((violation) => (
                  <li
                    key={`${violation.code}-${violation.productId ?? "all"}`}
                    className="text-sm text-destructive"
                  >
                    {violation.message}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}

          {warnings.length > 0 ? (
            <div className="flex flex-col gap-2 rounded-md border border-amber-300 bg-amber-50 p-3 dark:border-amber-900 dark:bg-amber-950/40">
              <div className="flex items-center gap-2 text-sm font-medium text-amber-900 dark:text-amber-200">
                <TriangleAlertIcon className="size-4" aria-hidden="true" />
                Upozorenja
              </div>
              <ul aria-label="Upozorenja" className="flex flex-col gap-1">
                {warnings.map((warning) => (
                  <li
                    key={`${warning.code}-${warning.productId ?? "all"}`}
                    className="text-sm text-amber-900 dark:text-amber-200"
                  >
                    {warning.message}
                  </li>
                ))}
              </ul>
              <p className="text-xs text-amber-800 dark:text-amber-300">
                {WARNINGS_DO_NOT_BLOCK}
              </p>
            </div>
          ) : null}

          {saveError ? (
            <Alert variant="destructive">
              <AlertTitle>Nacrt nije sačuvan</AlertTitle>
              <AlertDescription>{saveError}</AlertDescription>
            </Alert>
          ) : null}
        </section>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose}>
            Odustani
          </Button>
          <Button type="button" disabled={!canSave} onClick={save}>
            Sačuvaj nacrt
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/**
 * The anchor column is the validation report's, never the wizard's. `manual` is
 * the backend saying čl. 37 st. 3 cannot be computed for this item (perishable,
 * or too new / not offered / no history) and a human figure with a
 * justification must stand in its place; `computed` is the st. 3 value itself;
 * promotivna has no anchor at all and declares its post-campaign regular price
 * instead (čl. 36 st. 9).
 */
function ItemAnchorFields({
  item,
  isPromotivna,
  anchor,
  onChange,
}: {
  item: WizardItem;
  isPromotivna: boolean;
  anchor: CampaignItemAnchor | undefined;
  onChange: (patch: Partial<WizardItem>) => void;
}) {
  if (isPromotivna) {
    return (
      <Field>
        <FieldLabel htmlFor={`campaign-item-future-price-${item.productId}`}>
          Buduća redovna cena za {item.productName}
        </FieldLabel>
        <Input
          id={`campaign-item-future-price-${item.productId}`}
          value={item.futurePrice}
          onChange={(event) => onChange({ futurePrice: event.target.value })}
        />
        <FieldDescription>
          Cena koja važi po isteku promotivne prodaje (čl. 36 st. 9).
        </FieldDescription>
      </Field>
    );
  }

  if (anchor?.anchorStatus === "manual") {
    return (
      <div className="flex flex-col gap-3">
        <Field>
          <FieldLabel htmlFor={`campaign-item-manual-anchor-${item.productId}`}>
            Prethodna cena za {item.productName}
          </FieldLabel>
          <Input
            id={`campaign-item-manual-anchor-${item.productId}`}
            value={item.manualPrethodna}
            onChange={(event) => onChange({ manualPrethodna: event.target.value })}
          />
          <FieldDescription>{anchorReasonNote(anchor.anchorReason)}</FieldDescription>
        </Field>
        <Field>
          <FieldLabel htmlFor={`campaign-item-justification-${item.productId}`}>
            Obrazloženje prethodne cene za {item.productName}
          </FieldLabel>
          <Textarea
            id={`campaign-item-justification-${item.productId}`}
            value={item.justification}
            onChange={(event) => onChange({ justification: event.target.value })}
          />
        </Field>
      </div>
    );
  }

  if (anchor?.anchorStatus === "computed" && anchor.prethodnaCenaMinor != null) {
    return (
      <div className="flex flex-col gap-1 text-sm">
        <span>
          {`Prethodna: ${formatRsd(anchor.prethodnaCenaMinor)} (prozor ${
            anchor.anchorWindowDays ?? 0
          } d.)`}
        </span>
        {anchor.anchorTruncated ? (
          <span className="text-xs text-amber-700 dark:text-amber-500">
            {ANCHOR_TRUNCATED_NOTE}
          </span>
        ) : null}
      </div>
    );
  }

  return <span className="text-sm text-muted-foreground">Prethodna cena: —</span>;
}

/** Why the backend refused to compute this anchor — the four reasons it emits. */
function anchorReasonNote(reason: string | null): string {
  switch (reason) {
    case "perishable":
      return "Lako kvarljiva roba — prethodna cena se unosi ručno (čl. 37 st. 3).";
    case "too_new_in_assortment":
      return "Artikal je prekratko u ponudi za računanje prethodne cene (čl. 37 st. 4).";
    case "not_offered_in_window":
      return "Artikal nije bio u ponudi u referentnom prozoru (čl. 37 st. 3).";
    case "no_history":
      return "Nema evidencije cena za referentni prozor (čl. 37 st. 3).";
    default:
      return "Prethodnu cenu unesite ručno uz obrazloženje.";
  }
}

function initialItems(campaign: CampaignView | null): WizardItem[] {
  return (campaign?.items ?? []).map((item) => ({
    productId: item.productId,
    productName: item.productName,
    sku: item.sku,
    price: minorToInput(item.campaignPriceMinor),
    manualPrethodna:
      item.anchorStatus === "manual" && item.prethodnaCenaMinor != null
        ? minorToInput(item.prethodnaCenaMinor)
        : "",
    justification: item.anchorJustification ?? "",
    futurePrice:
      item.futureRegularPriceMinor == null
        ? ""
        : minorToInput(item.futureRegularPriceMinor),
  }));
}

function buildInput(form: {
  campaignType: CampaignType;
  startsOn: string;
  endsOn: string;
  displayMode: CampaignDisplayMode;
  headlinePercent: string;
  rasprodajaGround: RasprodajaGround | "";
  marketingLabel: string;
  specialConditions: string;
  reducedUtilityReason: string;
  seasonAttested: boolean;
  separationAttested: boolean;
  items: WizardItem[];
}): CampaignInput | null {
  const items = [];

  for (const item of form.items) {
    const campaignPriceMinor = parseMoney(item.price);

    // No price, no input: a placeholder would be validated as if the user had
    // typed it, and the report would describe a campaign nobody proposed.
    if (campaignPriceMinor == null) {
      return null;
    }

    items.push({
      productId: item.productId,
      campaignPriceMinor,
      manualPrethodnaMinor: parseMoney(item.manualPrethodna),
      anchorJustification: optionalText(item.justification),
      futureRegularPriceMinor: parseMoney(item.futurePrice),
    });
  }

  return {
    campaignType: form.campaignType,
    startsOn: toRfc3339(form.startsOn),
    endsOn: form.endsOn ? toRfc3339(form.endsOn) : null,
    displayMode: form.displayMode,
    headlinePercent: parseWholePercent(form.headlinePercent),
    rasprodajaGround: form.rasprodajaGround || null,
    specialConditions: optionalText(form.specialConditions),
    reducedUtilityReason: optionalText(form.reducedUtilityReason),
    marketingLabel: optionalText(form.marketingLabel),
    seasonAttested: form.seasonAttested,
    separationAttested: form.separationAttested,
    items,
  };
}

/**
 * `<input type="date">` speaks `YYYY-MM-DD`; the commands speak RFC3339. The
 * statute counts calendar days, so midnight UTC carries no meaning of its own —
 * it is only the format `parse_rfc3339` accepts. An empty date stays empty and
 * comes back as h14a rather than being silently turned into a real instant.
 */
function toRfc3339(date: string): string {
  return date ? `${date}T00:00:00Z` : "";
}

/** Reads the calendar-date component straight off the RFC3339 string; parsing
 *  to a `Date` first would re-project it into the viewer's timezone and can
 *  shift a legally-meaningful date by a day. */
function toDateInput(value: string | null | undefined): string {
  return value ? value.slice(0, 10) : "";
}

/**
 * Mirrors `campaigns::declared_duration_days` — inclusive of both ends, so
 * 01.07–03.07 is 3 days. Used only to hide an option the backend would refuse;
 * it is not the authority on h6.
 */
function declaredDurationDays(startsOn: string, endsOn: string): number | null {
  const start = utcDayNumber(startsOn);
  const end = utcDayNumber(endsOn);

  return start == null || end == null ? null : end - start + 1;
}

function utcDayNumber(date: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);

  if (!match) {
    return null;
  }

  const milliseconds = Date.UTC(
    Number(match[1]),
    Number(match[2]) - 1,
    Number(match[3]),
  );

  return Number.isNaN(milliseconds)
    ? null
    : Math.floor(milliseconds / 86_400_000);
}

function parseMoney(raw: string): number | null {
  try {
    return parseRsdInput(raw);
  } catch {
    return null;
  }
}

function parseWholePercent(raw: string): number | null {
  const trimmed = raw.trim();

  return /^\d{1,3}$/.test(trimmed) ? Number(trimmed) : null;
}

function optionalText(raw: string): string | null {
  return raw.trim() || null;
}

function minorToInput(minorUnits: number): string {
  return formatRsd(minorUnits).replace(" RSD", "");
}

function errorMessage(error: unknown, fallback: string) {
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

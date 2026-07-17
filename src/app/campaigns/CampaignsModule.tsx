import { PlusIcon, TagIcon, TriangleAlertIcon } from "lucide-react";
import { useEffect, useState } from "react";
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
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
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
import {
  ANCHOR_TRUNCATED_NOTE,
  displayModeLabels,
  groundLabels,
  STOCK_LABEL,
  typeLabels,
} from "./campaign-copy";
import { CampaignWizard } from "./CampaignWizard";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { PosServices } from "@/services/ports";
import type {
  CampaignAnchorStatus,
  CampaignItemView,
  CampaignStatus,
  CampaignSummary,
  CampaignView,
  CorrectionReport,
  EndCampaignOverride,
} from "@/services/types";

interface CampaignsModuleProps {
  services: PosServices;
}

const statusLabels: Record<CampaignStatus, string> = {
  draft: "Nacrt",
  active: "Aktivna",
  ended: "Završena",
  cancelled: "Otkazana",
};

const statusVariants: Record<
  CampaignStatus,
  "default" | "secondary" | "outline"
> = {
  draft: "outline",
  active: "default",
  ended: "secondary",
  cancelled: "outline",
};

const anchorStatusLabels: Record<CampaignAnchorStatus, string> = {
  computed: "Izračunata",
  manual: "Ručno uneta",
  none: "—",
};

const OVERDUE_LABEL = "Isteklo — vratite cene";

export function CampaignsModule({ services }: CampaignsModuleProps) {
  const campaignsService = services.campaigns;
  const [rows, setRows] = useState<CampaignSummary[]>([]);
  const [selected, setSelected] = useState<CampaignView | null>(null);
  const [listStatus, setListStatus] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [listError, setListError] = useState<string | undefined>();
  const [detailError, setDetailError] = useState<string | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();
  const [endOpen, setEndOpen] = useState(false);
  const [endPrices, setEndPrices] = useState<Record<number, string>>({});
  const [endError, setEndError] = useState<string | undefined>();
  const [stepProductId, setStepProductId] = useState<number | null>(null);
  const [stepPrice, setStepPrice] = useState("");
  const [stepError, setStepError] = useState<string | undefined>();
  // `null` draft = a new campaign. Mounting the wizard fresh each time is what
  // resets its form, so this pair is the whole of the wizard's lifecycle.
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardDraft, setWizardDraft] = useState<CampaignView | null>(null);
  const [correction, setCorrection] = useState<CorrectionReport | null>(null);
  const [correctionError, setCorrectionError] = useState<string | undefined>();

  useEffect(() => {
    let cancelled = false;

    setListStatus("loading");
    setListError(undefined);

    campaignsService
      .listCampaigns()
      .then((result) => {
        if (!cancelled) {
          setRows(result);
          setListStatus("ready");
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setListError(errorMessage(error, "Učitavanje kampanja nije uspelo."));
          setListStatus("error");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [campaignsService]);

  async function reloadList() {
    try {
      setRows(await campaignsService.listCampaigns());
    } catch (error) {
      setListError(errorMessage(error, "Učitavanje kampanja nije uspelo."));
    }
  }

  async function showDetail(id: number) {
    setDetailError(undefined);
    setActionError(undefined);
    closeStep();

    try {
      setSelected(await campaignsService.getCampaign(id));
    } catch (error) {
      setDetailError(errorMessage(error, "Učitavanje detalja nije uspelo."));
    }
  }

  function applyView(view: CampaignView) {
    setSelected(view);
    void reloadList();
  }

  async function runAction(
    action: () => Promise<CampaignView>,
    fallback: string,
  ) {
    setActionError(undefined);

    try {
      applyView(await action());
    } catch (error) {
      setActionError(errorMessage(error, fallback));
    }
  }

  function openEnd(campaign: CampaignView) {
    setEndPrices(
      Object.fromEntries(
        campaign.items.map((item) => [
          item.productId,
          defaultReturnPrice(campaign, item),
        ]),
      ),
    );
    setEndError(undefined);
    setEndOpen(true);
  }

  async function submitEnd() {
    if (!selected) {
      return;
    }

    let overrides: EndCampaignOverride[];

    try {
      overrides = selected.items.map((item) => ({
        productId: item.productId,
        returnPriceMinor: parseRsdInput(endPrices[item.productId] ?? ""),
      }));
    } catch (error) {
      setEndError(errorMessage(error, "Povratna cena nije ispravna."));
      return;
    }

    try {
      applyView(await campaignsService.endCampaign(selected.id, overrides));
      setEndOpen(false);
    } catch (error) {
      setEndError(errorMessage(error, "Kampanja nije završena."));
    }
  }

  function openWizard(draft: CampaignView | null) {
    setWizardDraft(draft);
    setWizardOpen(true);
  }

  function openStep(item: CampaignItemView) {
    setStepProductId(item.productId);
    setStepPrice(minorToInput(item.campaignPriceMinor));
    setStepError(undefined);
  }

  function closeStep() {
    setStepProductId(null);
    setStepPrice("");
    setStepError(undefined);
  }

  async function submitStep(productId: number) {
    if (!selected) {
      return;
    }

    let priceMinor: number;

    try {
      priceMinor = parseRsdInput(stepPrice);
    } catch (error) {
      setStepError(errorMessage(error, "Cena nije ispravna."));
      return;
    }

    try {
      applyView(
        await campaignsService.adjustItemPrice(
          selected.id,
          productId,
          priceMinor,
        ),
      );
      closeStep();
    } catch (error) {
      setStepError(errorMessage(error, "Nova cena nije sačuvana."));
    }
  }

  async function runExport(
    action: () => Promise<{ path: string }>,
    fallback: string,
  ) {
    try {
      const exported = await action();
      toast.success("Izvezeno", { description: exported.path });
    } catch (error) {
      toast.error("Izvoz nije uspeo", {
        description: errorMessage(error, fallback),
      });
    }
  }

  async function loadCorrection() {
    setCorrectionError(undefined);

    try {
      setCorrection(await campaignsService.correctionReport());
    } catch (error) {
      setCorrectionError(
        errorMessage(error, "Učitavanje ispravki nije uspelo."),
      );
    }
  }

  return (
    <div className="flex flex-1 flex-col gap-4">
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(28rem,0.9fr)]">
      <section className="flex min-w-0 flex-col gap-4">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-base font-semibold">Kampanje</h2>
          <Button type="button" onClick={() => openWizard(null)}>
            <PlusIcon data-icon="inline-start" />
            Nova kampanja
          </Button>
        </div>

        {listStatus === "loading" ? (
          <div className="flex items-center gap-2 rounded-md border border-border p-4 text-sm text-muted-foreground">
            <Spinner aria-hidden="true" />
            Učitavanje kampanja...
          </div>
        ) : listStatus === "error" ? (
          <Alert variant="destructive">
            <AlertTitle>Kampanje nisu učitane</AlertTitle>
            <AlertDescription>{listError}</AlertDescription>
          </Alert>
        ) : rows.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <TagIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>Nema kampanja</EmptyTitle>
              <EmptyDescription>
                Napravite kampanju da biste vodili sniženja i rasprodaje.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Vrsta</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Period</TableHead>
                <TableHead>Artikala</TableHead>
                <TableHead>Oznaka</TableHead>
                <TableHead>Akcije</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.id}>
                  <TableCell>{typeLabels[row.campaignType]}</TableCell>
                  <TableCell>
                    <div className="flex flex-wrap items-center gap-1">
                      <Badge variant={statusVariants[row.status]}>
                        {statusLabels[row.status]}
                      </Badge>
                      {row.overdue ? (
                        <Badge variant="destructive" data-overdue="true">
                          {OVERDUE_LABEL}
                        </Badge>
                      ) : null}
                    </div>
                  </TableCell>
                  <TableCell>{formatPeriod(row.startsOn, row.endsOn)}</TableCell>
                  <TableCell>{row.itemCount}</TableCell>
                  <TableCell>{row.marketingLabel ?? "—"}</TableCell>
                  <TableCell>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => showDetail(row.id)}
                    >
                      Detalji za kampanju #{row.id}
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
          <CampaignDetailPanel
            campaign={selected}
            actionError={actionError}
            stepProductId={stepProductId}
            stepPrice={stepPrice}
            stepError={stepError}
            onStepPriceChange={setStepPrice}
            onOpenStep={openStep}
            onCloseStep={closeStep}
            onSubmitStep={submitStep}
            onActivate={() =>
              runAction(
                () => campaignsService.activateCampaign(selected.id),
                "Kampanja nije aktivirana.",
              )
            }
            onCancel={() =>
              runAction(
                () => campaignsService.cancelCampaign(selected.id),
                "Kampanja nije otkazana.",
              )
            }
            onEdit={() => openWizard(selected)}
            onEnd={() => openEnd(selected)}
            onExportEvidence={() =>
              runExport(
                () => campaignsService.exportEvidence(selected.id),
                "Dokaz o ceni nije izvezen.",
              )
            }
            onExportLabels={() =>
              runExport(
                () => campaignsService.exportLabels(selected.id),
                "Etikete nisu izvezene.",
              )
            }
          />
        ) : (
          <div className="rounded-md border border-border p-4 text-sm text-muted-foreground">
            Izaberite kampanju za detalje.
          </div>
        )}
      </section>
      </div>

      <CorrectionPanel
        report={correction}
        error={correctionError}
        onRefresh={loadCorrection}
        onExport={() =>
          runExport(
            () => campaignsService.exportCorrectionReport(),
            "Izveštaj o ispravkama nije izvezen.",
          )
        }
      />

      <Dialog open={endOpen} onOpenChange={setEndOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Završi kampanju</DialogTitle>
            <DialogDescription>
              Proverite cenu na koju se svaki artikal vraća. Predlog je cena pre
              kampanje, a za promotivnu prodaju buduća redovna cena.
            </DialogDescription>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            {selected?.items.map((item) => (
              <Field key={item.productId}>
                <FieldLabel htmlFor={`return-price-${item.productId}`}>
                  Povratna cena za {item.productName}
                </FieldLabel>
                <Input
                  id={`return-price-${item.productId}`}
                  value={endPrices[item.productId] ?? ""}
                  onChange={(event) =>
                    setEndPrices((current) => ({
                      ...current,
                      [item.productId]: event.target.value,
                    }))
                  }
                />
              </Field>
            ))}
            {endError ? (
              <Alert variant="destructive">
                <AlertDescription>{endError}</AlertDescription>
              </Alert>
            ) : null}
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setEndOpen(false)}
            >
              Odustani
            </Button>
            <Button type="button" onClick={submitEnd}>
              Potvrdi završetak
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {wizardOpen ? (
        <CampaignWizard
          services={services}
          campaign={wizardDraft}
          onClose={() => setWizardOpen(false)}
          onSaved={(view) => {
            setWizardOpen(false);
            applyView(view);
          }}
        />
      ) : null}
    </div>
  );
}

function CampaignDetailPanel({
  campaign,
  actionError,
  stepProductId,
  stepPrice,
  stepError,
  onStepPriceChange,
  onOpenStep,
  onCloseStep,
  onSubmitStep,
  onActivate,
  onCancel,
  onEdit,
  onEnd,
  onExportEvidence,
  onExportLabels,
}: {
  campaign: CampaignView;
  actionError: string | undefined;
  stepProductId: number | null;
  stepPrice: string;
  stepError: string | undefined;
  onStepPriceChange: (value: string) => void;
  onOpenStep: (item: CampaignItemView) => void;
  onCloseStep: () => void;
  onSubmitStep: (productId: number) => void;
  onActivate: () => void;
  onCancel: () => void;
  onEdit: () => void;
  onEnd: () => void;
  onExportEvidence: () => void;
  onExportLabels: () => void;
}) {
  return (
    <section
      aria-label="Detalji kampanje"
      className="flex min-w-0 flex-col gap-4 rounded-md border border-border p-4"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-col gap-1">
          <h3 className="text-base font-semibold">
            {typeLabels[campaign.campaignType]}
          </h3>
          <span className="text-sm text-muted-foreground">
            {campaign.marketingLabel ?? "Bez marketinške oznake"}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-1">
          <Badge variant={statusVariants[campaign.status]}>
            {statusLabels[campaign.status]}
          </Badge>
          {campaign.overdue ? (
            <Badge variant="destructive" data-overdue="true">
              {OVERDUE_LABEL}
            </Badge>
          ) : null}
        </div>
      </div>

      <dl className="grid gap-2 text-sm sm:grid-cols-2">
        <DetailRow label="Period" value={formatPeriod(campaign.startsOn, campaign.endsOn)} />
        <DetailRow label="Prikaz cena" value={displayModeLabels[campaign.displayMode]} />
        <DetailRow
          label="Istaknuti procenat"
          value={
            campaign.headlinePercent == null ? "—" : `${campaign.headlinePercent} %`
          }
        />
        <DetailRow
          label="Zakonski osnov rasprodaje"
          value={
            campaign.rasprodajaGround
              ? groundLabels[campaign.rasprodajaGround]
              : "—"
          }
        />
        <DetailRow label="Posebni uslovi" value={campaign.specialConditions ?? "—"} />
        <DetailRow
          label="Razlog umanjene upotrebljivosti"
          value={campaign.reducedUtilityReason ?? "—"}
        />
        <DetailRow
          label="Izjava o proteku sezone"
          value={campaign.seasonAttested ? "Data" : "Nije data"}
        />
        <DetailRow
          label="Izjava o fizičkom izdvajanju"
          value={campaign.separationAttested ? "Data" : "Nije data"}
        />
        <DetailRow
          label="Aktivirana"
          value={campaign.activatedAt ? formatDateTime(campaign.activatedAt) : "—"}
        />
        <DetailRow
          label="Završena"
          value={campaign.endedAt ? formatDateTime(campaign.endedAt) : "—"}
        />
      </dl>

      {campaign.activatedAt ? (
        <p className="text-xs text-muted-foreground">
          Prethodna cena je zamrznuta na dan aktivacije i ne menja se daljim
          sniženjima (čl. 37 st. 5).
        </p>
      ) : null}

      <Separator />

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Artikal</TableHead>
            <TableHead>Cena u kampanji</TableHead>
            <TableHead>Prethodna cena</TableHead>
            <TableHead>Osnov</TableHead>
            <TableHead>Prozor</TableHead>
            {campaign.status === "active" ? <TableHead>Akcije</TableHead> : null}
          </TableRow>
        </TableHeader>
        <TableBody>
          {campaign.items.map((item) => (
            <TableRow key={item.productId}>
              <TableCell>
                <div className="flex flex-col">
                  <span>{item.productName}</span>
                  <span className="text-xs text-muted-foreground">{item.sku}</span>
                </div>
              </TableCell>
              <TableCell>{formatRsd(item.campaignPriceMinor)}</TableCell>
              <TableCell>
                {item.prethodnaCenaMinor == null
                  ? "—"
                  : formatRsd(item.prethodnaCenaMinor)}
              </TableCell>
              <TableCell>
                <div className="flex flex-col gap-1">
                  <span>{anchorStatusLabels[item.anchorStatus]}</span>
                  {item.futureRegularPriceMinor == null ? null : (
                    <span className="text-xs text-muted-foreground">
                      Buduća redovna cena:{" "}
                      {formatRsd(item.futureRegularPriceMinor)}
                    </span>
                  )}
                </div>
              </TableCell>
              <TableCell>
                <div className="flex flex-col gap-1">
                  <span>
                    {item.anchorWindowDays == null
                      ? "—"
                      : `${item.anchorWindowDays} dana`}
                  </span>
                  {item.anchorTruncated ? (
                    <span className="text-xs text-amber-700 dark:text-amber-500">
                      {ANCHOR_TRUNCATED_NOTE}
                    </span>
                  ) : null}
                </div>
              </TableCell>
              {campaign.status === "active" ? (
                <TableCell>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => onOpenStep(item)}
                  >
                    Nova cena za {item.productName}
                  </Button>
                </TableCell>
              ) : null}
            </TableRow>
          ))}
        </TableBody>
      </Table>

      {campaign.items.some(
        (item) => item.anchorJustification || item.anchorReason,
      ) ? (
        <div className="flex flex-col gap-2 text-sm">
          <h4 className="font-medium">Obrazloženja prethodne cene</h4>
          <ul className="flex flex-col gap-1">
            {campaign.items
              .filter((item) => item.anchorJustification || item.anchorReason)
              .map((item) => (
                <li key={item.productId} className="text-muted-foreground">
                  <span className="text-foreground">{item.productName}:</span>{" "}
                  {item.anchorJustification ?? item.anchorReason}
                </li>
              ))}
          </ul>
        </div>
      ) : null}

      {stepProductId == null ? null : (
        <div className="flex flex-col gap-2 rounded-md border border-border p-3">
          <Field>
            <FieldLabel htmlFor={`step-price-${stepProductId}`}>
              Nova cena za{" "}
              {campaign.items.find((item) => item.productId === stepProductId)
                ?.productName ?? ""}
            </FieldLabel>
            <Input
              id={`step-price-${stepProductId}`}
              value={stepPrice}
              onChange={(event) => onStepPriceChange(event.target.value)}
            />
          </Field>
          {stepError ? (
            <Alert variant="destructive">
              <AlertDescription>{stepError}</AlertDescription>
            </Alert>
          ) : null}
          <div className="flex gap-2">
            <Button type="button" onClick={() => onSubmitStep(stepProductId)}>
              Sačuvaj novu cenu
            </Button>
            <Button type="button" variant="outline" onClick={onCloseStep}>
              Odustani
            </Button>
          </div>
        </div>
      )}

      {campaign.warnings.length > 0 ? (
        <div className="flex flex-col gap-2 rounded-md border border-amber-300 bg-amber-50 p-3 dark:border-amber-900 dark:bg-amber-950/40">
          <div className="flex items-center gap-2 text-sm font-medium text-amber-900 dark:text-amber-200">
            <TriangleAlertIcon className="size-4" aria-hidden="true" />
            Upozorenja
          </div>
          <ul aria-label="Upozorenja" className="flex flex-col gap-1">
            {campaign.warnings.map((warning) => (
              <li
                key={`${warning.code}-${warning.productId ?? "all"}`}
                className="text-sm text-amber-900 dark:text-amber-200"
              >
                {warning.message}
              </li>
            ))}
          </ul>
          <p className="text-xs text-amber-800 dark:text-amber-300">
            Upozorenja ne blokiraju rad sa kampanjom.
          </p>
        </div>
      ) : null}

      {actionError ? (
        <Alert variant="destructive">
          <AlertTitle>Radnja nije izvršena</AlertTitle>
          <AlertDescription>{actionError}</AlertDescription>
        </Alert>
      ) : null}

      <div className="flex flex-wrap gap-2">
        {campaign.status === "draft" ? (
          <>
            <Button type="button" onClick={onActivate}>
              Aktiviraj
            </Button>
            {/* Draft-only, like `campaigns_update` itself: an activated
                campaign's anchor is frozen (čl. 37 st. 5). */}
            <Button type="button" variant="outline" onClick={onEdit}>
              Izmeni
            </Button>
            <Button type="button" variant="outline" onClick={onCancel}>
              Otkaži
            </Button>
          </>
        ) : null}
        {campaign.status === "active" ? (
          <Button type="button" onClick={onEnd}>
            Završi kampanju…
          </Button>
        ) : null}
      </div>

      <Separator />

      <div className="flex flex-col gap-2">
        <h4 className="text-sm font-medium">Dokazi i etikete</h4>
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="outline" onClick={onExportEvidence}>
            Izvezi dokaz o ceni
          </Button>
          <Button type="button" variant="outline" onClick={onExportLabels}>
            Izvezi etikete
          </Button>
        </div>
      </div>
    </section>
  );
}

function CorrectionPanel({
  report,
  error,
  onRefresh,
  onExport,
}: {
  report: CorrectionReport | null;
  error: string | undefined;
  onRefresh: () => void;
  onExport: () => void;
}) {
  return (
    <section
      aria-label="Ispravke etiketa"
      className="flex min-w-0 flex-col gap-4 rounded-md border border-border p-4"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-col gap-1">
          <h3 className="text-base font-semibold">Ispravke etiketa</h3>
          <span className="text-sm text-muted-foreground">
            Aktivne kampanje — etikete koje treba proveriti.
          </span>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="outline" onClick={onRefresh}>
            Osveži ispravke
          </Button>
          <Button type="button" variant="outline" onClick={onExport}>
            Izvezi
          </Button>
        </div>
      </div>

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Ispravke nisu učitane</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {report == null ? (
        <p className="text-sm text-muted-foreground">
          Osvežite da vidite etikete aktivnih kampanja.
        </p>
      ) : report.rows.length === 0 ? (
        <p className="text-sm text-muted-foreground">Nema etiketa za proveru.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Artikal</TableHead>
              <TableHead>Vrsta</TableHead>
              <TableHead>Snižena cena</TableHead>
              <TableHead>Prethodna cena</TableHead>
              <TableHead>Napomena</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {report.rows.map((row) => (
              <TableRow key={`${row.campaignId}-${row.productId}`}>
                <TableCell>
                  <div className="flex flex-col">
                    <span>{row.productName}</span>
                    <span className="text-xs text-muted-foreground">
                      {row.sku}
                    </span>
                  </div>
                </TableCell>
                <TableCell>{typeLabels[row.campaignType]}</TableCell>
                <TableCell>{formatRsd(row.campaignPriceMinor)}</TableCell>
                <TableCell>
                  {row.prethodnaCenaMinor == null
                    ? "—"
                    : formatRsd(row.prethodnaCenaMinor)}
                </TableCell>
                <TableCell
                  className={
                    row.needsAttention ? "font-medium text-destructive" : undefined
                  }
                >
                  {row.attentionReason ?? (row.needsAttention ? "Proveriti" : "—")}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </section>
  );
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/**
 * Mirrors `campaigns::end_campaign`: promotivna returns to its declared future
 * regular price, everything else to the price that was in force before the
 * campaign. Keep the two in step — a divergence would silently propose a return
 * price the backend would not have chosen.
 */
function defaultReturnPrice(
  campaign: CampaignView,
  item: CampaignItemView,
): string {
  const minor =
    campaign.campaignType === "promotivna_prodaja"
      ? item.futureRegularPriceMinor
      : item.preCampaignPriceMinor;

  return minor == null ? "" : minorToInput(minor);
}

function minorToInput(minorUnits: number): string {
  return formatRsd(minorUnits).replace(" RSD", "");
}

/**
 * Formats the calendar-date component straight off the RFC3339 string. Parsing
 * to a `Date` first would re-project the instant into the viewer's timezone and
 * can shift a legally-meaningful start/end date by a day.
 */
function formatCampaignDate(value: string): string {
  const [year, month, day] = value.slice(0, 10).split("-");

  if (!year || !month || !day) {
    return value;
  }

  return `${day}.${month}.${year}.`;
}

function formatPeriod(startsOn: string, endsOn: string | null): string {
  const start = formatCampaignDate(startsOn);

  return endsOn == null
    ? `${start} — ${STOCK_LABEL}`
    : `${start} — ${formatCampaignDate(endsOn)}`;
}

function formatDateTime(value: string): string {
  return new Intl.DateTimeFormat("sr-Latn-RS", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value));
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

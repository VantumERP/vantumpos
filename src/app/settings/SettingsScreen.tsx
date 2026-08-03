import {
  DatabaseBackupIcon,
  PlusIcon,
  RotateCcwIcon,
  SaveIcon,
  ShieldAlertIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { toast } from "sonner";

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
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { PosServices, SettingsService } from "@/services/ports";
import type {
  BackupJob,
  BackupStatus,
  CashDepositCalendar,
  CompanySettings,
  EurRateStatus,
  LegalNotice,
  ReceiptSettings,
  SalesSettings,
  ShopProfile,
  TaxRate,
} from "@/services/types";

import { CenovnikPanel } from "./CenovnikPanel";
import { RetentionPanel } from "./RetentionPanel";
import { ShopProfilePanel } from "./ShopProfilePanel";

interface SettingsScreenProps {
  services: PosServices;
  usersPanel: ReactNode;
}

type LoadState =
  | { status: "loading" }
  | {
      status: "ready";
      company: CompanySettings;
      taxRates: TaxRate[];
      receipt: ReceiptSettings;
      sales: SalesSettings;
      shopProfile: ShopProfile;
      /**
       * `settings_lpfr_notice` — the ZF čl. 6 st. 4 duty with its penalty
       * resolved against the stored legal form. Refetched after every profile
       * save, so the tier on screen can never lag the tier on record.
       */
      lpfrNotice: LegalNotice;
      backupStatus: BackupStatus;
      backupJobs: BackupJob[];
    }
  | { status: "error"; message: string };

type SettingsTab =
  | "company"
  | "profile"
  | "vat"
  | "receipts"
  | "rate"
  | "calendar"
  | "users"
  | "rokovi"
  | "cenovnik"
  | "backup";

export function SettingsScreen({ services, usersPanel }: SettingsScreenProps) {
  const [state, setState] = useState<LoadState>({ status: "loading" });
  const [activeTab, setActiveTab] = useState<SettingsTab>("company");

  const reload = () => {
    setState({ status: "loading" });
    Promise.all([
      services.settings.getCompanySettings(),
      services.settings.listTaxRates(),
      services.settings.getReceiptSettings(),
      services.settings.getSalesSettings(),
      services.settings.getShopProfile(),
      services.settings.getLpfrNotice(),
      services.backup.getBackupStatus(),
      services.backup.listBackupJobs(),
    ])
      .then(
        ([
          company,
          taxRates,
          receipt,
          sales,
          shopProfile,
          lpfrNotice,
          backupStatus,
          backupJobs,
        ]) => {
          setState({
            status: "ready",
            company,
            taxRates,
            receipt,
            sales,
            shopProfile,
            lpfrNotice,
            backupStatus,
            backupJobs,
          });
        },
      )
      .catch((error) => {
        setState({
          status: "error",
          message: errorMessage(error, "Podešavanja nisu učitana."),
        });
      });
  };

  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [services]);

  if (state.status === "loading") {
    return (
      <Badge variant="outline" className="w-fit">
        <Spinner data-icon="inline-start" aria-hidden="true" />
        Učitavanje podešavanja
      </Badge>
    );
  }

  if (state.status === "error") {
    return (
      <Alert variant="destructive">
        <ShieldAlertIcon aria-hidden="true" />
        <AlertTitle>Podešavanja nisu dostupna</AlertTitle>
        <AlertDescription>{state.message}</AlertDescription>
      </Alert>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div
        role="tablist"
        aria-label="Podešavanja"
        className="inline-flex w-fit items-center justify-center gap-1 rounded-lg bg-muted p-1 text-muted-foreground"
      >
        <SettingsTabButton
          active={activeTab === "company"}
          onSelect={() => setActiveTab("company")}
        >
          Radnja
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "profile"}
          onSelect={() => setActiveTab("profile")}
        >
          Profil
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "vat"}
          onSelect={() => setActiveTab("vat")}
        >
          PDV
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "receipts"}
          onSelect={() => setActiveTab("receipts")}
        >
          Računi
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "rate"}
          onSelect={() => setActiveTab("rate")}
        >
          Kurs
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "calendar"}
          onSelect={() => setActiveTab("calendar")}
        >
          Kalendar
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "users"}
          onSelect={() => setActiveTab("users")}
        >
          Korisnici
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "rokovi"}
          onSelect={() => setActiveTab("rokovi")}
        >
          Rokovi čuvanja
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "cenovnik"}
          onSelect={() => setActiveTab("cenovnik")}
        >
          Cenovnik
        </SettingsTabButton>
        <SettingsTabButton
          active={activeTab === "backup"}
          onSelect={() => setActiveTab("backup")}
        >
          Backup
        </SettingsTabButton>
      </div>

      {activeTab === "company" ? (
        <CompanySettingsPanel
          settings={state.company}
          onSave={async (request) => {
            const company = await services.settings.updateCompanySettings(request);
            setState((current) =>
              current.status === "ready" ? { ...current, company } : current,
            );
            toast.success("Podešavanja radnje su sačuvana.");
          }}
        />
      ) : null}

      {activeTab === "profile" ? (
        <ShopProfilePanel
          profile={state.shopProfile}
          lpfrNotice={state.lpfrNotice}
          onOpenRegistry={(url) => services.print.openExternalUrl(url)}
          onSave={async (request) => {
            const shopProfile = await services.settings.updateShopProfile(request);
            // The saved legal form may have changed the tier the čl. 6 st. 4
            // figure is resolved at, so the notice is refetched with it.
            const lpfrNotice = await services.settings.getLpfrNotice();
            setState((current) =>
              current.status === "ready"
                ? { ...current, shopProfile, lpfrNotice }
                : current,
            );
            toast.success("Profil radnje je sačuvan.");
          }}
        />
      ) : null}

      {activeTab === "vat" ? (
        <TaxRatesPanel
          taxRates={state.taxRates}
          onSave={async (request) => {
            const saved = await services.settings.saveTaxRate(request);
            setState((current) => {
              if (current.status !== "ready") {
                return current;
              }

              const exists = current.taxRates.some((rate) => rate.id === saved.id);
              const taxRates = exists
                ? current.taxRates.map((rate) => (rate.id === saved.id ? saved : rate))
                : [saved, ...current.taxRates];

              return { ...current, taxRates };
            });
            toast.success("PDV stopa je sačuvana.");
          }}
        />
      ) : null}

      {activeTab === "receipts" ? (
        <div className="flex flex-col gap-6">
          <ReceiptSettingsPanel
            settings={state.receipt}
            onSave={async (request) => {
              const receipt =
                await services.settings.updateReceiptSettings(request);
              setState((current) =>
                current.status === "ready" ? { ...current, receipt } : current,
              );
              toast.success("Numeracija računa je sačuvana.");
            }}
          />
          <div className="flex items-center justify-between rounded-md border p-4">
            <div className="flex flex-col gap-1">
              <span className="text-sm font-medium">
                Dozvoli prodaju ispod stanja
              </span>
              <span className="text-xs text-muted-foreground">
                Kasir može da proda i kada je stanje na kartici nedovoljno.
              </span>
            </div>
            <Switch
              aria-label="Dozvoli prodaju ispod stanja"
              checked={state.sales.allowOverselling}
              onCheckedChange={async (checked) => {
                const sales = await services.settings.updateSalesSettings({
                  allowOverselling: checked,
                });
                setState((current) =>
                  current.status === "ready" ? { ...current, sales } : current,
                );
                toast.success("Podešavanje prodaje je sačuvano.");
              }}
            />
          </div>
        </div>
      ) : null}

      {activeTab === "rate" ? (
        <div className="flex flex-col gap-4">
          <EurRatePanel settings={services.settings} />
          <AmlAggregationDisclosure />
        </div>
      ) : null}

      {activeTab === "calendar" ? (
        <DepositCalendarPanel settings={services.settings} />
      ) : null}

      {activeTab === "users" ? (
        <div className="flex flex-col gap-4">
          {usersPanel}
          {/*
            The ZZPL surfaces are one click away from the accounts they describe,
            but they are NOT in Podešavanja: the čl. 46 nalog, the čl. 48
            evidencija pristupa and the čl. 52 evidencija povreda are records the
            rukovalac keeps, not settings anybody adjusts, and filing them under
            „Podešavanja“ would suggest they can be turned off.
          */}
          <Alert>
            <AlertTitle>Zaštita podataka o ličnosti</AlertTitle>
            <AlertDescription>
              Nalog za pristup tehničke podrške, evidencija pristupa podacima,
              evidencija povreda podataka i evidencija radnji obrade nalaze se u
              odeljku „Privatnost“ u glavnom meniju. To su evidencije koje
              rukovalac vodi, a ne podešavanja. Rok čuvanja tih evidencija jeste
              podešavanje i nalazi se na kartici „Rokovi čuvanja“.
            </AlertDescription>
          </Alert>
        </div>
      ) : null}

      {/*
        The rok IS a podešavanje, and that is why it sits here rather than under
        „Privatnost“ beside the evidencije. The alert on the Korisnici tab draws
        the line: the čl. 46 nalog, the čl. 48 evidencija pristupa and the čl. 52
        evidencija povreda are records the rukovalac keeps and nothing may turn
        them off, while ZZPL čl. 5 st. 1 tač. 5 leaves the *period* to the shop —
        the shortest defensible default, lengthened when the shop needs longer.
        Req. 6 and req. 22 ask for exactly that to be exposed.
      */}
      {activeTab === "rokovi" ? (
        <RetentionPanel retention={services.retention} />
      ) : null}

      {/*
        Where the published cenovnik goes IS a podešavanje (ZZP čl. 6 st. 2, req.
        15), and it is the only part of SW-12 that is: the file itself is
        republished by the write that moved a price, and the archive is a record
        the shop keeps, not something anybody switches off. The panel loads its
        own data — a shop that has published on every price move for a year has a
        long archive, and no other tab needs it.
      */}
      {activeTab === "cenovnik" ? (
        <CenovnikPanel cenovnik={services.cenovnik} />
      ) : null}

      {activeTab === "backup" ? (
        <BackupPanel
          status={state.backupStatus}
          jobs={state.backupJobs}
          onSaveSettings={async (request) => {
            await services.backup.updateBackupSettings(request);
            const backupStatus = await services.backup.getBackupStatus();
            setState((current) =>
              current.status === "ready" ? { ...current, backupStatus } : current,
            );
            toast.success("Backup podešavanja su sačuvana.");
          }}
          onCreateBackup={async (backupFolder) => {
            const job = await services.backup.createBackup({ backupFolder });
            const backupStatus = await services.backup.getBackupStatus();
            setState((current) =>
              current.status === "ready"
                ? {
                    ...current,
                    backupStatus,
                    backupJobs: [job, ...current.backupJobs],
                  }
                : current,
            );
            toast.success("Backup je napravljen.");
          }}
          onRestore={async (path, confirmationText, passphrase) => {
            const job = await services.backup.restoreBackup({
              path,
              confirmationText,
              passphrase,
            });
            const backupStatus = await services.backup.getBackupStatus();
            setState((current) =>
              current.status === "ready"
                ? {
                    ...current,
                    backupStatus,
                    backupJobs: [job, ...current.backupJobs],
                  }
                : current,
            );
            toast.success("Restore je završen.");
          }}
          onSetPassphrase={async (passphrase) => {
            await services.backup.setBackupPassphrase(passphrase);
            const backupStatus = await services.backup.getBackupStatus();
            setState((current) =>
              current.status === "ready" ? { ...current, backupStatus } : current,
            );
          }}
        />
      ) : null}

      {activeTab === "backup" ? (
        <GoLiveResetCard
          onReset={async (confirmationText) => {
            await services.backup.resetTradingData(confirmationText);
            reload();
            toast.success("Podaci za probu su obrisani.");
          }}
        />
      ) : null}
    </div>
  );
}

function SettingsTabButton({
  active,
  children,
  onSelect,
}: {
  active: boolean;
  children: ReactNode;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      className={[
        "inline-flex h-7 items-center justify-center rounded-md px-3 text-xs font-medium transition-colors",
        active
          ? "bg-secondary text-secondary-foreground"
          : "text-muted-foreground hover:bg-background hover:text-foreground",
      ].join(" ")}
      onClick={onSelect}
    >
      {children}
    </button>
  );
}

function CompanySettingsPanel({
  settings,
  onSave,
}: {
  settings: CompanySettings;
  onSave: (request: CompanySettings) => Promise<void>;
}) {
  const [form, setForm] = useState(settings);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setForm(settings);
  }, [settings]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    if (!form.shopName.trim()) {
      setError("Naziv radnje je obavezan.");
      return;
    }

    setSaving(true);
    try {
      await onSave({
        shopName: form.shopName,
        address: form.address,
        pib: form.pib,
        registrationNumber: form.registrationNumber,
        phone: form.phone,
        logoPath: form.logoPath,
        currency: form.currency,
      });
    } catch (saveError) {
      setError(errorMessage(saveError, "Podešavanja radnje nisu sačuvana."));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle role="heading" aria-level={2}>
          Radnja
        </CardTitle>
        <CardDescription>Podaci koji se koriste na lokalnim računima.</CardDescription>
      </CardHeader>
      <CardContent>
        <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <FieldGroup>
            {error ? <FieldError>{error}</FieldError> : null}
            <div className="grid gap-4 md:grid-cols-2">
              <Field>
                <FieldLabel htmlFor="company-shop-name">Naziv radnje</FieldLabel>
                <Input
                  id="company-shop-name"
                  value={form.shopName}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      shopName: event.target.value,
                    }))
                  }
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="company-pib">PIB</FieldLabel>
                <Input
                  id="company-pib"
                  value={form.pib}
                  inputMode="numeric"
                  onChange={(event) =>
                    setForm((current) => ({ ...current, pib: event.target.value }))
                  }
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="company-registration">Matični broj</FieldLabel>
                <Input
                  id="company-registration"
                  value={form.registrationNumber}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      registrationNumber: event.target.value,
                    }))
                  }
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="company-phone">Telefon</FieldLabel>
                <Input
                  id="company-phone"
                  value={form.phone}
                  onChange={(event) =>
                    setForm((current) => ({ ...current, phone: event.target.value }))
                  }
                />
              </Field>
            </div>
            <Field>
              <FieldLabel htmlFor="company-address">Adresa</FieldLabel>
              <Input
                id="company-address"
                value={form.address}
                onChange={(event) =>
                  setForm((current) => ({ ...current, address: event.target.value }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="company-currency">Valuta</FieldLabel>
              <Input id="company-currency" value={form.currency} disabled />
            </Field>
            <Field>
              <Button type="submit" disabled={saving}>
                {saving ? (
                  <Spinner data-icon="inline-start" aria-hidden="true" />
                ) : (
                  <SaveIcon data-icon="inline-start" />
                )}
                Sačuvaj radnju
              </Button>
            </Field>
          </FieldGroup>
        </form>
      </CardContent>
    </Card>
  );
}

function TaxRatesPanel({
  taxRates,
  onSave,
}: {
  taxRates: TaxRate[];
  onSave: (request: {
    id: number | null;
    name: string;
    rateBasisPoints: number;
    active: boolean;
  }) => Promise<void>;
}) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingRate, setEditingRate] = useState<TaxRate | null>(null);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-3">
          <div>
            <CardTitle role="heading" aria-level={2}>
              PDV stope
            </CardTitle>
            <CardDescription>Stope se deaktiviraju kada više nisu u upotrebi.</CardDescription>
          </div>
          <Button
            type="button"
            onClick={() => {
              setEditingRate(null);
              setDialogOpen(true);
            }}
          >
            <PlusIcon data-icon="inline-start" />
            Nova PDV stopa
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Naziv</TableHead>
              <TableHead>Stopa</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Akcije</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {taxRates.map((rate) => (
              <TableRow key={rate.id}>
                <TableCell>{rate.name}</TableCell>
                <TableCell>{formatBasisPoints(rate.rateBasisPoints)}</TableCell>
                <TableCell>
                  <Badge variant={rate.active ? "secondary" : "outline"}>
                    {rate.active ? "Aktivna" : "Neaktivna"}
                  </Badge>
                </TableCell>
                <TableCell className="text-right">
                  <div className="flex justify-end gap-2">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      aria-label={`Uredi ${rate.name}`}
                      onClick={() => {
                        setEditingRate(rate);
                        setDialogOpen(true);
                      }}
                    >
                      Uredi
                    </Button>
                    {rate.active ? (
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        aria-label={`Deaktiviraj ${rate.name}`}
                        onClick={() =>
                          void onSave({
                            id: rate.id,
                            name: rate.name,
                            rateBasisPoints: rate.rateBasisPoints,
                            active: false,
                          })
                        }
                      >
                        Deaktiviraj
                      </Button>
                    ) : null}
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
      <TaxRateDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        taxRate={editingRate}
        onSave={async (request) => {
          await onSave(request);
          setDialogOpen(false);
        }}
      />
    </Card>
  );
}

function TaxRateDialog({
  open,
  onOpenChange,
  onSave,
  taxRate,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  taxRate: TaxRate | null;
  onSave: (request: {
    id: number | null;
    name: string;
    rateBasisPoints: number;
    active: boolean;
  }) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [rate, setRate] = useState("");
  const [active, setActive] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setName(taxRate?.name ?? "");
      setRate(taxRate ? String(taxRate.rateBasisPoints / 100).replace(".", ",") : "");
      setActive(taxRate?.active ?? true);
      setError(null);
    }
  }, [open, taxRate]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!name.trim()) {
      setError("Naziv PDV stope je obavezan.");
      return;
    }

    const normalizedRate = Number(rate.replace(",", "."));
    if (!Number.isFinite(normalizedRate) || normalizedRate < 0) {
      setError("PDV stopa mora biti ispravan broj.");
      return;
    }

    try {
      await onSave({
        id: taxRate?.id ?? null,
        name: name.trim(),
        rateBasisPoints: Math.round(normalizedRate * 100),
        active,
      });
    } catch (saveError) {
      setError(errorMessage(saveError, "PDV stopa nije sačuvana."));
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>PDV stopa</DialogTitle>
          <DialogDescription>Unesite naziv i procenat PDV stope.</DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <FieldGroup>
            {error ? <FieldError>{error}</FieldError> : null}
            <Field data-invalid={error === "Naziv PDV stope je obavezan."}>
              <FieldLabel htmlFor="tax-rate-name">Naziv</FieldLabel>
              <Input
                id="tax-rate-name"
                value={name}
                aria-invalid={error === "Naziv PDV stope je obavezan."}
                onChange={(event) => setName(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="tax-rate-percent">Stopa (%)</FieldLabel>
              <Input
                id="tax-rate-percent"
                inputMode="decimal"
                value={rate}
                onChange={(event) => setRate(event.target.value)}
              />
            </Field>
            <Field orientation="horizontal">
              <FieldLabel htmlFor="tax-rate-active">Aktivna</FieldLabel>
              <Switch
                id="tax-rate-active"
                checked={active}
                onCheckedChange={setActive}
              />
            </Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Odustani
            </Button>
            <Button type="submit">Sačuvaj PDV stopu</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ReceiptSettingsPanel({
  settings,
  onSave,
}: {
  settings: ReceiptSettings;
  onSave: (request: { prefix: string; nextSequenceNumber: number }) => Promise<void>;
}) {
  const [prefix, setPrefix] = useState(settings.prefix);
  const [nextSequenceNumber, setNextSequenceNumber] = useState(
    String(settings.nextSequenceNumber),
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setPrefix(settings.prefix);
    setNextSequenceNumber(String(settings.nextSequenceNumber));
  }, [settings]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsedSequence = Number(nextSequenceNumber);

    if (!prefix.trim() || !Number.isInteger(parsedSequence) || parsedSequence <= 0) {
      setError("Unesite prefiks i sledeći broj računa.");
      return;
    }

    try {
      await onSave({
        prefix,
        nextSequenceNumber: parsedSequence,
      });
      setError(null);
    } catch (saveError) {
      setError(errorMessage(saveError, "Numeracija nije sačuvana."));
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle role="heading" aria-level={2}>
          Računi
        </CardTitle>
        <CardDescription>Automatski reset nije uključen za MVP.</CardDescription>
      </CardHeader>
      <CardContent>
        <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <FieldGroup>
            {error ? <FieldError>{error}</FieldError> : null}
            <Field>
              <FieldLabel htmlFor="receipt-prefix">Prefiks računa</FieldLabel>
              <Input
                id="receipt-prefix"
                value={prefix}
                onChange={(event) => setPrefix(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="receipt-next">Sledeći broj</FieldLabel>
              <Input
                id="receipt-next"
                inputMode="numeric"
                value={nextSequenceNumber}
                onChange={(event) => setNextSequenceNumber(event.target.value)}
              />
              <FieldDescription>Reset politika: bez automatskog resetovanja.</FieldDescription>
            </Field>
            <Field>
              <Button type="submit">
                <SaveIcon data-icon="inline-start" />
                Sačuvaj numeraciju
              </Button>
            </Field>
          </FieldGroup>
        </form>
      </CardContent>
    </Card>
  );
}

/**
 * The NBS middle rate the AML čl. 46 st. 1 dinar threshold is derived from.
 *
 * This panel is the control the till points at. When no rate has ever been
 * cached, `sales_assess_cash_payment` reports `rateUnavailable` and every AML
 * column on the sale stays NULL — so the operator must be able to *fix* that
 * here, not merely be told about it.
 *
 * Three things it has to keep straight. First, **an unknown rate is an unrun
 * check, never a breach** — the copy says the sale is not blocked, because a
 * dead NBS must not read as something the cashier has to clear before selling.
 * Second, **the statute names neither the rate nor the conversion day**: čl. 46
 * is silent and the "zvanični srednji kurs NBS on the transaction date" rule is
 * imported from čl. 8 st. 1 tač. 2, so it is presented as the reading applied,
 * not as a quoted rule. Third, the manual entry is band-checked backend-side —
 * a tenfold typo would multiply the threshold by ten and silently pass an
 * unlawful cash amount — and this panel never second-guesses that verdict, it
 * renders it.
 *
 * No fine figure appears here. Penalties live in `src-tauri/src/legal.rs`.
 */
function EurRatePanel({ settings }: { settings: SettingsService }) {
  const [status, setStatus] = useState<EurRateStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rateInput, setRateInput] = useState("");
  const [dateInput, setDateInput] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let active = true;

    settings.getEurRate().then(
      (loaded) => {
        if (active) {
          setStatus(loaded);
        }
      },
      (loadError) => {
        if (active) {
          setError(errorMessage(loadError, "Kurs nije učitan."));
        }
      },
    );

    return () => {
      active = false;
    };
  }, [settings]);

  async function run(
    action: () => Promise<EurRateStatus>,
    success: string,
    failure: string,
  ) {
    setBusy(true);
    try {
      setStatus(await action());
      setError(null);
      toast.success(success);
    } catch (actionError) {
      setError(errorMessage(actionError, failure));
    } finally {
      setBusy(false);
    }
  }

  async function saveManual(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    let rateMinor: number;
    try {
      rateMinor = parseRsdInput(rateInput);
    } catch {
      setError("Unesite kurs u obliku 117,23.");
      return;
    }
    if (!dateInput) {
      setError("Unesite datum kursa.");
      return;
    }

    await run(
      () => settings.setManualEurRate(rateMinor, dateInput),
      "Ručni kurs je sačuvan.",
      "Ručni kurs nije sačuvan.",
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          <h2>Kurs evra za proveru gotovine</h2>
        </CardTitle>
        <CardDescription>
          Kurs se koristi samo da bi se izračunao dinarski limit za prijem
          gotovine. Zakon o sprečavanju pranja novca i finansiranja terorizma,
          čl. 46 st. 1; nadzor: tržišna inspekcija (čl. 110 st. 6). Sam čl. 46
          ne imenuje ni kurs ni dan preračuna — primenjuje se zvanični srednji
          kurs Narodne banke Srbije na dan transakcije, po definiciji iz čl. 8
          st. 1 tač. 2 istog zakona.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {error ? (
          <Alert variant="destructive">
            <ShieldAlertIcon aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        <p className="text-xs text-muted-foreground">
          Neuspešno osvežavanje kursa ne blokira prodaju. Račun se može završiti
          i kada kurs nije poznat — u tom slučaju se provera limita gotovine ne
          izvršava i to piše na kasi.
        </p>

        {status === null ? (
          <Badge variant="outline" className="w-fit">
            <Spinner data-icon="inline-start" aria-hidden="true" />
            Učitavanje kursa
          </Badge>
        ) : (
          <>
            {status.rate === null ? (
              <Alert>
                <AlertTitle>Kurs nije poznat</AlertTitle>
                <AlertDescription>
                  Provera limita gotovine ne može da se izvrši dok kurs nije
                  poznat. Osvežite kurs sa NBS-a ili ga unesite ručno. Prodaja
                  nije blokirana.
                </AlertDescription>
              </Alert>
            ) : (
              <div className="flex flex-col gap-1 rounded-md border p-4 text-sm">
                <span className="font-medium">
                  {formatRsd(status.rate.rateMinor)} za 1 EUR
                </span>
                <span className="text-muted-foreground">
                  Datum kursa: {formatRateDate(status.rate.rateDate)}
                </span>
                <span className="text-muted-foreground">
                  Izvor: {status.rate.source === "nbs" ? "NBS" : "ručno"}
                </span>
              </div>
            )}

            {status.rate !== null && status.isStale ? (
              <Alert>
                <AlertTitle>Kurs nije od današnjeg dana</AlertTitle>
                <AlertDescription>
                  Provera je izvršena za {formatRateDate(status.checkedFor)}, a
                  sačuvani kurs nosi datum{" "}
                  {formatRateDate(status.rate.rateDate)}. Osvežite ga sa NBS-a
                  ili unesite današnji kurs ručno; do tada se limit gotovine
                  računa po starijem kursu.
                </AlertDescription>
              </Alert>
            ) : null}

            <div>
              <Button
                type="button"
                variant="outline"
                disabled={busy}
                onClick={() =>
                  void run(
                    () => settings.refreshEurRate(),
                    "Kurs je osvežen.",
                    "Kurs nije osvežen.",
                  )
                }
              >
                <RotateCcwIcon data-icon="inline-start" />
                Osveži kurs sa NBS-a
              </Button>
            </div>

            <Separator />

            <form className="flex flex-col gap-4" onSubmit={saveManual}>
              <FieldGroup className="grid gap-3 md:grid-cols-[12rem_12rem_auto] md:items-end">
                <Field>
                  <FieldLabel htmlFor="manual-eur-rate">
                    Kurs (RSD za 1 EUR)
                  </FieldLabel>
                  <Input
                    id="manual-eur-rate"
                    inputMode="decimal"
                    value={rateInput}
                    onChange={(event) => setRateInput(event.target.value)}
                  />
                  <FieldDescription>
                    Unesite zvanični srednji kurs, na primer 117,23.
                  </FieldDescription>
                </Field>
                <Field>
                  <FieldLabel htmlFor="manual-eur-rate-date">
                    Datum kursa
                  </FieldLabel>
                  <Input
                    id="manual-eur-rate-date"
                    type="date"
                    value={dateInput}
                    onChange={(event) => setDateInput(event.target.value)}
                  />
                </Field>
                <Field>
                  <Button type="submit" disabled={busy}>
                    <SaveIcon data-icon="inline-start" />
                    Sačuvaj ručni kurs
                  </Button>
                </Field>
              </FieldGroup>
            </form>
          </>
        )}
      </CardContent>
    </Card>
  );
}

/** `2026-07-01` -> `01.07.2026`, without going through a Date (no TZ shift). */
function formatRateDate(value: string): string {
  const [year, month, day] = value.split("-");

  return year && month && day ? `${day}.${month}.${year}` : value;
}

/**
 * The written disclosure the one-year aggregation limb needs
 * (`docs/SW11-SW15-VERIFIED-RULES.md` §3 req 5).
 *
 * čl. 46 st. 1 bans the cash acceptance „bez obzira na to da li se radi o
 * jednoj ili više međusobno povezanih gotovinskih transakcija ili jednom ili
 * više ugovora u periodu od godinu dana“. The till only ever sees the sale in
 * front of it: there is no customers table, no buyer tag and no rolling
 * 365-day total, and a customer-identity store without a lawful ZZPL basis
 * would be its own exposure (§5 Q-3). So the gap is **stated in writing** here
 * instead of being left for the owner to discover — the duty binds the shop
 * whether or not the software can compute it.
 *
 * Two things this copy must not do. It must not read as a feature: nothing
 * here may suggest the program watches a buyer over a year, because an owner
 * who believes that stops watching himself. And it must carry no fine figure —
 * penalties live in `src-tauri/src/legal.rs` and are rendered from the
 * backend's `LegalNotice`, so that a preduzetnik is never shown a pravno-lice
 * tier.
 */
function AmlAggregationDisclosure() {
  return (
    <Card>
      <CardHeader>
        <CardTitle role="heading" aria-level={2}>
          Šta provera gotovine ne obuhvata
        </CardTitle>
        <CardDescription>
          Pročitajte pre nego što se oslonite na proveru koja se prikazuje na
          kasi.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Alert>
          <ShieldAlertIcon aria-hidden="true" />
          <AlertTitle>Program ne sabira uplate istog kupca</AlertTitle>
          <AlertDescription>
            <div className="flex flex-col gap-2">
              <p>
                Zabrana prijema gotovine ne odnosi se samo na jednu uplatu. Ona
                važi i kada se radi o više međusobno povezanih gotovinskih
                transakcija, kao i o jednom ili više ugovora u periodu od
                godinu dana.
              </p>
              <p>
                Program proverava isključivo pojedinačnu prodaju koja je u tom
                trenutku na kasi. On ne vodi evidenciju kupaca i ne sabira
                ranije uplate istog kupca, pa povezane uplate ne može ni da
                prepozna ni da ih prikaže.
              </p>
              <p>
                Zakonska obaveza važi za radnju i onda kada je program ne
                proverava. Procenu da li su uplate međusobno povezane donosi
                radnja sama; kod većih iznosa kupcu ponudite uplatu na tekući
                račun.
              </p>
              <p className="text-xs">
                Član 46. stav 1. Zakona o sprečavanju pranja novca i
                finansiranja terorizma.
              </p>
            </div>
          </AlertDescription>
        </Alert>
      </CardContent>
    </Card>
  );
}

/**
 * The calendar the seven-working-day deposit deadline (Zakon 68/2015, čl. 3
 * st. 1) is counted against.
 *
 * Two things this panel must keep straight. First, **"radni dan" is
 * statutorily undefined** — neither the Zakon nor Pravilnik 77/2011 defines it
 * — so whether Saturday counts is an assumption the shop makes, defaulted to
 * counting because that yields the earlier and therefore conservative deadline.
 * Second, the holiday list is **this shop's list**: the app ships the state
 * holidays for the years it knows, but a wrong future holiday pushes a deadline
 * *later*, which is the unsafe direction, so the operator is asked to check it
 * per year rather than told it is authoritative.
 *
 * There is no blagajnički maksimum here and there must never be one: no propis
 * prescribes a cash-on-hand ceiling.
 */
function DepositCalendarPanel({ settings }: { settings: SettingsService }) {
  const [calendar, setCalendar] = useState<CashDepositCalendar | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [day, setDay] = useState("");
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let active = true;

    settings.getCashDepositCalendar().then(
      (loaded) => {
        if (active) {
          setCalendar(loaded);
        }
      },
      (loadError) => {
        if (active) {
          setError(errorMessage(loadError, "Kalendar nije učitan."));
        }
      },
    );

    return () => {
      active = false;
    };
  }, [settings]);

  async function run(
    action: () => Promise<CashDepositCalendar>,
    success: string,
    failure: string,
  ) {
    setBusy(true);
    try {
      setCalendar(await action());
      setError(null);
      toast.success(success);
    } catch (actionError) {
      setError(errorMessage(actionError, failure));
    } finally {
      setBusy(false);
    }
  }

  async function addDay(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!day || !label.trim()) {
      setError("Unesite datum i naziv neradnog dana.");
      return;
    }

    await run(
      () => settings.saveNonWorkingDay(day, label.trim()),
      "Neradni dan je sačuvan.",
      "Neradni dan nije sačuvan.",
    );
    setDay("");
    setLabel("");
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          <h2>Rok za polog gotovine</h2>
        </CardTitle>
        <CardDescription>
          Gotovina primljena po bilo kom osnovu uplaćuje se na tekući račun u
          roku od sedam radnih dana. Zakon o obavljanju plaćanja pravnih lica,
          preduzetnika i fizičkih lica koja ne obavljaju delatnost (Sl. glasnik
          RS, br. 68/2015), čl. 3 st. 1; kazne čl. 7 st. 1 tač. 2) i st. 3.
          Nadzor: Poreska uprava (čl. 6). Ova podešavanja određuju kako se ti
          radni dani broje.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {error ? (
          <Alert variant="destructive">
            <ShieldAlertIcon aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {calendar === null ? (
          <Badge variant="outline" className="w-fit">
            <Spinner data-icon="inline-start" aria-hidden="true" />
            Učitavanje kalendara
          </Badge>
        ) : (
          <>
            <div className="flex items-center justify-between rounded-md border p-4">
              <div className="flex flex-col gap-1">
                <span className="text-sm font-medium">Subota je radni dan</span>
                <span className="text-xs text-muted-foreground">
                  „Radni dan“ nije definisan ni u Zakonu 68/2015 ni u Pravilniku
                  77/2011. Podrazumevano se subota računa, jer tako rok pada
                  ranije.
                </span>
                {calendar.saturdayIsWorking ? null : (
                  <span className="text-xs text-muted-foreground">
                    Subota se ne računa kao radni dan, pa se rok pomera kasnije
                    nego po podrazumevanoj pretpostavci.
                  </span>
                )}
              </div>
              <Switch
                aria-label="Subota je radni dan"
                checked={calendar.saturdayIsWorking}
                disabled={busy}
                onCheckedChange={(checked) =>
                  void run(
                    () => settings.setSaturdayIsWorking(checked),
                    "Pretpostavka o suboti je sačuvana.",
                    "Pretpostavka o suboti nije sačuvana.",
                  )
                }
              />
            </div>

            <div className="flex flex-col gap-2">
              <span className="text-sm font-medium">Neradni dani</span>
              {calendar.days.length === 0 ? (
                <span className="text-xs text-muted-foreground">
                  Lista neradnih dana je prazna — rok se broji samo po nedeljama
                  i po pretpostavci o suboti. Dodajte praznike koje radnja ne
                  radi, jer svaki neradni dan pomera rok kasnije.
                </span>
              ) : (
                <span className="text-xs text-muted-foreground">
                  Lista je pripremljena zaključno sa {calendar.horizonYear}.
                  godinom — proverite listu za svaku godinu i dopunite je, jer
                  se pokretni praznici pomeraju. Dodatni neradni dan pomera rok
                  kasnije, a uklonjen ga vraća ranije.
                </span>
              )}
              <div className="overflow-x-auto rounded-md border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Datum</TableHead>
                      <TableHead>Naziv</TableHead>
                      <TableHead className="w-24" />
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {calendar.days.map((entry) => (
                      <TableRow key={entry.day}>
                        <TableCell>{entry.day}</TableCell>
                        <TableCell>{entry.label}</TableCell>
                        <TableCell>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            disabled={busy}
                            onClick={() =>
                              void run(
                                () => settings.deleteNonWorkingDay(entry.day),
                                "Neradni dan je uklonjen.",
                                "Neradni dan nije uklonjen.",
                              )
                            }
                          >
                            Ukloni
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </div>

            <form className="flex flex-col gap-4" onSubmit={addDay}>
              <FieldGroup className="grid gap-3 md:grid-cols-[12rem_1fr_auto] md:items-end">
                <Field>
                  <FieldLabel htmlFor="non-working-day">Datum</FieldLabel>
                  <Input
                    id="non-working-day"
                    type="date"
                    value={day}
                    onChange={(event) => setDay(event.target.value)}
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="non-working-label">Naziv</FieldLabel>
                  <Input
                    id="non-working-label"
                    value={label}
                    onChange={(event) => setLabel(event.target.value)}
                  />
                </Field>
                <Field>
                  <Button type="submit" disabled={busy}>
                    <PlusIcon data-icon="inline-start" />
                    Dodaj neradni dan
                  </Button>
                </Field>
              </FieldGroup>
            </form>
          </>
        )}
      </CardContent>
    </Card>
  );
}

function BackupPanel({
  status,
  jobs,
  onSaveSettings,
  onCreateBackup,
  onRestore,
  onSetPassphrase,
}: {
  status: BackupStatus;
  jobs: BackupJob[];
  onSaveSettings: (request: {
    backupFolder: string;
    automaticBackupEnabled: boolean;
  }) => Promise<void>;
  onCreateBackup: (backupFolder: string) => Promise<void>;
  onRestore: (
    path: string,
    confirmationText: string,
    passphrase: string | null,
  ) => Promise<void>;
  onSetPassphrase: (passphrase: string) => Promise<void>;
}) {
  const [backupFolder, setBackupFolder] = useState(status.backupFolder);
  const [automaticBackupEnabled, setAutomaticBackupEnabled] = useState(
    status.automaticBackupEnabled,
  );
  const [restorePath, setRestorePath] = useState("");
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [confirmationText, setConfirmationText] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [restorePassphrase, setRestorePassphrase] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setBackupFolder(status.backupFolder);
    setAutomaticBackupEnabled(status.automaticBackupEnabled);
  }, [status]);

  async function saveSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    try {
      await onSaveSettings({ backupFolder, automaticBackupEnabled });
    } catch (saveError) {
      setError(errorMessage(saveError, "Backup podešavanja nisu sačuvana."));
    }
  }

  async function createBackup() {
    setError(null);
    try {
      await onCreateBackup(backupFolder);
    } catch (backupError) {
      setError(errorMessage(backupError, "Backup nije uspeo."));
    }
  }

  async function restore() {
    setError(null);
    try {
      await onRestore(restorePath, confirmationText, restorePassphrase || null);
      setRestoreOpen(false);
      setConfirmationText("");
      setRestorePassphrase("");
    } catch (restoreError) {
      setError(errorMessage(restoreError, "Restore nije uspeo."));
    }
  }

  async function setBackupPassphrase() {
    setError(null);
    try {
      await onSetPassphrase(passphrase);
      setPassphrase("");
      toast.success("Lozinka za šifrovanje je postavljena.");
    } catch (passphraseError) {
      setError(errorMessage(passphraseError, "Lozinka nije postavljena."));
    }
  }

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,28rem)_1fr]">
      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            Status backupa
          </CardTitle>
          <CardDescription>Lokalni backup za ovu kasu.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {status.stale ? (
            <Alert variant="destructive">
              <ShieldAlertIcon aria-hidden="true" />
              <AlertTitle>Backup nije napravljen</AlertTitle>
              <AlertDescription>
                Napravite ručni backup pre završetka rada.
              </AlertDescription>
            </Alert>
          ) : (
            <Alert>
              <DatabaseBackupIcon aria-hidden="true" />
              <AlertTitle>Backup je spreman</AlertTitle>
              <AlertDescription>
                Poslednji backup: {status.lastSuccessfulBackup?.createdAt}
              </AlertDescription>
            </Alert>
          )}

          {error ? (
            <Alert variant="destructive">
              <ShieldAlertIcon aria-hidden="true" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          {!status.encryptionConfigured ? (
            <div
              role="alert"
              className="rounded-md border-2 border-destructive bg-destructive/10 p-3 text-sm text-destructive"
            >
              Rezervne kopije nisu šifrovane — postavite lozinku za šifrovanje.
            </div>
          ) : null}

          <div className="flex flex-col gap-2">
            <label htmlFor="backup-passphrase" className="text-sm font-medium">
              Lozinka za šifrovanje
            </label>
            <Input
              id="backup-passphrase"
              type="password"
              value={passphrase}
              onChange={(event) => setPassphrase(event.target.value)}
            />
            <p className="text-xs text-muted-foreground">
              Ako izgubite lozinku, šifrovane rezervne kopije su NEPOVRATNO
              nečitljive na drugom računaru.
            </p>
            <Button type="button" onClick={() => void setBackupPassphrase()}>
              Postavi lozinku
            </Button>
          </div>

          <Separator />

          <form className="flex flex-col gap-4" onSubmit={saveSettings}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="backup-folder">Backup folder</FieldLabel>
                <Input
                  id="backup-folder"
                  value={backupFolder}
                  onChange={(event) => setBackupFolder(event.target.value)}
                />
              </Field>
              <Field orientation="horizontal">
                <FieldLabel htmlFor="automatic-backup">Automatski backup</FieldLabel>
                <Switch
                  id="automatic-backup"
                  checked={automaticBackupEnabled}
                  onCheckedChange={setAutomaticBackupEnabled}
                />
              </Field>
              <div className="flex flex-wrap gap-2">
                <Button type="submit" variant="outline">
                  <SaveIcon data-icon="inline-start" />
                  Sačuvaj backup podešavanja
                </Button>
                <Button type="button" onClick={() => void createBackup()}>
                  <DatabaseBackupIcon data-icon="inline-start" />
                  Napravi backup
                </Button>
              </div>
            </FieldGroup>
          </form>

          <Separator />

          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="restore-path">Putanja backup fajla</FieldLabel>
              <Input
                id="restore-path"
                value={restorePath}
                onChange={(event) => setRestorePath(event.target.value)}
              />
            </Field>
            <Field>
              <Button
                type="button"
                variant="outline"
                disabled={!restorePath.trim()}
                onClick={() => setRestoreOpen(true)}
              >
                <RotateCcwIcon data-icon="inline-start" />
                Vrati backup
              </Button>
            </Field>
          </FieldGroup>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            Istorija backupa
          </CardTitle>
          <CardDescription>Ručni, automatski i restore poslovi.</CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Tip</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Putanja</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {jobs.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3}>Nema backup poslova.</TableCell>
                </TableRow>
              ) : (
                jobs.map((job) => (
                  <TableRow key={job.id}>
                    <TableCell>{backupTypeLabel(job.backupType)}</TableCell>
                    <TableCell>
                      <Badge variant={job.status === "completed" ? "secondary" : "destructive"}>
                        {job.status === "completed" ? "Završen" : "Neuspešan"}
                      </Badge>
                    </TableCell>
                    <TableCell>{job.path}</TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <AlertDialog open={restoreOpen} onOpenChange={setRestoreOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Potvrdite restore</AlertDialogTitle>
            <AlertDialogDescription>
              Restore zamenjuje trenutne lokalne podatke podacima iz izabranog
              backup fajla.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="restore-confirmation">Potvrda</FieldLabel>
              <Input
                id="restore-confirmation"
                value={confirmationText}
                onChange={(event) => setConfirmationText(event.target.value)}
              />
              <FieldDescription>Unesite VRATI PODATKE.</FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="restore-passphrase">
                Lozinka (za šifrovane kopije)
              </FieldLabel>
              <Input
                id="restore-passphrase"
                type="password"
                value={restorePassphrase}
                onChange={(event) => setRestorePassphrase(event.target.value)}
              />
              <FieldDescription>
                Ostavite prazno za lokalne (nešifrovane) kopije.
              </FieldDescription>
            </Field>
          </FieldGroup>
          <AlertDialogFooter>
            <AlertDialogCancel>Odustani</AlertDialogCancel>
            <AlertDialogAction
              disabled={confirmationText !== "VRATI PODATKE"}
              onClick={() => void restore()}
            >
              Potvrdi restore
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function formatBasisPoints(value: number) {
  return `${(value / 100).toLocaleString("sr-Latn-RS", {
    maximumFractionDigits: 2,
  })}%`;
}

function backupTypeLabel(value: BackupJob["backupType"]) {
  switch (value) {
    case "automatic":
      return "Automatski";
    case "restore":
      return "Restore";
    case "pre_restore":
      return "Pre restore";
    case "manual":
      return "Ručni";
  }
}

function errorMessage(error: unknown, fallback: string) {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  if (error instanceof Error && error.message) {
    return error.message;
  }

  return fallback;
}

function GoLiveResetCard({
  onReset,
}: {
  onReset: (confirmationText: string) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [confirmationText, setConfirmationText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      await onReset(confirmationText);
      setOpen(false);
      setConfirmationText("");
    } catch (caught) {
      setError(errorMessage(caught, "Brisanje nije uspelo."));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="border-destructive/40">
      <CardHeader>
        <CardTitle>Priprema za početak rada</CardTitle>
        <CardDescription>
          Obriši probne račune i vrati brojač računa na 1. Katalog, korisnici i
          podešavanja ostaju. Oznake da je deklaracija proverena se poništavaju;
          proveru ponovite pri prvom prijemu robe. Pravi se sigurnosna kopija
          pre brisanja.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Button
          type="button"
          variant="destructive"
          onClick={() => setOpen(true)}
        >
          Obriši probne podatke
        </Button>
        <AlertDialog open={open} onOpenChange={setOpen}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Obrisati sve probne podatke?</AlertDialogTitle>
              <AlertDialogDescription>
                Ova radnja je nepovratna. Unesite OBRISI PODATKE za potvrdu.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <p className="text-sm text-muted-foreground">
              Zakon zahteva čuvanje evidencija do 10 godina (ZoRač čl. 28; ZPDV
              čl. 47). Pre brisanja se obavezno pravi rezervna kopija — čuvajte je
              trajno. Pravna lica ne smeju uništavati dokumentarni materijal bez
              pismenog odobrenja arhiva.
            </p>
            {/*
              `reset_trading_data` also runs DELETE FROM kalkulacije. The
              kalkulacija is the isprava behind a receipt zaduženje and is
              numbered per poslovna godina, exactly like the KEP it feeds — so
              it is a numbered book the owner is losing, not a by-product of
              „obriši probne račune“, and it gets its own sentence.
            */}
            <p className="text-sm text-muted-foreground">
              Briše se i knjiga kalkulacija. Kalkulacija je isprava koja se
              numeriše po poslovnoj godini, pa bi uz zadržane probne kalkulacije
              prva prava kalkulacija dobila redni broj veći od 1. Zajedno sa njom
              briše se i KEP (evidencija prometa) sa zaključenjima poslovnih
              godina, da probna knjiženja ne bi ušla u pravu knjigu.
            </p>
            <Input
              aria-label="Potvrda brisanja"
              value={confirmationText}
              onChange={(event) => setConfirmationText(event.target.value)}
              placeholder="OBRISI PODATKE"
            />
            {error ? (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            ) : null}
            <AlertDialogFooter>
              <AlertDialogCancel>Odustani</AlertDialogCancel>
              <Button
                type="button"
                variant="destructive"
                disabled={busy || confirmationText !== "OBRISI PODATKE"}
                onClick={submit}
              >
                Obriši
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </CardContent>
    </Card>
  );
}

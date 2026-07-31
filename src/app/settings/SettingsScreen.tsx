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
import type { PosServices, SettingsService } from "@/services/ports";
import type {
  BackupJob,
  BackupStatus,
  CashDepositCalendar,
  CompanySettings,
  ReceiptSettings,
  SalesSettings,
  ShopProfile,
  TaxRate,
} from "@/services/types";

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
      backupStatus: BackupStatus;
      backupJobs: BackupJob[];
    }
  | { status: "error"; message: string };

type SettingsTab =
  | "company"
  | "profile"
  | "vat"
  | "receipts"
  | "calendar"
  | "users"
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
          onOpenRegistry={(url) => services.print.openExternalUrl(url)}
          onSave={async (request) => {
            const shopProfile = await services.settings.updateShopProfile(request);
            setState((current) =>
              current.status === "ready" ? { ...current, shopProfile } : current,
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

      {activeTab === "calendar" ? (
        <DepositCalendarPanel settings={services.settings} />
      ) : null}

      {activeTab === "users" ? usersPanel : null}

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
          roku od sedam radnih dana (Zakon 68/2015, čl. 3 st. 1; nadzor: Poreska
          uprava). Ova podešavanja određuju kako se ti radni dani broje.
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
                  „Radni dan" nije definisan ni u Zakonu 68/2015 ni u Pravilniku
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
              <span className="text-xs text-muted-foreground">
                Lista je pripremljena zaključno sa {calendar.horizonYear}.
                godinom — proverite listu za svaku godinu i dopunite je, jer se
                pokretni praznici pomeraju. Dodatni neradni dan pomera rok
                kasnije, a uklonjen ga vraća ranije.
              </span>
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
          podešavanja ostaju. Pravi se sigurnosna kopija pre brisanja.
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

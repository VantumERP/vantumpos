import {
  AlertCircleIcon,
  DatabaseBackupIcon,
  LogInIcon,
  LogOutIcon,
  PlusIcon,
  StoreIcon,
  UserPlusIcon,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";

import { AppVersion } from "@/app/AppVersion";
import { BackendStatus } from "@/app/BackendStatus";
import { CatalogModule } from "@/app/catalog/CatalogModule";
import { FirstRunTaxSetup } from "@/app/FirstRunTaxSetup";
import { ImportWizard } from "@/app/import/ImportWizard";
import { InventoryScreen } from "@/app/inventory/InventoryScreen";
import {
  foundationCards,
  navigationItems,
  type NavigationItemId,
} from "@/app/navigation";
import { ReportsScreen } from "@/app/reports/ReportsScreen";
import { ReceiptsScreen } from "@/app/ReceiptsScreen";
import { RegisterScreen } from "@/app/register/RegisterScreen";
import { SettingsScreen } from "@/app/settings/SettingsScreen";
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
  CardAction,
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
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Separator } from "@/components/ui/separator";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarSeparator,
  SidebarTrigger,
} from "@/components/ui/sidebar";
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
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { PosServices } from "@/services/ports";
import type {
  AppSession,
  BackupStatus,
  CommandError,
  SaveUserRequest,
  ShiftSummary,
  UserAccount,
  UserRole,
} from "@/services/types";

interface AppShellProps {
  services: PosServices;
}

type SessionState =
  | { status: "loading" }
  | { status: "ready"; session: AppSession | null }
  | { status: "error"; message: string };

const ROLE_LABELS: Record<UserRole, string> = {
  admin: "Admin",
  cashier: "Kasir",
};

export function AppShell({ services }: AppShellProps) {
  const [activeId, setActiveId] = useState<NavigationItemId>("register");
  const [inventoryLedgerProductId, setInventoryLedgerProductId] = useState<
    number | null
  >(null);
  const initialSession = (services as PosServices & {
    initialSession?: AppSession | null;
  }).initialSession;
  const [sessionState, setSessionState] = useState<SessionState>(() =>
    initialSession === undefined
      ? { status: "loading" }
      : { status: "ready", session: initialSession },
  );
  const [companyName, setCompanyName] = useState("VantumPOS");
  const activeItem =
    navigationItems.find((item) => item.id === activeId) ?? navigationItems[0];

  useEffect(() => {
    if (initialSession !== undefined) {
      return;
    }

    let ignore = false;

    services.auth
      .getSession()
      .then((session) => {
        if (!ignore) {
          setSessionState({ status: "ready", session });
        }
      })
      .catch(() => {
        if (!ignore) {
          setSessionState({
            status: "error",
            message: "Sesija nije dostupna. Pokušajte ponovo.",
          });
        }
      });

    return () => {
      ignore = true;
    };
  }, [initialSession, services]);

  useEffect(() => {
    let ignore = false;

    services.settings
      .getCompanySettings()
      .then((company) => {
        if (ignore) {
          return;
        }
        const name = company.shopName.trim();
        setCompanyName(name.length > 0 ? name : "VantumPOS");
      })
      .catch((error) => {
        // Keep the default branding if company settings cannot be read,
        // but surface the failure so it stays diagnosable in dev.
        console.warn(
          "Naziv radnje nije ucitan; koristi se podrazumevani naziv.",
          error,
        );
      });

    return () => {
      ignore = true;
    };
  }, [services]);

  if (sessionState.status === "loading") {
    return (
      <main className="flex min-h-svh items-center justify-center bg-background p-6">
        <Badge variant="outline">
          <Spinner data-icon="inline-start" aria-hidden="true" />
          Učitavanje radne sesije
        </Badge>
      </main>
    );
  }

  if (sessionState.status === "error") {
    return (
      <main className="flex min-h-svh items-center justify-center bg-background p-6">
        <Alert variant="destructive" className="max-w-sm">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Prijava nije spremna</AlertTitle>
          <AlertDescription>{sessionState.message}</AlertDescription>
        </Alert>
      </main>
    );
  }

  if (!sessionState.session) {
    return (
      <LoginScreen
        services={services}
        onLogin={(session) => {
          setSessionState({ status: "ready", session });
          setActiveId(session.currentShift ? "register" : "register");
        }}
      />
    );
  }

  const session = sessionState.session;
  const shiftBadge = session.currentShift ? "Smena otvorena" : "Smena nije otvorena";

  return (
    <TooltipProvider>
      <FirstRunTaxSetup settings={services.settings} role={session.user.role} />
      <SidebarProvider>
        <Sidebar collapsible="icon">
          <SidebarHeader>
            <div className="flex min-w-0 items-center gap-2 p-2">
              <div className="flex size-8 shrink-0 items-center justify-center rounded-md bg-sidebar-primary text-sidebar-primary-foreground">
                <StoreIcon aria-hidden="true" />
              </div>
              <div className="min-w-0 group-data-[collapsible=icon]:hidden">
                <div className="truncate text-sm font-medium">{companyName}</div>
                <div className="truncate text-xs text-sidebar-foreground/70">
                  Lokalna kasa
                </div>
              </div>
            </div>
          </SidebarHeader>
          <SidebarContent>
            <SidebarGroup>
              <SidebarGroupLabel>Rad</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {navigationItems
                    .filter(
                      (item) =>
                        session.user.role === "admin" ||
                        !("adminOnly" in item && item.adminOnly),
                    )
                    .map((item) => {
                      const Icon = item.icon;
                      const isActive = item.id === activeId;

                      return (
                        <SidebarMenuItem key={item.id}>
                          <SidebarMenuButton
                            type="button"
                            tooltip={item.label}
                            isActive={isActive}
                            aria-current={isActive ? "page" : undefined}
                            onClick={() => setActiveId(item.id)}
                          >
                            <Icon aria-hidden="true" />
                            <span>{item.label}</span>
                          </SidebarMenuButton>
                        </SidebarMenuItem>
                      );
                    })}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
          <SidebarSeparator />
          <SidebarFooter>
            <div className="flex flex-col gap-2 group-data-[collapsible=icon]:hidden">
              <div className="truncate px-2 text-sm font-medium">
                {session.user.displayName}
              </div>
              <div className="truncate px-2 text-xs text-sidebar-foreground/70">
                @{session.user.username}
              </div>
              <div className="truncate px-2 text-xs text-sidebar-foreground/70">
                {ROLE_LABELS[session.user.role]}
              </div>
              <AppVersion settings={services.settings} />
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => {
                  void services.auth.logout().finally(() =>
                    setSessionState({ status: "ready", session: null }),
                  );
                }}
              >
                <LogOutIcon data-icon="inline-start" />
                Odjava
              </Button>
            </div>
          </SidebarFooter>
        </Sidebar>
        <SidebarInset>
          <header className="flex min-h-14 shrink-0 items-center gap-3 px-4 py-3">
            <SidebarTrigger />
            <Separator orientation="vertical" className="h-6" />
            <div className="min-w-0 flex-1">
              <h1 className="truncate text-lg font-semibold">
                {activeItem.label}
              </h1>
              <p className="truncate text-xs text-muted-foreground">
                {session.user.displayName} · {ROLE_LABELS[session.user.role]}
              </p>
            </div>
            <Badge variant={session.currentShift ? "secondary" : "outline"}>
              {shiftBadge}
            </Badge>
            <ShellBackupStatus services={services} />
            <BackendStatus services={services} />
          </header>
          <Separator />
          <main className="flex flex-1 flex-col gap-4 p-4">
            {renderModule({
              activeId,
              session,
              services,
              onSessionChange: (nextSession) =>
                setSessionState({ status: "ready", session: nextSession }),
              inventoryLedgerProductId,
              onOpenProductLedger: (productId) => {
                setInventoryLedgerProductId(productId);
                setActiveId("inventory");
              },
              onInventoryLedgerOpened: () => setInventoryLedgerProductId(null),
            })}
          </main>
        </SidebarInset>
        <Toaster />
      </SidebarProvider>
    </TooltipProvider>
  );
}

function renderModule({
  activeId,
  session,
  services,
  onSessionChange,
  inventoryLedgerProductId,
  onOpenProductLedger,
  onInventoryLedgerOpened,
}: {
  activeId: NavigationItemId;
  session: AppSession;
  services: PosServices;
  onSessionChange: (session: AppSession) => void;
  inventoryLedgerProductId: number | null;
  onOpenProductLedger: (productId: number) => void;
  onInventoryLedgerOpened: () => void;
}) {
  if (session.user.role === "cashier" && !session.currentShift) {
    return (
      <OpenShiftScreen
        services={services}
        session={session}
        onOpened={(shift) => onSessionChange({ ...session, currentShift: shift })}
      />
    );
  }

  if (activeId === "register") {
    return session.currentShift ? (
      <RegisterWithShiftPanel
        services={services}
        session={session}
        onClosed={() => onSessionChange({ ...session, currentShift: null })}
      />
    ) : (
      <OpenShiftScreen
        services={services}
        session={session}
        onOpened={(shift) => onSessionChange({ ...session, currentShift: shift })}
      />
    );
  }

  if (activeId === "products") {
    return (
      <CatalogModule services={services} onOpenInventory={onOpenProductLedger} />
    );
  }

  if (activeId === "settings") {
    if (session.user.role !== "admin") {
      return (
        <Alert>
          <AlertTitle>Podešavanja</AlertTitle>
          <AlertDescription>
            Samo administrator može da menja podešavanja.
          </AlertDescription>
        </Alert>
      );
    }

    return (
      <SettingsScreen
        services={services}
        usersPanel={<UsersScreen services={services} currentUser={session.user} />}
      />
    );
  }

  if (activeId === "reports") {
    return (
      <ReportsScreen
        reports={services.reports}
        users={services.users}
        currentUser={session.user}
      />
    );
  }

  if (activeId === "inventory") {
    return (
      <InventoryScreen
        services={services}
        userId={session.user.id}
        initialLedgerProductId={inventoryLedgerProductId}
        onLedgerOpened={onInventoryLedgerOpened}
      />
    );
  }

  if (activeId === "receipts") {
    return <ReceiptsScreen receipts={services.receipts} userId={session.user.id} />;
  }

  if (activeId === "import") {
    return <ImportWizard services={services} />;
  }

  return null;
}

function ShellBackupStatus({ services }: { services: PosServices }) {
  const backupService = (services as Partial<PosServices>).backup;
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let ignore = false;

    if (!backupService) {
      return () => {
        ignore = true;
      };
    }

    backupService
      .getBackupStatus()
      .then((nextStatus) => {
        if (!ignore) {
          setStatus(nextStatus);
          setFailed(false);
        }
      })
      .catch(() => {
        if (!ignore) {
          setFailed(true);
        }
      });

    return () => {
      ignore = true;
    };
  }, [backupService]);

  if (!backupService || (!status && !failed)) {
    return null;
  }

  if (failed || status?.lastFailedBackup) {
    return (
      <Badge variant="destructive">
        <DatabaseBackupIcon data-icon="inline-start" />
        Backup greška
      </Badge>
    );
  }

  const currentStatus = status;

  if (!currentStatus) {
    return null;
  }

  if (currentStatus.stale) {
    return (
      <Badge variant="destructive">
        <DatabaseBackupIcon data-icon="inline-start" />
        Backup kasni
      </Badge>
    );
  }

  return null;
}

function LoginScreen({
  services,
  onLogin,
}: {
  services: PosServices;
  onLogin: (session: AppSession) => void;
}) {
  const [username, setUsername] = useState("");
  const [credential, setCredential] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);

    try {
      const session = await services.auth.login({ username, credential });
      onLogin(session);
    } catch (loginError) {
      setError(errorMessage(loginError, "Prijava nije uspela."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="grid min-h-svh bg-background lg:grid-cols-[minmax(0,26rem)_1fr]">
      <section className="flex min-h-svh flex-col justify-between gap-6 border-e p-6">
        <div className="flex min-w-0 items-center gap-2">
          <div className="flex size-8 shrink-0 items-center justify-center rounded-md bg-primary text-primary-foreground">
            <StoreIcon aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <div className="truncate text-sm font-medium">VantumPOS</div>
            <div className="truncate text-xs text-muted-foreground">
              Lokalna kasa
            </div>
          </div>
        </div>

        <form className="flex flex-col gap-5" onSubmit={handleSubmit}>
          <FieldGroup>
            <div className="flex flex-col gap-1">
              <h1 className="text-2xl font-semibold">Prijava u kasu</h1>
              <p className="text-sm text-muted-foreground">
                Unesite korisničko ime i PIN ili lozinku.
              </p>
            </div>
            {error ? (
              <Alert variant="destructive">
                <AlertCircleIcon aria-hidden="true" />
                <AlertTitle>Prijava nije uspela</AlertTitle>
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            ) : null}
            <Field>
              <FieldLabel htmlFor="login-username">Korisničko ime</FieldLabel>
              <Input
                id="login-username"
                value={username}
                autoComplete="username"
                onChange={(event) => setUsername(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="login-credential">PIN ili lozinka</FieldLabel>
              <Input
                id="login-credential"
                type="password"
                value={credential}
                autoComplete="current-password"
                onChange={(event) => setCredential(event.target.value)}
              />
            </Field>
            <Field>
              <Button type="submit" disabled={submitting}>
                {submitting ? (
                  <Spinner data-icon="inline-start" aria-hidden="true" />
                ) : (
                  <LogInIcon data-icon="inline-start" />
                )}
                Prijavi se
              </Button>
            </Field>
          </FieldGroup>
        </form>

        <BackendStatus services={services} />
      </section>
      <section className="hidden bg-muted/30 p-6 lg:flex lg:flex-col lg:justify-end">
        <div className="grid gap-3">
          {foundationCards.map((card) => {
            const Icon = card.icon;

            return (
              <Card key={card.title} size="sm">
                <CardHeader>
                  <CardTitle>{card.title}</CardTitle>
                  <CardAction>
                    <Icon aria-hidden="true" className="text-muted-foreground" />
                  </CardAction>
                </CardHeader>
                <CardContent>
                  <CardDescription>{card.description}</CardDescription>
                </CardContent>
              </Card>
            );
          })}
        </div>
      </section>
    </main>
  );
}

function OpenShiftScreen({
  services,
  session,
  onOpened,
}: {
  services: PosServices;
  session: AppSession;
  onOpened: (shift: ShiftSummary) => void;
}) {
  const [openingCash, setOpeningCash] = useState("");
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    let openingCashMinor: number;
    try {
      openingCashMinor = parseRsdInput(openingCash);
      if (openingCashMinor < 0) {
        throw new Error("Iznos nije ispravan.");
      }
    } catch (parseError) {
      setError(errorMessage(parseError, "Iznos nije ispravan."));
      return;
    }

    setSubmitting(true);
    try {
      const shift = await services.shifts.openShift({
        openingCashMinor,
        note: note.trim() || null,
      });
      onOpened(shift);
    } catch (shiftError) {
      setError(errorMessage(shiftError, "Smena nije otvorena."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="grid gap-4 lg:grid-cols-[minmax(0,28rem)_1fr]">
      <Card>
        <CardHeader>
          <CardTitle>
            <h2>Otvori smenu</h2>
          </CardTitle>
          <CardDescription>{session.user.displayName}</CardDescription>
        </CardHeader>
        <CardContent>
          <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
            <FieldGroup>
              {error ? (
                <Alert variant="destructive">
                  <AlertCircleIcon aria-hidden="true" />
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              ) : null}
              <Field data-invalid={!!error}>
                <FieldLabel htmlFor="opening-cash">Početni novac</FieldLabel>
                <Input
                  id="opening-cash"
                  inputMode="decimal"
                  value={openingCash}
                  aria-invalid={!!error}
                  onChange={(event) => setOpeningCash(event.target.value)}
                />
                <FieldDescription>Unesite iznos gotovine u fioci.</FieldDescription>
              </Field>
              <Field>
                <FieldLabel htmlFor="opening-note">Napomena</FieldLabel>
                <Textarea
                  id="opening-note"
                  value={note}
                  onChange={(event) => setNote(event.target.value)}
                />
              </Field>
              <Field>
                <Button type="submit" disabled={submitting}>
                  {submitting ? (
                    <Spinner data-icon="inline-start" aria-hidden="true" />
                  ) : (
                    <PlusIcon data-icon="inline-start" />
                  )}
                  Otvori smenu
                </Button>
              </Field>
            </FieldGroup>
          </form>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Status rada</CardTitle>
          <CardDescription>
            Prodaja se može završiti tek kada je smena otvorena.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          <StatusRow label="Korisnik" value={session.user.displayName} />
          <StatusRow label="Uloga" value={ROLE_LABELS[session.user.role]} />
          <StatusRow label="Smena" value="Nije otvorena" />
        </CardContent>
      </Card>
    </section>
  );
}

function RegisterWithShiftPanel({
  services,
  session,
  onClosed,
}: {
  services: PosServices;
  session: AppSession;
  onClosed: () => void;
}) {
  const shift = session.currentShift;

  if (!shift) {
    return null;
  }

  return (
    <section className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(22rem,26rem)]">
      <div className="min-w-0">
        <RegisterScreen services={services} />
      </div>
      <CloseShiftPanel services={services} shift={shift} onClosed={onClosed} />
    </section>
  );
}

function CloseShiftPanel({
  services,
  shift,
  onClosed,
}: {
  services: PosServices;
  shift: ShiftSummary;
  onClosed: () => void;
}) {
  const [countedCash, setCountedCash] = useState("");
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const countedMinor = useMemo(() => {
    try {
      return countedCash.trim() ? parseRsdInput(countedCash) : null;
    } catch {
      return null;
    }
  }, [countedCash]);
  const differenceMinor =
    countedMinor === null ? null : countedMinor - shift.expectedCashMinor;

  function requestClose(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    if (countedMinor === null || countedMinor < 0) {
      setError("Iznos nije ispravan.");
      return;
    }

    setConfirmOpen(true);
  }

  async function closeShift() {
    if (countedMinor === null) {
      return;
    }

    setSubmitting(true);
    try {
      await services.shifts.closeShift({
        shiftId: shift.id,
        countedCashMinor: countedMinor,
        note: note.trim() || null,
      });
      setConfirmOpen(false);
      onClosed();
    } catch (closeError) {
      setError(errorMessage(closeError, "Smena nije zatvorena."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Zatvori smenu</CardTitle>
        <CardDescription>Prebrojte gotovinu pre zatvaranja.</CardDescription>
      </CardHeader>
      <CardContent>
        <form className="flex flex-col gap-4" onSubmit={requestClose}>
          <FieldGroup>
            {error ? (
              <Alert variant="destructive">
                <AlertCircleIcon aria-hidden="true" />
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            ) : null}
            <StatusRow label="Očekivana gotovina" value={formatRsd(shift.expectedCashMinor)} />
            <StatusRow label="Kartice" value={formatRsd(shift.cardSalesMinor)} />
            <Field data-invalid={!!error}>
              <FieldLabel htmlFor="counted-cash">Prebrojana gotovina</FieldLabel>
              <Input
                id="counted-cash"
                inputMode="decimal"
                value={countedCash}
                aria-invalid={!!error}
                onChange={(event) => setCountedCash(event.target.value)}
              />
              {differenceMinor !== null ? (
                <FieldDescription>
                  Razlika: {formatRsd(differenceMinor)}
                </FieldDescription>
              ) : null}
            </Field>
            <Field>
              <FieldLabel htmlFor="closing-note">Napomena</FieldLabel>
              <Textarea
                id="closing-note"
                value={note}
                onChange={(event) => setNote(event.target.value)}
              />
            </Field>
            <Field>
              <Button type="submit">Zatvori smenu</Button>
            </Field>
          </FieldGroup>
        </form>
      </CardContent>
      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Potvrdite zatvaranje smene</AlertDialogTitle>
            <AlertDialogDescription>
              Zatvaranje će upisati prebrojanu gotovinu i razliku za ovu smenu.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={submitting}>Odustani</AlertDialogCancel>
            <AlertDialogAction disabled={submitting} onClick={() => void closeShift()}>
              {submitting ? (
                <Spinner data-icon="inline-start" aria-hidden="true" />
              ) : null}
              Potvrdi zatvaranje
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  );
}

function UsersScreen({
  services,
  currentUser,
}: {
  services: PosServices;
  currentUser: UserAccount;
}) {
  const [users, setUsers] = useState<UserAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingUser, setEditingUser] = useState<UserAccount | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  useEffect(() => {
    let ignore = false;

    setLoading(true);
    services.users
      .listUsers()
      .then((loadedUsers) => {
        if (!ignore) {
          setUsers(loadedUsers);
          setError(null);
        }
      })
      .catch((loadError) => {
        if (!ignore) {
          setError(errorMessage(loadError, "Korisnici nisu učitani."));
        }
      })
      .finally(() => {
        if (!ignore) {
          setLoading(false);
        }
      });

    return () => {
      ignore = true;
    };
  }, [services]);

  if (currentUser.role !== "admin") {
    return (
      <Alert>
        <AlertTitle>Korisnici</AlertTitle>
        <AlertDescription>
          Samo administrator može da uređuje korisnike.
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <section className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Korisnici</h2>
          <p className="text-xs text-muted-foreground">
            Lokalni nalozi za administratore i kasire.
          </p>
        </div>
        <Button
          type="button"
          onClick={() => {
            setEditingUser(null);
            setDialogOpen(true);
          }}
        >
          <UserPlusIcon data-icon="inline-start" />
          Novi korisnik
        </Button>
      </div>

      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon aria-hidden="true" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <Card>
        <CardContent className="p-0">
          {loading ? (
            <div className="p-4">
              <Badge variant="outline">
                <Spinner data-icon="inline-start" aria-hidden="true" />
                Učitavanje korisnika
              </Badge>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Korisnik</TableHead>
                  <TableHead>Uloga</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Poslednja prijava</TableHead>
                  <TableHead>Akcije</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {users.map((user) => (
                  <TableRow key={user.id}>
                    <TableCell>
                      <div className="font-medium">{user.displayName}</div>
                      <div className="text-xs text-muted-foreground">
                        {user.username}
                      </div>
                    </TableCell>
                    <TableCell>{ROLE_LABELS[user.role]}</TableCell>
                    <TableCell>
                      <Badge variant={user.active ? "secondary" : "outline"}>
                        {user.active ? "Aktivan" : "Neaktivan"}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      {user.lastLoginAt ? formatDateTime(user.lastLoginAt) : "Nema"}
                    </TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => {
                            setEditingUser(user);
                            setDialogOpen(true);
                          }}
                        >
                          Uredi
                        </Button>
                        {user.active ? (
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => {
                              void services.users
                                .deactivateUser(user.id)
                                .then(() =>
                                  setUsers((current) =>
                                    current.map((item) =>
                                      item.id === user.id
                                        ? { ...item, active: false }
                                        : item,
                                    ),
                                  ),
                                )
                                .catch((deactivateError) =>
                                  setError(
                                    errorMessage(
                                      deactivateError,
                                      "Korisnik nije deaktiviran.",
                                    ),
                                  ),
                                );
                            }}
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
          )}
        </CardContent>
      </Card>

      <UserDialog
        open={dialogOpen}
        user={editingUser}
        onOpenChange={setDialogOpen}
        onSave={async (request) => {
          const saved = editingUser
            ? await services.users.updateUser(editingUser.id, request)
            : await services.users.createUser(request);

          setUsers((current) =>
            editingUser
              ? current.map((item) => (item.id === saved.id ? saved : item))
              : [saved, ...current],
          );
          setDialogOpen(false);
        }}
      />
    </section>
  );
}

function UserDialog({
  open,
  user,
  onOpenChange,
  onSave,
}: {
  open: boolean;
  user: UserAccount | null;
  onOpenChange: (open: boolean) => void;
  onSave: (request: SaveUserRequest) => Promise<void>;
}) {
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [role, setRole] = useState<UserRole>("cashier");
  const [active, setActive] = useState(true);
  const [pin, setPin] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }

    setUsername(user?.username ?? "");
    setDisplayName(user?.displayName ?? "");
    setRole(user?.role ?? "cashier");
    setActive(user?.active ?? true);
    setPin("");
    setPassword("");
    setError(null);
  }, [open, user]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    if (!username.trim()) {
      setError("Korisničko ime je obavezno.");
      return;
    }

    if (!displayName.trim()) {
      setError("Ime za prikaz je obavezno.");
      return;
    }

    if (!user && !pin.trim() && !password.trim()) {
      setError("Unesite PIN ili lozinku za novog korisnika.");
      return;
    }

    setSubmitting(true);
    try {
      await onSave({
        username: username.trim(),
        displayName: displayName.trim(),
        role,
        active,
        pin: pin.trim() || null,
        password: password.trim() || null,
      });
    } catch (saveError) {
      setError(errorMessage(saveError, "Korisnik nije sačuvan."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{user ? "Uredi korisnika" : "Novi korisnik"}</DialogTitle>
          <DialogDescription>
            Nalog važi samo na ovoj lokalnoj kasi.
          </DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <FieldGroup>
            {error ? <FieldError>{error}</FieldError> : null}
            <Field>
              <FieldLabel htmlFor="user-username">Korisničko ime</FieldLabel>
              <Input
                id="user-username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="user-display-name">Ime za prikaz</FieldLabel>
              <Input
                id="user-display-name"
                value={displayName}
                onChange={(event) => setDisplayName(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="user-role">Uloga</FieldLabel>
              <NativeSelect
                id="user-role"
                value={role}
                className="w-full"
                onChange={(event) => setRole(event.target.value as UserRole)}
              >
                <NativeSelectOption value="cashier">Kasir</NativeSelectOption>
                <NativeSelectOption value="admin">Admin</NativeSelectOption>
              </NativeSelect>
            </Field>
            <Field>
              <FieldLabel htmlFor="user-active">Status</FieldLabel>
              <NativeSelect
                id="user-active"
                value={active ? "active" : "inactive"}
                className="w-full"
                onChange={(event) => setActive(event.target.value === "active")}
              >
                <NativeSelectOption value="active">Aktivan</NativeSelectOption>
                <NativeSelectOption value="inactive">Neaktivan</NativeSelectOption>
              </NativeSelect>
            </Field>
            <Field>
              <FieldLabel htmlFor="user-pin">PIN</FieldLabel>
              <Input
                id="user-pin"
                value={pin}
                inputMode="numeric"
                onChange={(event) => setPin(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="user-password">Lozinka</FieldLabel>
              <Input
                id="user-password"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
            </Field>
          </FieldGroup>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Odustani
            </Button>
            <Button type="submit" disabled={submitting}>
              {submitting ? (
                <Spinner data-icon="inline-start" aria-hidden="true" />
              ) : null}
              Sačuvaj korisnika
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function StatusRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-md border p-3">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-sm font-medium">{value}</span>
    </div>
  );
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat("sr-Latn-RS", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value));
}

function errorMessage(error: unknown, fallback: string) {
  if (isCommandError(error)) {
    return error.message;
  }

  if (error instanceof Error && error.message && !error.message.includes("Error:")) {
    return error.message;
  }

  return fallback;
}

function isCommandError(error: unknown): error is CommandError {
  return (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  );
}

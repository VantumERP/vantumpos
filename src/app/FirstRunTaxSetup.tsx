import { useEffect, useState } from "react";

import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import type { SettingsService } from "@/services/ports";
import type { UserRole } from "@/services/types";

interface FirstRunTaxSetupProps {
  settings: SettingsService;
  role: UserRole;
}

export function FirstRunTaxSetup({ settings, role }: FirstRunTaxSetupProps) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    if (role !== "admin") {
      return;
    }

    let cancelled = false;
    settings
      .listTaxRates()
      .then((rates) => {
        if (!cancelled && rates.length === 0) {
          setOpen(true);
        }
      })
      .catch(() => {
        // Non-blocking: a load failure should not trap the operator in a modal.
      });

    return () => {
      cancelled = true;
    };
  }, [settings, role]);

  async function answer(inVatSystem: boolean) {
    setBusy(true);
    setError(undefined);
    try {
      await settings.seedTaxRates(inVatSystem);
      setOpen(false);
    } catch (caught) {
      setError((caught as { message?: string }).message ?? "Čuvanje nije uspelo.");
    } finally {
      setBusy(false);
    }
  }

  if (!open) {
    return null;
  }

  return (
    <AlertDialog open={open}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Da li ste u PDV sistemu?</AlertDialogTitle>
          <AlertDialogDescription>
            Podesite početne poreske stope. Ovo možete kasnije promeniti u
            Podešavanjima.
          </AlertDialogDescription>
        </AlertDialogHeader>
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <AlertDialogFooter>
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => answer(false)}
          >
            Ne
          </Button>
          <Button type="button" disabled={busy} onClick={() => answer(true)}>
            Da
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

import { AlertCircleIcon, CheckCircle2Icon } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import type { PosServices } from "@/services/ports";
import type { AppHealth } from "@/services/types";

type BackendStatusState =
  | { variant: "loading" }
  | { variant: "ready"; health: AppHealth }
  | { variant: "error"; message: string };

interface BackendStatusProps {
  services: PosServices;
}

export function BackendStatus({ services }: BackendStatusProps) {
  const [status, setStatus] = useState<BackendStatusState>({
    variant: "loading",
  });

  useEffect(() => {
    let ignore = false;

    setStatus({ variant: "loading" });

    services.settings
      .getHealth()
      .then((health) => {
        if (!ignore) {
          setStatus({ variant: "ready", health });
        }
      })
      .catch((error: unknown) => {
        if (ignore) {
          return;
        }

        setStatus({
          variant: "error",
          message:
            error instanceof Error ? error.message : "Backend nije dostupan.",
        });
      });

    return () => {
      ignore = true;
    };
  }, [services]);

  if (status.variant === "loading") {
    return (
      <Badge variant="outline">
        <Spinner data-icon="inline-start" />
        Provera lokalne baze
      </Badge>
    );
  }

  if (status.variant === "error") {
    return (
      <Button variant="destructive" size="sm" type="button">
        <AlertCircleIcon data-icon="inline-start" />
        {status.message}
      </Button>
    );
  }

  if (!status.health.migrated) {
    return (
      <Badge variant="destructive">
        <AlertCircleIcon data-icon="inline-start" />
        Migracije nisu spremne
      </Badge>
    );
  }

  return (
    <Badge variant="secondary">
      <CheckCircle2Icon data-icon="inline-start" />
      Lokalna baza spremna
    </Badge>
  );
}

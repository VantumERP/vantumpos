import { AlertCircleIcon, CheckCircle2Icon } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import type { PosServices } from "@/services/ports";
import type { AppHealth } from "@/services/types";

const BACKEND_UNAVAILABLE_MESSAGE = "Backend nije dostupan.";

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
      .catch(() => {
        if (ignore) {
          return;
        }

        setStatus({
          variant: "error",
          message: BACKEND_UNAVAILABLE_MESSAGE,
        });
      });

    return () => {
      ignore = true;
    };
  }, [services]);

  return (
    <span role="status" aria-live="polite" className="inline-flex">
      {status.variant === "loading" ? (
        <Badge variant="outline">
          <Spinner
            data-icon="inline-start"
            role="presentation"
            aria-hidden="true"
          />
          Provera lokalne baze
        </Badge>
      ) : status.variant === "error" ? (
        <Badge variant="destructive">
          <AlertCircleIcon data-icon="inline-start" />
          {status.message}
        </Badge>
      ) : !status.health.migrated ? (
        <Badge variant="destructive">
          <AlertCircleIcon data-icon="inline-start" />
          Migracije nisu spremne
        </Badge>
      ) : (
        <Badge variant="secondary">
          <CheckCircle2Icon data-icon="inline-start" />
          Lokalna baza spremna
        </Badge>
      )}
    </span>
  );
}

import { useEffect, useState } from "react";

import type { SettingsService } from "@/services/ports";

export function AppVersion({ settings }: { settings: SettingsService }) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    settings
      .getHealth()
      .then((health) => {
        if (!cancelled) setVersion(health.appVersion);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [settings]);

  if (!version) return null;
  return (
    <p className="px-2 text-xs text-sidebar-foreground/60">Verzija {version}</p>
  );
}

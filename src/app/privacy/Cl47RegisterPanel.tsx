import { AlertCircleIcon, FileDownIcon, RefreshCwIcon } from "lucide-react";
import { useEffect, useState } from "react";

import { errorMessage, formatInstant } from "./shared";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Spinner } from "@/components/ui/spinner";
import type { PosServices } from "@/services/ports";
import type { ProcessingActivity } from "@/services/types";

/**
 * The ZZPL čl. 47 evidencija radnji obrade, as a **generated** artefact.
 *
 * Req. 28: it is driven off the app's own configured purposes, recipients,
 * retention rows and čl. 50 measures, and never typed by the operator — a
 * register that can be edited into agreement with whatever the till happens to
 * do documents nothing. That is also why this panel has no field: the only
 * writes it offers are „regenerate“ and „export“.
 *
 * **St. 7's *„čuvaju se trajno“* belongs to this register and to nothing else.**
 * §5 item 6: copying it onto the evidencija pristupa or the evidencija povreda
 * would put the product in permanent breach of storage limitation, so the
 * sentence appears here, once, attached to the record the statute attaches it
 * to.
 *
 * This register is also the hard dependency SW-17 has (Pravilnik 40/2019 čl. 4
 * st. 1): without it the breach obrazac cannot be a complete filing.
 */
const ST_7_TRAJNO =
  "Evidenciju radnji obrade rukovalac čuva trajno — ZZPL čl. 47 st. 7. Taj rok važi za " +
  "ovaj registar i ni za jednu drugu evidenciju u programu.";

const SVRHA_REGISTRA =
  "Registar se generiše iz podešavanja i iz radnji koje program stvarno obavlja, pa ga " +
  "operater ne popunjava ručno. Osvežite ga posle svake izmene podešavanja radnje ili " +
  "rokova čuvanja.";

export function Cl47RegisterPanel({ services }: { services: PosServices }) {
  const [activities, setActivities] = useState<ProcessingActivity[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let ignore = false;

    services.privacy
      .listProcessingActivities()
      .then((loaded) => {
        if (!ignore) {
          setActivities(loaded);
          setError(null);
        }
      })
      .catch((readError) => {
        if (!ignore) {
          setError(errorMessage(readError, "Registar radnji obrade nije učitan."));
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

  async function regenerate() {
    setError(null);
    setBusy(true);
    try {
      setActivities(await services.privacy.generateProcessingActivities());
    } catch (generateError) {
      setError(errorMessage(generateError, "Registar nije osvežen."));
    } finally {
      setBusy(false);
    }
  }

  async function exportRegister() {
    setError(null);
    setBusy(true);
    try {
      const file = await services.privacy.exportProcessingActivities();
      await services.print.openForPrint(file.path);
    } catch (exportError) {
      setError(errorMessage(exportError, "Registar nije izvezen."));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Evidencija radnji obrade</CardTitle>
          <CardDescription>{SVRHA_REGISTRA}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <Alert>
            <AlertTitle>Rok čuvanja registra</AlertTitle>
            <AlertDescription>{ST_7_TRAJNO}</AlertDescription>
          </Alert>

          {error ? (
            <Alert variant="destructive">
              <AlertCircleIcon aria-hidden="true" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              disabled={busy}
              onClick={() => void regenerate()}
            >
              {busy ? (
                <Spinner data-icon="inline-start" aria-hidden="true" />
              ) : (
                <RefreshCwIcon data-icon="inline-start" />
              )}
              Osveži registar
            </Button>
            <Button type="button" disabled={busy} onClick={() => void exportRegister()}>
              <FileDownIcon data-icon="inline-start" />
              Izvezi registar
            </Button>
          </div>

          {loading ? (
            <Badge variant="outline">
              <Spinner data-icon="inline-start" aria-hidden="true" />
              Učitavanje registra
            </Badge>
          ) : activities.length === 0 ? (
            <Alert>
              <AlertDescription>
                Registar još nije generisan. Pokrenite „Osveži registar“.
              </AlertDescription>
            </Alert>
          ) : (
            <ul className="flex flex-col gap-3">
              {activities.map((activity) => (
                <li key={activity.id}>
                  <Card size="sm">
                    <CardHeader>
                      <CardTitle>{activity.svrhaObrade}</CardTitle>
                      <CardDescription>
                        Poslednja izmena: {formatInstant(activity.updatedAt)}
                      </CardDescription>
                    </CardHeader>
                    <CardContent>
                      <dl className="grid gap-2 text-sm sm:grid-cols-[14rem_minmax(0,1fr)]">
                        <dt className="text-muted-foreground">
                          Rukovalac (t. 1)
                        </dt>
                        <dd>
                          {activity.rukovalacNaziv}
                          {activity.rukovalacKontakt
                            ? ` · ${activity.rukovalacKontakt}`
                            : ""}
                        </dd>
                        <dt className="text-muted-foreground">
                          Vrsta lica (t. 3)
                        </dt>
                        <dd>{activity.vrstaLica}</dd>
                        <dt className="text-muted-foreground">
                          Vrsta podataka (t. 3)
                        </dt>
                        <dd>{activity.vrstaPodataka}</dd>
                        <dt className="text-muted-foreground">
                          Vrsta primalaca (t. 4)
                        </dt>
                        <dd>{activity.vrstaPrimalaca ?? "—"}</dd>
                        <dt className="text-muted-foreground">
                          Prenos u druge države (t. 5)
                        </dt>
                        <dd>
                          {activity.prenosUDrugeDrzave ?? "—"}
                          {activity.mereZastitePrenosa
                            ? ` ${activity.mereZastitePrenosa}`
                            : ""}
                        </dd>
                        <dt className="text-muted-foreground">
                          Rok čuvanja (t. 6)
                        </dt>
                        <dd>{activity.rokCuvanja ?? "—"}</dd>
                        <dt className="text-muted-foreground">
                          Mere zaštite (t. 7)
                        </dt>
                        <dd>{activity.opisMeraZastite ?? "—"}</dd>
                      </dl>
                    </CardContent>
                  </Card>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </section>
  );
}

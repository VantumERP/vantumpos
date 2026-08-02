import { AlertCircleIcon, KeyRoundIcon, ShieldCheckIcon } from "lucide-react";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";

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
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import type { PosServices } from "@/services/ports";
import type { SupportSession } from "@/services/types";

/**
 * The ZZPL čl. 46 nalog za pristup tehničke podrške.
 *
 * **This is the legal half of SW-10 and the copy must keep it that way.** Čl. 46
 * is penalised (čl. 95 st. 1 t. 23) and it makes the rukovalac's nalog the
 * condition on which an obrađivač — including the individual support engineer —
 * may process the shop's data at all. The evidencija pristupa beside it is the
 * prudential half, so nothing on this surface presents a log as the duty, and
 * čl. 50 is not mentioned here at all: naming an unpenalised article beside a
 * penalised one invites the reader to merge them.
 *
 * Four facts ARE the nalog and all four are on screen: who signed it, when, what
 * obim it authorises and until when. The obim is free text because a nalog says
 * what the vlasnik authorised in the vlasnik's own words; the trajanje is
 * integer minutes, like every other duration in this app.
 */
const PODRAZUMEVANO_TRAJANJE_MINUTA = 60;

export function SupportApprovalPanel({ services }: { services: PosServices }) {
  const [session, setSession] = useState<SupportSession | null>(null);
  const [loading, setLoading] = useState(true);
  const [scope, setScope] = useState("");
  const [duration, setDuration] = useState(String(PODRAZUMEVANO_TRAJANJE_MINUTA));
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    let ignore = false;

    services.privacy
      .activeSupportSession()
      .then((active) => {
        if (!ignore) {
          setSession(active);
          setError(null);
        }
      })
      .catch((readError) => {
        if (!ignore) {
          setError(errorMessage(readError, "Stanje naloga nije učitano."));
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

  async function issue(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    const obim = scope.trim();
    if (!obim) {
      setError(
        "Upišite šta se tehničkoj podršci odobrava — nalog bez obima ne odobrava ništa.",
      );
      return;
    }

    const minutes = Number.parseInt(duration, 10);
    if (!Number.isInteger(minutes) || minutes <= 0) {
      setError("Trajanje naloga unesite u punim minutima, kao ceo broj veći od nule.");
      return;
    }

    setSubmitting(true);
    try {
      const granted = await services.privacy.grantSupportAccess(obim, minutes);
      setSession(granted);
      setScope("");
      setDuration(String(PODRAZUMEVANO_TRAJANJE_MINUTA));
    } catch (grantError) {
      setError(errorMessage(grantError, "Nalog nije izdat."));
    } finally {
      setSubmitting(false);
    }
  }

  async function end() {
    setError(null);
    setSubmitting(true);
    try {
      await services.privacy.endSupportSession();
      setSession(null);
    } catch (endError) {
      setError(errorMessage(endError, "Nalog nije okončan."));
    } finally {
      setSubmitting(false);
    }
  }

  async function enter() {
    setError(null);
    setSubmitting(true);
    try {
      setSession(await services.privacy.enterSupportSession());
    } catch (enterError) {
      setError(errorMessage(enterError, "Ulazak u sesiju nije evidentiran."));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Nalog za pristup tehničke podrške</CardTitle>
          <CardDescription>
            Obrađivač i svako drugo lice ovlašćeno za pristup smeju da obrađuju
            podatke rukovaoca samo po nalogu rukovaoca — Zakon o zaštiti podataka
            o ličnosti, čl. 46. Ovaj zapis je taj nalog.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {error ? (
            <Alert variant="destructive">
              <AlertCircleIcon aria-hidden="true" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          {loading ? (
            <Badge variant="outline">
              <Spinner data-icon="inline-start" aria-hidden="true" />
              Učitavanje naloga
            </Badge>
          ) : session ? (
            <div className="flex flex-col gap-3">
              <Alert>
                <ShieldCheckIcon aria-hidden="true" />
                <AlertTitle>Nalog važi</AlertTitle>
                <AlertDescription>
                  Tehnička podrška sme da pristupi podacima samo u okviru
                  upisanog obima i samo do isteka roka.
                </AlertDescription>
              </Alert>
              <dl className="grid gap-2 text-sm sm:grid-cols-[10rem_minmax(0,1fr)]">
                <dt className="text-muted-foreground">Nalog izdao</dt>
                <dd>{session.grantedByName}</dd>
                <dt className="text-muted-foreground">Izdat</dt>
                <dd>{formatInstant(session.grantedAt)}</dd>
                <dt className="text-muted-foreground">Obim pristupa</dt>
                <dd>{session.scope}</dd>
                <dt className="text-muted-foreground">Važi do</dt>
                <dd>{formatInstant(session.expiresAt)}</dd>
                <dt className="text-muted-foreground">Podrška ušla u sesiju</dt>
                <dd>
                  {session.startedAt
                    ? formatInstant(session.startedAt)
                    : "Nije ušla"}
                </dd>
              </dl>
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  variant="destructive"
                  disabled={submitting}
                  onClick={() => void end()}
                >
                  {submitting ? (
                    <Spinner data-icon="inline-start" aria-hidden="true" />
                  ) : null}
                  Okončaj nalog
                </Button>
                {session.startedAt ? null : (
                  <Button
                    type="button"
                    variant="outline"
                    disabled={submitting}
                    onClick={() => void enter()}
                  >
                    <KeyRoundIcon data-icon="inline-start" />
                    Tehnička podrška je ušla u sesiju
                  </Button>
                )}
              </div>
            </div>
          ) : (
            <Alert>
              <AlertTitle>Pristup nije odobren</AlertTitle>
              <AlertDescription>
                Nema izdatog naloga za pristup tehničke podrške. Dok nalog ne
                postoji, tehnička podrška ne sme da obrađuje podatke iz ove kase.
              </AlertDescription>
            </Alert>
          )}

          <Separator />

          <form className="flex flex-col gap-4" onSubmit={issue}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="support-scope">Obim pristupa</FieldLabel>
                <Textarea
                  id="support-scope"
                  value={scope}
                  disabled={submitting}
                  onChange={(event) => setScope(event.target.value)}
                />
                <FieldDescription>
                  Upišite svojim rečima šta se odobrava, na primer „pregled
                  greške na štampi fiskalnog isečka“.
                </FieldDescription>
              </Field>
              <Field>
                <FieldLabel htmlFor="support-duration">
                  Trajanje naloga (minuta)
                </FieldLabel>
                <Input
                  id="support-duration"
                  inputMode="numeric"
                  value={duration}
                  disabled={submitting}
                  onChange={(event) => setDuration(event.target.value)}
                />
                <FieldDescription>
                  Nalog se izdaje za jednu sesiju. Po isteku roka prestaje da
                  važi i mora da se izda novi.
                </FieldDescription>
              </Field>
              <Field>
                <Button type="submit" disabled={submitting}>
                  {submitting ? (
                    <Spinner data-icon="inline-start" aria-hidden="true" />
                  ) : (
                    <ShieldCheckIcon data-icon="inline-start" />
                  )}
                  Izdaj nalog
                </Button>
              </Field>
            </FieldGroup>
          </form>
        </CardContent>
      </Card>
    </section>
  );
}

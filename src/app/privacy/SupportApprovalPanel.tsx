import {
  AlertCircleIcon,
  EyeIcon,
  KeyRoundIcon,
  ShieldCheckIcon,
} from "lucide-react";
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
import type { SupportSession, UserAccount } from "@/services/types";

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

/**
 * SW-14 req. 28's unmask, and it lives here rather than on the working-time
 * screen for one reason: it is a property of **the nalog**.
 *
 * `support_sessions.odsustvo_otkriveno_at` is a per-nalog stamp, the backend
 * refuses an unmask with no live nalog, and the disclosure expires when the
 * nalog does. On the register it would look like a view setting — something the
 * person reading a payroll screen turns on to see a column — when what it
 * actually is, is the rukovalac widening what an obrađivač may process under
 * čl. 46. So the decision sits beside the four facts that are the nalog.
 *
 * **No re-mask control, because there is no re-mask verb.** The support engineer
 * who has read `kategorija_odsustva` does not unread it; a button that claimed
 * to put it back would assert something neither this app nor the world can do.
 * `PrivacyService` therefore carries exactly one absence-reason method, and
 * `local-adapter.test.ts` pins that count.
 */
export function SupportApprovalPanel({
  services,
  currentUser,
}: {
  services: PosServices;
  /**
   * The signed-in operator, used for one thing: the admin gate over the unmask.
   *
   * Optional, and **absent means closed** — the house rule this repository
   * applies to every privacy gate. The whole Privatnost surface is already
   * admin-only in `navigation.ts` and `support_reveal_absence_reason` is
   * `require_admin`-gated inside the domain function, so this is the third of
   * three layers rather than the only one; it exists so a cashier is never
   * offered a control whose only possible answer is a refusal.
   */
  currentUser?: UserAccount;
}) {
  const [session, setSession] = useState<SupportSession | null>(null);
  const [loading, setLoading] = useState(true);
  const [scope, setScope] = useState("");
  const [duration, setDuration] = useState(String(PODRAZUMEVANO_TRAJANJE_MINUTA));
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const canUnmask = currentUser?.role === "admin";

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

  async function reveal() {
    setError(null);
    setSubmitting(true);
    try {
      setSession(await services.privacy.revealAbsenceReason());
    } catch (revealError) {
      // The refusal is shown as it came back. A surface that swallowed it would
      // leave the vlasnik believing a disclosure happened — and the nalog may
      // have expired between the render and the click, which is precisely the
      // case `support_bez_naloga_za_otkrivanje` exists to name.
      setError(errorMessage(revealError, "Razlog odsustva nije otkriven."));
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

              <Separator />

              <div className="flex flex-col gap-3">
                <h4 className="text-sm font-medium">
                  Razlog odsustva u evidenciji radnog vremena
                </h4>
                <p className="text-sm text-muted-foreground">
                  Dok ovaj nalog važi, kategorija odsustva se ne prikazuje u
                  evidenciji radnog vremena — podatak se za to vreme ne čita iz
                  baze. Skriva se sama kolona: broj časova odsustva po zakonskim
                  vrstama ostaje prikazan, pa se iz njega i dalje može zaključiti o
                  kojoj je vrsti odsustva reč.
                </p>

                {session.odsustvoOtkrivenoAt ? (
                  <Alert>
                    <EyeIcon aria-hidden="true" />
                    <AlertTitle>Razlog odsustva je otkriven</AlertTitle>
                    <AlertDescription>
                      Otkriveno {formatInstant(session.odsustvoOtkrivenoAt)}, za
                      ovaj nalog. Važi do isteka ovog naloga i upisano je u
                      evidenciju pristupa.
                    </AlertDescription>
                  </Alert>
                ) : canUnmask ? (
                  <>
                    <p className="text-sm text-muted-foreground">
                      Otkrivanje važi samo za ovaj nalog i ne može da se povuče:
                      tehnička podrška koja je kategoriju videla više ne može da je
                      ne vidi, pa u programu nema radnje koja je vraća pod masku.
                      Otkrivanje se upisuje u evidenciju pristupa (ZZPL čl. 48), uz
                      oznaku ovog naloga.
                    </p>
                    <Button
                      type="button"
                      variant="outline"
                      className="w-fit"
                      disabled={submitting}
                      onClick={() => void reveal()}
                    >
                      {submitting ? (
                        <Spinner data-icon="inline-start" aria-hidden="true" />
                      ) : (
                        <EyeIcon data-icon="inline-start" />
                      )}
                      Otkrij razlog odsustva za ovaj nalog
                    </Button>
                  </>
                ) : null}
              </div>
            </div>
          ) : (
            <Alert>
              <AlertTitle>Pristup nije odobren</AlertTitle>
              <AlertDescription>
                Nema izdatog naloga za pristup tehničke podrške. Dok nalog ne
                postoji, tehnička podrška ne sme da obrađuje podatke iz ove kase.
                Kategorija odsustva se skriva samo dok nalog važi, pa sada nema
                šta da se otkrije.
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

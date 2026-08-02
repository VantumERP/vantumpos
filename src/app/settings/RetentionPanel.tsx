import { AlertCircleIcon, CalendarClockIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

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
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { RetentionService } from "@/services/ports";
import type { RetentionPolicy } from "@/services/types";

/**
 * Rokovi čuvanja — the one ZZPL/ZEOR surface that genuinely **is** a setting.
 *
 * The evidencije themselves are not: the čl. 46 nalog, the čl. 48 evidencija
 * pristupa and the čl. 52 evidencija povreda are records the rukovalac keeps,
 * and they live under „Privatnost“ so that filing them here could not suggest
 * they can be turned off. A rok is the other thing — ZZPL čl. 5 st. 1 tač. 5
 * makes the shortest defensible period the right default and leaves the shop to
 * lengthen it, and req. 6/22 ask for exactly that to be exposed.
 *
 * **Three properties this panel must not soften.**
 *
 * 1. **Only forward.** The backend refuses an earlier date rather than clamping
 *    it, and the refusal is shown verbatim — „samo unapred“ is printed on the
 *    čl. 23 notice the employee holds, so a screen that silently ignored a
 *    shortening would leave the two disagreeing.
 * 2. **No control at all for a trajno class.** `adjustable` comes from the
 *    backend's own list of classes a command can move, so the row for the ZEOR
 *    čl. 5 evidencija carries a sentence instead of a field.
 * 3. **A rok is a not-before bound, never a deletion date.** „Ništa se ne briše
 *    pre …“ is the same wording the čl. 47 register prints for the Poverenik.
 */
const SVRHA =
  "Rok čuvanja je najraniji dan od kojeg zapis sme da se ukloni, a ne dan kada se uklanja. " +
  "Rok se pomera samo unapred — program odbija skraćivanje, jer je „samo unapred“ ono što " +
  "piše i u obaveštenju zaposlenima i u evidenciji radnji obrade koja se daje Povereniku.";

const TRAJNO =
  "Evidencije koje se po zakonu čuvaju trajno nemaju rok koji bi se pomerao: trajno je " +
  "odsustvo roka, pa bi svaki datum bio kraći od njega.";

export function RetentionPanel({ retention }: { retention: RetentionService }) {
  const [policies, setPolicies] = useState<RetentionPolicy[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let ignore = false;

    retention
      .listPolicies()
      .then((loaded) => {
        if (!ignore) {
          setPolicies(loaded);
          setError(null);
        }
      })
      .catch((readError) => {
        if (!ignore) {
          setError(errorMessage(readError, "Rokovi čuvanja nisu učitani."));
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
  }, [retention]);

  async function move(policy: RetentionPolicy) {
    const retainUntil = (drafts[policy.recordClass] ?? "").trim();
    setError(null);
    setBusy(policy.recordClass);
    try {
      const moved = await retention.extendPolicy(policy.recordClass, retainUntil);
      setPolicies((current) =>
        current.map((row) =>
          row.recordClass === moved.recordClass ? moved : row,
        ),
      );
      setDrafts((current) => ({ ...current, [moved.recordClass]: "" }));
      toast.success("Rok čuvanja je pomeren.");
    } catch (moveError) {
      setError(errorMessage(moveError, "Rok čuvanja nije pomeren."));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle role="heading" aria-level={2}>
          Rokovi čuvanja
        </CardTitle>
        <CardDescription>{SVRHA}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <Alert>
          <CalendarClockIcon aria-hidden="true" />
          <AlertTitle>Trajno se ne podešava</AlertTitle>
          <AlertDescription>{TRAJNO}</AlertDescription>
        </Alert>

        {error ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {loading ? (
          <Badge variant="outline" className="w-fit">
            <Spinner data-icon="inline-start" aria-hidden="true" />
            Učitavanje rokova
          </Badge>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Vrsta zapisa</TableHead>
                <TableHead>Ništa se ne briše pre</TableHead>
                <TableHead>Novi rok</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {policies.map((policy) => (
                <TableRow key={policy.recordClass}>
                  <TableCell className="align-top font-medium">
                    {policy.naziv}
                    <p className="mt-1 max-w-prose text-xs font-normal text-muted-foreground">
                      {policy.napomena}
                    </p>
                  </TableCell>
                  <TableCell className="align-top whitespace-nowrap">
                    {policy.retainUntil ?? "Trajno — bez datuma isteka"}
                    {policy.legalHold ? (
                      <Badge variant="outline" className="ml-2">
                        Pravni zastoj
                      </Badge>
                    ) : null}
                  </TableCell>
                  <TableCell className="align-top">
                    {policy.adjustable ? (
                      <div className="flex flex-wrap items-center gap-2">
                        <Input
                          type="date"
                          className="w-40"
                          aria-label={`Novi rok — ${policy.naziv}`}
                          value={drafts[policy.recordClass] ?? ""}
                          onChange={(event) =>
                            setDrafts((current) => ({
                              ...current,
                              [policy.recordClass]: event.target.value,
                            }))
                          }
                        />
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          disabled={busy !== null}
                          onClick={() => void move(policy)}
                        >
                          {busy === policy.recordClass ? (
                            <Spinner data-icon="inline-start" aria-hidden="true" />
                          ) : null}
                          Pomeri rok
                        </Button>
                      </div>
                    ) : (
                      <span className="text-xs text-muted-foreground">
                        Rok se ne podešava.
                      </span>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  );
}

/**
 * A backend refusal, verbatim. „Rok čuvanja se pomera samo unapred: „2029-01-01“
 * se ne skraćuje na „2028-12-31“.“ is the whole answer the operator needs, and a
 * generic sentence would hide the one fact this panel exists to surface.
 */
function errorMessage(error: unknown, fallback: string): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  return fallback;
}

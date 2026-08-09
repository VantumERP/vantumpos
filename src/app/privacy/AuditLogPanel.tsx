import { AlertCircleIcon, FileDownIcon, SearchIcon } from "lucide-react";
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
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { PosServices } from "@/services/ports";
import type { AuditEvent, AuditSearchResult, UserAccount } from "@/services/types";

/**
 * The ZZPL čl. 48 evidencija pristupa podacima o ličnosti — read, filtered and
 * exported for the Poverenik. Nothing here writes to it.
 *
 * **The one sentence this panel must get right.** Čl. 50 appears nowhere in
 * čl. 95, so no prekršaj is prescribed for breaching it — but *„no fine“* is
 * verified and *„no consequence“* is not: the Poverenik's corrective powers
 * under čl. 79 are the live route. So the copy says exactly „za povredu člana 50
 * nije propisan prekršaj“, names the opomena and the obavezujući nalog that ARE
 * consequences, and never tells the operator that the law requires this log.
 * Overstating it is a credibility risk the moment their lawyer checks, and would
 * have the app asserting a duty the statute does not create.
 *
 * **Req. 7 — no write verb, anywhere.** The panel renders rows and offers no
 * control that could edit or remove one: an owner-editable audit log proves
 * nothing, and proving something is the entire reason it exists. Reading is
 * deliberately not logged either; the export is, because that is the copy that
 * leaves the till.
 *
 * **§5 item 3 — no aggregates.** In a two-or-three employee shop the log
 * identifies each cashier by construction, so rolling it up per person would be
 * a new purpose under čl. 5 st. 1 t. 2 with its own basis and its own čl. 23
 * notice. The actor filter req. 8 asks for narrows the izvod; it never totals.
 */
const CL_50_POSLEDICE =
  "Za povredu člana 50 Zakona o zaštiti podataka o ličnosti nije propisan prekršaj. " +
  "Posledice ipak postoje: Poverenik u nadzoru može da izrekne opomenu i obavezujući " +
  "nalog sa rokom za postupanje.";

/** ZZPL čl. 48 st. 3, verbatim — the only purposes this evidencija may serve. */
const SVRHE_CL_48_ST_3 =
  "Evidencija se koristi samo radi ocene zakonitosti obrade, internog nadzora, " +
  "obezbeđivanja integriteta i bezbednosti podataka i pokretanja i vođenja krivičnog " +
  "postupka — ZZPL čl. 48 st. 3.";

/** Why the shop keeps it at all, stated without inventing a duty. */
const OSNOV_VODJENJA =
  "Rukovalac ovu evidenciju vodi kao sopstvenu meru odgovornosti za postupanje " +
  "(ZZPL čl. 5 st. 2) i radi mogućnosti predočavanja primene načela obrade " +
  "(ZZPL čl. 41 st. 1).";

/**
 * Who performed the radnja, for the person reading the row.
 *
 * An absent actor means two different things and this column must not merge
 * them. On a line linked to a čl. 46 nalog it is the support side entering: a
 * person outside the shop, holding no account on this till, which is precisely
 * why the backend records no id rather than borrowing the vlasnik's. Printing
 * „automatska obrada“ there would deny the one fact the line exists to state.
 *
 * Kept in step with `crate::commands::audit::actor_label`, which draws the same
 * distinction for the čl. 48 st. 4 izvod — the screen and the document must not
 * disagree about the same row. The name-only rendering is this panel's own: §5
 * item 3 keeps the id out of a column a reader might start tallying.
 */
function actorLabel(event: AuditEvent): string {
  if (event.actorName !== null) {
    return event.actorName;
  }
  if (event.actorUserId !== null) {
    return `ID ${event.actorUserId}`;
  }
  return event.supportSessionId !== null
    ? "Obrađivač tehničke podrške (bez korisničkog naloga)"
    : "Automatska obrada (bez korisnika)";
}

export function AuditLogPanel({ services }: { services: PosServices }) {
  const [result, setResult] = useState<AuditSearchResult | null>(null);
  const [users, setUsers] = useState<UserAccount[]>([]);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [actor, setActor] = useState("");
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let ignore = false;

    services.privacy
      .searchAudit({ from: null, to: null, actorUserId: null })
      .then((loaded) => {
        if (!ignore) {
          setResult(loaded);
          setError(null);
        }
      })
      .catch((readError) => {
        if (!ignore) {
          setError(errorMessage(readError, "Evidencija pristupa nije učitana."));
        }
      })
      .finally(() => {
        if (!ignore) {
          setLoading(false);
        }
      });

    // The actor filter needs names; a failed read leaves the filter as „sva
    // lica“ rather than turning a convenience into an alarm.
    services.users
      .listUsers()
      .then((loaded) => {
        if (!ignore) {
          setUsers(loaded);
        }
      })
      .catch(() => {});

    return () => {
      ignore = true;
    };
  }, [services]);

  function query() {
    return {
      from: from.trim() || null,
      to: to.trim() || null,
      actorUserId: actor ? Number.parseInt(actor, 10) : null,
    };
  }

  async function search(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setLoading(true);
    try {
      setResult(await services.privacy.searchAudit(query()));
    } catch (searchError) {
      setError(errorMessage(searchError, "Evidencija pristupa nije učitana."));
    } finally {
      setLoading(false);
    }
  }

  async function exportIzvod() {
    setError(null);
    setExporting(true);
    try {
      const file = await services.privacy.exportAuditCsv(query());
      await services.print.openForPrint(file.path);
    } catch (exportError) {
      setError(errorMessage(exportError, "Izvod nije napravljen."));
    } finally {
      setExporting(false);
    }
  }

  const events = result?.events ?? [];

  return (
    <section className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Evidencija pristupa podacima o ličnosti</CardTitle>
          <CardDescription>{OSNOV_VODJENJA}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <Alert>
            <AlertTitle>Šta ova evidencija jeste, a šta nije</AlertTitle>
            <AlertDescription className="flex flex-col gap-2">
              <span>{CL_50_POSLEDICE}</span>
              <span>{SVRHE_CL_48_ST_3}</span>
            </AlertDescription>
          </Alert>

          {error ? (
            <Alert variant="destructive">
              <AlertCircleIcon aria-hidden="true" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(event) => void search(event)}
          >
            <FieldGroup className="flex-row flex-wrap items-end gap-3">
              <Field className="w-40">
                <FieldLabel htmlFor="audit-from">Od</FieldLabel>
                <Input
                  id="audit-from"
                  type="date"
                  value={from}
                  onChange={(event) => setFrom(event.target.value)}
                />
              </Field>
              <Field className="w-40">
                <FieldLabel htmlFor="audit-to">Do</FieldLabel>
                <Input
                  id="audit-to"
                  type="date"
                  value={to}
                  onChange={(event) => setTo(event.target.value)}
                />
              </Field>
              <Field className="w-56">
                <FieldLabel htmlFor="audit-actor">Lice</FieldLabel>
                <NativeSelect
                  id="audit-actor"
                  value={actor}
                  className="w-full"
                  onChange={(event) => setActor(event.target.value)}
                >
                  <NativeSelectOption value="">Sva lica</NativeSelectOption>
                  {users.map((user) => (
                    <NativeSelectOption key={user.id} value={String(user.id)}>
                      {user.displayName}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </Field>
              <Field className="w-auto">
                <Button type="submit" variant="outline" disabled={loading}>
                  <SearchIcon data-icon="inline-start" />
                  Prikaži
                </Button>
              </Field>
              <Field className="w-auto">
                <Button
                  type="button"
                  disabled={exporting}
                  onClick={() => void exportIzvod()}
                >
                  {exporting ? (
                    <Spinner data-icon="inline-start" aria-hidden="true" />
                  ) : (
                    <FileDownIcon data-icon="inline-start" />
                  )}
                  Izvod za Poverenika
                </Button>
              </Field>
            </FieldGroup>
          </form>

          {result ? (
            <Badge variant={result.chain.intact ? "secondary" : "destructive"}>
              {result.chain.label}
            </Badge>
          ) : null}

          {loading ? (
            <Badge variant="outline">
              <Spinner data-icon="inline-start" aria-hidden="true" />
              Učitavanje evidencije
            </Badge>
          ) : events.length === 0 ? (
            <Alert>
              <AlertDescription>
                Nema zapisa za izabrani period.
              </AlertDescription>
            </Alert>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Datum i vreme</TableHead>
                  <TableHead>Lice</TableHead>
                  <TableHead>Radnja</TableHead>
                  <TableHead>Objekat</TableHead>
                  <TableHead>Razlog</TableHead>
                  <TableHead>Primalac</TableHead>
                  <TableHead>Otisak</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {events.map((event) => (
                  <TableRow key={event.id}>
                    <TableCell>{formatInstant(event.at)}</TableCell>
                    <TableCell>
                      {actorLabel(event)}
                    </TableCell>
                    <TableCell>{event.actionLabel}</TableCell>
                    <TableCell>
                      {event.objectTypeLabel} #{event.objectId}
                    </TableCell>
                    <TableCell>{event.reasonLabel ?? "—"}</TableCell>
                    <TableCell>{event.recipientLabel ?? "—"}</TableCell>
                    <TableCell className="font-mono text-xs">
                      {event.hash.slice(0, 12)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </section>
  );
}

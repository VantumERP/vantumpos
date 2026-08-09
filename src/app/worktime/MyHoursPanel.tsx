import { AlertCircleIcon, ClockIcon, LockIcon } from "lucide-react";
import { useEffect, useState } from "react";

import {
  AbsenceCell,
  buildYearOptions,
  danaSaUnosom,
  formatDan,
  formatMinutes,
  napomenaZa,
} from "@/app/worktime/WorkTimeModule";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { MESECI, nazivPerioda } from "@/lib/period";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { PosServices } from "@/services/ports";
import type { WorkTimeMonth } from "@/services/types";

/**
 * The čl. 55 st. 6 sentence, used only until the month has loaded. It is the
 * same string `crate::commands::worktime::EVIDENCIJA_ZAGLAVLJE` writes into the
 * CSV header, so the export and the screen state the same thing (§4 req. 22).
 */
const EVIDENCIJA_ZAGLAVLJE =
  "Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac.";

/** `crate::commands::worktime::ADVISORY_TAG`. Invariable — quote, never inflect. */
const ADVISORY_TAG = "izračunato radi provere usklađenosti";

export interface MyHoursPanelProps {
  hours: WorkTimeMonth;
}

/**
 * „Moji sati“ — one employee's own month, read-only.
 *
 * It discharges two separate rights at once. **ZoR čl. 83 st. 1** gives the
 * employee a right of inspection, rectification and erasure *vis-à-vis the
 * employer*, and **ZZPL čl. 26** gives the data subject a right of access; both
 * are satisfied by showing the employee the rows the employer actually holds.
 *
 * Four things this surface must not get wrong:
 *
 * 1. **It is read-only.** čl. 83 st. 1 is a right to *ask* the employer to
 *    rectify, not a right to edit the register — a self-service edit would make
 *    the employer's record unreliable as evidence and would defeat the
 *    append-only chain it depends on. There is no control of any kind here.
 * 2. **No obrazac is claimed.** čl. 55 st. 6 delegates nothing, so the heading
 *    sentence comes from the backend and nothing here is called a *propisani
 *    obrazac* or a *propisana evidencija o zaradama*.
 * 3. **The two computed columns are tagged as computed.** No Serbian provision
 *    requires night hours or holiday hours.
 * 4. **Superseded versions stay visible.** A correction appends; hiding the
 *    predecessor would hide from the employee precisely the change čl. 83 st. 1
 *    entitles them to see.
 *
 * The absence category *is* shown here, unlike in the employer-facing grid: the
 * viewer is the data subject, and ZZPL čl. 26 is the right to be told what is
 * recorded about oneself. Withholding it would defeat the point of the screen.
 * That is the carve-out written into docs/SW14-VERIFIED-RULES.md §4 req. 25 —
 * the čl. 50 / čl. 42 gate governs disclosure to *others*, and it is not
 * widened anywhere else. No penalty figure is composed or rendered here — this
 * screen states rights, not offences.
 */
export function MyHoursPanel({ hours }: MyHoursPanelProps) {
  const liveEntries = hours.entries.filter((entry) => !entry.zamenjen);

  return (
    <section className="flex flex-col gap-4">
      <div>
        <div className="flex items-center gap-2">
          <h2 className="text-lg font-semibold">Moji sati</h2>
          {hours.zatvoren ? (
            <Badge variant="secondary">
              <LockIcon data-icon="inline-start" aria-hidden="true" />
              Period je zaključen
            </Badge>
          ) : null}
        </div>
        <p className="text-sm text-muted-foreground">
          {hours.napomena || EVIDENCIJA_ZAGLAVLJE}
        </p>
        <p className="text-sm text-muted-foreground">
          {`${hours.zaposleni} — ${nazivPerioda(hours.godina, hours.mesec)}.`}
        </p>
      </div>

      <Alert>
        <ClockIcon aria-hidden="true" />
        <AlertTitle>Uvid u sopstvene podatke</AlertTitle>
        <AlertDescription>
          <p>
            Ovaj pregled je samo za čitanje i pokazuje evidenciju koju o Vama
            vodi poslodavac — ZoR čl. 83 st. 1 i ZZPL čl. 26.
          </p>
          <p>
            Ispravku netačnog podatka, kao i brisanje podatka koji nije od
            neposrednog značaja za poslove koje obavljate, tražite od
            poslodavca. Ispravka se upisuje kao nova verzija dana, a raniji unos
            ostaje vidljiv.
          </p>
        </AlertDescription>
      </Alert>

      {hours.entries.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <ClockIcon aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>Nema unosa za izabrani mesec</EmptyTitle>
            <EmptyDescription>
              Evidenciju radnog vremena vodi poslodavac, po danu.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Datum</TableHead>
                <TableHead>Verzija</TableHead>
                <TableHead>Odsustvo</TableHead>
                <TableHead className="text-right">Mogući</TableHead>
                <TableHead className="text-right">Ukupno ostvareni</TableHead>
                <TableHead className="text-right">Efektivno izvršeni</TableHead>
                <TableHead className="text-right">Čekanja i zastoji</TableHead>
                <TableHead className="text-right">Ukupno neizvršeni</TableHead>
                <TableHead className="text-right">Prekovremeni</TableHead>
                <TableHead className="text-right">Noćni*</TableHead>
                <TableHead className="text-right">Rad na praznik*</TableHead>
                <TableHead>Uneo</TableHead>
                <TableHead>Napomena</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {hours.entries.map((entry) => (
                <TableRow
                  key={entry.id}
                  className={
                    entry.zamenjen
                      ? "text-muted-foreground line-through"
                      : undefined
                  }
                >
                  <TableCell>{formatDan(entry.dan)}</TableCell>
                  <TableCell>{entry.verzija}</TableCell>
                  <TableCell>
                    {/*
                      `true` on purpose: the viewer is the data subject. The gate
                      exists so a colleague's category is never shown, and here
                      there is no colleague — `worktime_my_hours` resolves the
                      employee from the session and returns own rows only. This
                      is the carve-out recorded in docs/SW14-VERIFIED-RULES.md
                      §4 req. 25, not a hole in it.

                      `razlogSkriven` is passed from the read rather than written
                      as `false`, even though `my_hours` masks nothing today and
                      deliberately so (SW-14 req. 28 is about the vendor, not
                      about the person whose month it is). A literal here would
                      be this panel asserting a backend property instead of
                      reporting one, and it would go silently stale the day that
                      decision is revisited.
                    */}
                    <AbsenceCell
                      entry={entry}
                      canSeeAbsenceReason
                      razlogSkriven={hours.razlogOdsustvaSkriven}
                    />
                  </TableCell>
                  <MinuteCell value={entry.minuti.moguciMinuta} />
                  <MinuteCell value={entry.minuti.ukupnoOstvareniMinuta} />
                  <MinuteCell value={entry.minuti.efektivnoIzvrseniMinuta} />
                  <MinuteCell value={entry.minuti.casoviCekanjaIZastojaMinuta} />
                  <MinuteCell value={entry.minuti.ukupnoNeizvrseniMinuta} />
                  <MinuteCell value={entry.minuti.prekovremeniMinuta} />
                  <MinuteCell value={entry.minuti.nocniMinuta} />
                  <MinuteCell value={entry.minuti.radNaPraznikMinuta} />
                  <TableCell>{entry.unioIme ?? "—"}</TableCell>
                  <TableCell>{napomenaZa(entry)}</TableCell>
                </TableRow>
              ))}
              <TableRow className="font-medium">
                <TableCell>Ukupno</TableCell>
                <TableCell />
                <TableCell>{danaSaUnosom(liveEntries.length)}</TableCell>
                <MinuteCell value={hours.ukupno.moguciMinuta} />
                <MinuteCell value={hours.ukupno.ukupnoOstvareniMinuta} />
                <MinuteCell value={hours.ukupno.efektivnoIzvrseniMinuta} />
                <MinuteCell value={hours.ukupno.casoviCekanjaIZastojaMinuta} />
                <MinuteCell value={hours.ukupno.ukupnoNeizvrseniMinuta} />
                <MinuteCell value={hours.ukupno.prekovremeniMinuta} />
                <MinuteCell value={hours.ukupno.nocniMinuta} />
                <MinuteCell value={hours.ukupno.radNaPraznikMinuta} />
                <TableCell />
                <TableCell />
              </TableRow>
            </TableBody>
          </Table>
          <p className="mt-2 text-xs text-muted-foreground">
            {`* ${hours.advisoryNapomena || ADVISORY_TAG}.`}
          </p>
        </div>
      )}
    </section>
  );
}

export interface MyHoursScreenProps {
  services: PosServices;
}

/**
 * The period picker around {@link MyHoursPanel}.
 *
 * It calls `myHours`, never `listMonth`: `listMonth` takes a `userId` and is
 * admin-gated, while `worktime_my_hours` resolves the employee from the
 * server-side session. There is deliberately no employee selector here — the
 * only thing a cashier may read is their own month, and a picker would suggest
 * otherwise even though the backend would refuse.
 */
export function MyHoursScreen({ services }: MyHoursScreenProps) {
  const worktime = services.worktime;
  const [godina, setGodina] = useState(() => new Date().getFullYear());
  const [mesec, setMesec] = useState(() => new Date().getMonth() + 1);
  const [hours, setHours] = useState<WorkTimeMonth | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    let cancelled = false;
    setStatus("loading");
    setError(undefined);

    worktime.myHours(godina, mesec).then(
      (result) => {
        if (!cancelled) {
          setHours(result);
          setStatus("ready");
        }
      },
      (reason) => {
        if (!cancelled) {
          setError(errorMessage(reason, "Evidencija nije učitana."));
          setStatus("error");
        }
      },
    );

    return () => {
      cancelled = true;
    };
  }, [worktime, godina, mesec]);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end gap-2">
        <Field className="w-auto">
          <FieldLabel htmlFor="moji-sati-godina">Godina</FieldLabel>
          <NativeSelect
            id="moji-sati-godina"
            value={String(godina)}
            onChange={(event) => setGodina(Number(event.target.value))}
          >
            {buildYearOptions(new Date().getFullYear()).map((year) => (
              <NativeSelectOption key={year} value={String(year)}>
                {year}.
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
        <Field className="w-auto">
          <FieldLabel htmlFor="moji-sati-mesec">Mesec</FieldLabel>
          <NativeSelect
            id="moji-sati-mesec"
            value={String(mesec)}
            onChange={(event) => setMesec(Number(event.target.value))}
          >
            {MESECI.map((label, index) => (
              <NativeSelectOption key={label} value={String(index + 1)}>
                {label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </Field>
      </div>

      {status === "loading" ? (
        <Badge variant="outline">
          <Spinner data-icon="inline-start" aria-hidden="true" />
          Učitavanje evidencije
        </Badge>
      ) : null}

      {status === "error" ? (
        <Alert variant="destructive">
          <AlertCircleIcon aria-hidden="true" />
          <AlertTitle>Evidencija nije učitana</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {status === "ready" && hours ? <MyHoursPanel hours={hours} /> : null}
    </div>
  );
}

function MinuteCell({ value }: { value: number }) {
  return (
    <TableCell className="text-right tabular-nums">
      {value === 0 ? "—" : formatMinutes(value)}
    </TableCell>
  );
}

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

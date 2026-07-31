import { DownloadIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatRsd } from "@/lib/money";
import type {
  CashDepositReport as CashDepositReportDto,
  DepositBucket,
} from "@/services/types";

type BucketStatus = "deposited" | "overdue" | "inTime" | "unknownDeadline";

const STATUS_LABELS: Record<BucketStatus, string> = {
  deposited: "Položeno",
  overdue: "Kasni",
  inTime: "U roku",
  // A missing deadline means the count could not be made at all — not that the
  // money is inside a rok. „U roku" there would be a positive affirmation next
  // to an empty Rok column, which is exactly the false „clean" state the report
  // exists to prevent. The unknown stays visible as an unknown.
  unknownDeadline: "Rok nije izračunat",
};

/**
 * „Izveštaj o nedeponovanom gotovom novcu" — the seven-working-day aging report
 * (Zakon 68/2015, čl. 3 st. 1).
 *
 * Presentational and advisory. It renders the deadline as a **date**, never as
 * a countdown, and it has no action that could refuse a sale, a shift close or
 * a fiscalization: the duty is fiscal hygiene supervised by Poreska uprava, not
 * a condition of a valid sale. Every honesty label the backend attached to the
 * numbers (`report.footer`) is printed with them, because each sentence
 * corrects something the bare table would otherwise imply — that the
 * per-trading-date roll-up is statutory, or that the float exclusion is
 * settled law rather than bylaw-level relief.
 *
 * There is no blagajnički maksimum here, and there must never be one: no propis
 * sets a cash-on-hand ceiling, so any such alert would be an internal policy
 * with no default, never a legal duty.
 */
export function CashDepositReport({
  report,
  onExportCsv,
  exporting = false,
}: {
  report: CashDepositReportDto;
  onExportCsv?: () => void;
  exporting?: boolean;
}) {
  const hasOverdue = report.overdueMinor > 0;

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          <h2>Nedeponovan gotov novac</h2>
        </CardTitle>
        <CardDescription>
          Presek na dan {formatIsoDate(report.asOf)} — gotovina primljena po
          bilo kom osnovu i deo koji je stigao na tekući račun radnje.
        </CardDescription>
        {onExportCsv ? (
          <CardAction>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={exporting}
              onClick={onExportCsv}
            >
              <DownloadIcon data-icon="inline-start" aria-hidden="true" />
              Izvezi CSV
            </Button>
          </CardAction>
        ) : null}
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        <div className="grid gap-3 sm:grid-cols-3">
          <SummaryTile
            label="Neizmireno ukupno"
            value={formatRsd(report.outstandingMinor)}
          />
          <SummaryTile
            label="Od toga van roka"
            value={formatRsd(report.overdueMinor)}
          />
          <SummaryTile
            label="Izuzeto (podizanja sa računa)"
            value={formatRsd(report.excludedFloatMinor)}
          />
        </div>

        {hasOverdue ? (
          <Alert>
            <AlertTitle>Rok od sedam radnih dana je istekao</AlertTitle>
            <AlertDescription>
              <div className="flex flex-col gap-1">
                <p>
                  Van roka je {formatRsd(report.overdueMinor)}. Izveštaj je
                  informativan i ništa ne blokira — ni prodaju, ni zatvaranje
                  smene, ni fiskalizaciju.
                </p>
                <p>{report.notice.summary}</p>
              </div>
            </AlertDescription>
          </Alert>
        ) : null}

        {report.buckets.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyTitle>Nema nepoloženog gotovog novca</EmptyTitle>
              <EmptyDescription>
                Na dan preseka nijedan dan prometa nema neizmiren iznos. Izveštaj
                prikazuje samo ono što je evidentirano u ovoj aplikaciji.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <div className="overflow-x-auto rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Datum prometa</TableHead>
                  <TableHead className="text-right">Primljeno</TableHead>
                  <TableHead className="text-right">Položeno na račun</TableHead>
                  <TableHead className="text-right">Ostatak</TableHead>
                  <TableHead>Rok</TableHead>
                  <TableHead>Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {report.buckets.map((bucket) => {
                  const status = bucketStatus(bucket);
                  return (
                    <TableRow key={bucket.tradingDate}>
                      <TableCell>{formatIsoDate(bucket.tradingDate)}</TableCell>
                      <TableCell className="text-right">
                        {formatRsd(bucket.subjectMinor)}
                      </TableCell>
                      <TableCell className="text-right">
                        {formatRsd(bucket.depositedMinor)}
                      </TableCell>
                      <TableCell className="text-right">
                        {formatRsd(bucket.outstandingMinor)}
                      </TableCell>
                      <TableCell>
                        {bucket.dueOn ? formatIsoDate(bucket.dueOn) : "-"}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant={
                            status === "overdue" ? "destructive" : "outline"
                          }
                        >
                          {STATUS_LABELS[status]}
                        </Badge>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}

        <div className="flex flex-col gap-2 text-xs text-muted-foreground">
          <p>
            {report.saturdayIsWorking
              ? "Subota se računa kao radni dan."
              : "Subota se ne računa kao radni dan."}{" "}
            „Radni dan“ nije definisan ni u Zakonu 68/2015 ni u Pravilniku
            77/2011 — pretpostavka se menja u Podešavanjima.
          </p>
          {report.beyondSeededCalendar ? (
            <p>
              Tabela praznika je pripremljena zaključno sa{" "}
              {report.calendarHorizonYear}. godinom, a pojedini rok pada posle
              nje — dopunite listu neradnih dana u Podešavanjima.
            </p>
          ) : null}
          <p>{report.footer}</p>
          {report.notice.penalty ? (
            <p>{report.notice.penalty}</p>
          ) : (
            <p>
              Unesite pravnu formu u Podešavanja → Profil da bi kazna bila
              prikazana.
            </p>
          )}
          <p>{report.notice.citation}</p>
        </div>
      </CardContent>
    </Card>
  );
}

function SummaryTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1 rounded-md border p-3">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-lg font-semibold tabular-nums">{value}</span>
    </div>
  );
}

function bucketStatus(bucket: DepositBucket): BucketStatus {
  if (bucket.outstandingMinor === 0) {
    return "deposited";
  }
  if (bucket.dueOn === null) {
    return "unknownDeadline";
  }
  return bucket.isOverdue ? "overdue" : "inTime";
}

/**
 * `YYYY-MM-DD` -> `DD.MM.YYYY.` without touching `Date`: the deadline is a
 * calendar date the backend already decided, and parsing it into a timestamp
 * would let the machine's timezone shift it by a day.
 */
function formatIsoDate(iso: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!match) {
    return iso;
  }
  const [, year, month, day] = match;
  return `${day}.${month}.${year}.`;
}

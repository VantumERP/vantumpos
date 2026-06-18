import {
  AlertCircleIcon,
  DownloadIcon,
  PackageSearchIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  XAxis,
  YAxis,
} from "recharts";
import { toast } from "sonner";

import { Alert, AlertDescription } from "@/components/ui/alert";
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
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import { formatRsd } from "@/lib/money";
import type { ReportsService } from "@/services/ports";
import type {
  CashierTurnoverReport,
  CategorySalesReport,
  DailyTurnoverReport,
  ExportReportType,
  LowStockReport,
  PaymentMethodReport,
  ProductSalesQuery,
  ProductSalesReport,
  ReportDateQuery,
  ShiftTurnoverReport,
} from "@/services/types";

const turnoverChartConfig = {
  totalMinor: {
    label: "Ukupno",
    color: "var(--chart-2)",
  },
  cashMinor: {
    label: "Gotovina",
    color: "var(--chart-3)",
  },
  cardMinor: {
    label: "Kartica",
    color: "var(--chart-4)",
  },
} satisfies ChartConfig;

interface ReportsData {
  dailyTurnover: DailyTurnoverReport;
  shiftTurnover: ShiftTurnoverReport;
  cashierTurnover: CashierTurnoverReport;
  paymentMethods: PaymentMethodReport;
  productSales: ProductSalesReport;
  categorySales: CategorySalesReport;
  lowStock: LowStockReport;
}

interface ReportsScreenProps {
  reports: ReportsService;
  initialQuery?: ReportDateQuery;
}

export function ReportsScreen({ reports, initialQuery }: ReportsScreenProps) {
  const defaultQuery = useMemo(() => initialQuery ?? todayQuery(), [initialQuery]);
  const [filters, setFilters] = useState<ReportDateQuery>(defaultQuery);
  const [appliedQuery, setAppliedQuery] = useState<ReportDateQuery>(defaultQuery);
  const [data, setData] = useState<ReportsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const loadReports = useCallback(
    async (query: ReportDateQuery) => {
      setLoading(true);
      setErrorMessage(null);

      try {
        const productQuery = toProductSalesQuery(query);
        const [
          dailyTurnover,
          shiftTurnover,
          cashierTurnover,
          paymentMethods,
          productSales,
          categorySales,
          lowStock,
        ] = await Promise.all([
          reports.getDailyTurnover(query),
          reports.getShiftTurnover(query),
          reports.getCashierTurnover(query),
          reports.getPaymentMethodTurnover(query),
          reports.getProductSales(productQuery),
          reports.getCategorySales(query),
          reports.getLowStock(),
        ]);

        setData({
          dailyTurnover,
          shiftTurnover,
          cashierTurnover,
          paymentMethods,
          productSales,
          categorySales,
          lowStock,
        });
        setAppliedQuery(query);
      } catch (error) {
        setErrorMessage(errorToMessage(error));
      } finally {
        setLoading(false);
      }
    },
    [reports],
  );

  useEffect(() => {
    void loadReports(defaultQuery);
  }, [defaultQuery, loadReports]);

  function applyFilters(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void loadReports(filters);
  }

  async function exportCsv(reportType: ExportReportType) {
    try {
      const exported = await reports.exportReportCsv({
        reportType,
        query: toProductSalesQuery(appliedQuery),
      });
      toast.success("CSV izvezen", {
        description: exported.path,
      });
    } catch (error) {
      toast.error("CSV export nije uspeo", {
        description: errorToMessage(error),
      });
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <form
        className="flex flex-col gap-3 rounded-md border bg-background p-3 md:flex-row md:items-end"
        onSubmit={applyFilters}
      >
        <FieldGroup className="grid flex-1 gap-3 md:grid-cols-2">
          <Field>
            <FieldLabel htmlFor="reports-date-from">Od datuma</FieldLabel>
            <Input
              id="reports-date-from"
              type="date"
              value={filters.from}
              onChange={(event) =>
                setFilters((current) => ({
                  ...current,
                  from: event.target.value,
                }))
              }
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="reports-date-to">Do datuma</FieldLabel>
            <Input
              id="reports-date-to"
              type="date"
              value={filters.to}
              onChange={(event) =>
                setFilters((current) => ({
                  ...current,
                  to: event.target.value,
                }))
              }
            />
          </Field>
        </FieldGroup>
        <Button type="submit">Primeni filtere</Button>
      </form>

      {errorMessage ? (
        <Alert variant="destructive">
          <AlertCircleIcon data-icon="inline-start" />
          <AlertDescription>{errorMessage}</AlertDescription>
        </Alert>
      ) : null}

      {loading || !data ? (
        <ReportsLoading />
      ) : (
        <Tabs defaultValue="turnover">
          <TabsList>
            <TabsTrigger value="turnover">Promet</TabsTrigger>
            <TabsTrigger value="items">Artikli</TabsTrigger>
            <TabsTrigger value="stock">Lager</TabsTrigger>
            <TabsTrigger value="export">Izvoz</TabsTrigger>
          </TabsList>

          <TabsContent value="turnover" className="flex flex-col gap-4">
            <DailyTurnoverSection report={data.dailyTurnover} />
            <TurnoverBreakdownSection
              shiftTurnover={data.shiftTurnover}
              cashierTurnover={data.cashierTurnover}
              paymentMethods={data.paymentMethods}
            />
          </TabsContent>

          <TabsContent value="items" className="grid gap-4 xl:grid-cols-2">
            <ProductSalesTable report={data.productSales} />
            <CategorySalesTable report={data.categorySales} />
          </TabsContent>

          <TabsContent value="stock" className="flex flex-col gap-4">
            <LowStockTable report={data.lowStock} />
          </TabsContent>

          <TabsContent value="export" className="grid gap-3 md:grid-cols-3">
            <ExportAction
              title="Dnevni promet"
              description="Ukupno, gotovina, kartica i storniranja."
              buttonLabel="Izvezi dnevni promet"
              onExport={() => void exportCsv("dailyTurnover")}
            />
            <ExportAction
              title="Prodaja artikala"
              description="Kolicina, promet, popust i procena marze."
              buttonLabel="Izvezi artikle"
              onExport={() => void exportCsv("productSales")}
            />
            <ExportAction
              title="Nizak lager"
              description="Artikli ispod minimalne zalihe."
              buttonLabel="Izvezi nizak lager"
              onExport={() => void exportCsv("lowStock")}
            />
          </TabsContent>
        </Tabs>
      )}
    </div>
  );
}

function DailyTurnoverSection({ report }: { report: DailyTurnoverReport }) {
  return (
    <section className="flex flex-col gap-3">
      <div className="grid gap-3 md:grid-cols-5">
        <MetricCard title="Ukupan promet" value={formatRsd(report.summary.totalMinor)} />
        <MetricCard title="Gotovina" value={formatRsd(report.summary.cashMinor)} />
        <MetricCard title="Kartica" value={formatRsd(report.summary.cardMinor)} />
        <MetricCard title="Racuni" value={report.summary.receiptCount.toString()} />
        <MetricCard
          title="Prosecan racun"
          value={formatRsd(report.summary.averageReceiptMinor)}
        />
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
        <Card>
          <CardHeader>
            <CardTitle>
              <h2>Dnevni promet</h2>
            </CardTitle>
            <CardDescription>Net promet po danu za izabrani period.</CardDescription>
          </CardHeader>
          <CardContent>
            {report.rows.length === 0 ? (
              <ReportEmpty title="Nema prometa" />
            ) : (
              <ChartContainer
                config={turnoverChartConfig}
                className="min-h-56 w-full"
              >
                <BarChart accessibilityLayer data={report.rows}>
                  <CartesianGrid vertical={false} />
                  <XAxis dataKey="day" tickLine={false} axisLine={false} />
                  <YAxis hide />
                  <ChartTooltip
                    content={
                      <ChartTooltipContent
                        formatter={(value, name) => (
                          <div className="flex min-w-32 items-center justify-between gap-3">
                            <span>
                              {turnoverChartConfig[
                                name as keyof typeof turnoverChartConfig
                              ]?.label ?? name}
                            </span>
                            <span className="font-mono">
                              {formatRsd(Number(value))}
                            </span>
                          </div>
                        )}
                      />
                    }
                  />
                  <Bar
                    dataKey="totalMinor"
                    fill="var(--color-totalMinor)"
                    radius={4}
                  />
                </BarChart>
              </ChartContainer>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Dnevna tabela</CardTitle>
            <CardDescription>Gotovina, kartica i storniranja.</CardDescription>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Dan</TableHead>
                  <TableHead className="text-right">Broj racuna</TableHead>
                  <TableHead className="text-right">Gotovina</TableHead>
                  <TableHead className="text-right">Kartica</TableHead>
                  <TableHead className="text-right">Ukupno</TableHead>
                  <TableHead className="text-right">Povrati/storno</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {report.rows.map((row) => (
                  <TableRow key={row.day}>
                    <TableCell className="font-medium">{row.day}</TableCell>
                    <TableCell className="text-right">{row.receiptCount}</TableCell>
                    <TableCell className="text-right">{formatRsd(row.cashMinor)}</TableCell>
                    <TableCell className="text-right">{formatRsd(row.cardMinor)}</TableCell>
                    <TableCell className="text-right">{formatRsd(row.totalMinor)}</TableCell>
                    <TableCell className="text-right">
                      {formatRsd(row.refundsOrVoidsMinor)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </div>
    </section>
  );
}

function TurnoverBreakdownSection({
  shiftTurnover,
  cashierTurnover,
  paymentMethods,
}: {
  shiftTurnover: ShiftTurnoverReport;
  cashierTurnover: CashierTurnoverReport;
  paymentMethods: PaymentMethodReport;
}) {
  return (
    <div className="grid gap-4 xl:grid-cols-3">
      <Card>
        <CardHeader>
          <CardTitle>Smene</CardTitle>
          <CardDescription>Promet po otvorenim i zatvorenim smenama.</CardDescription>
        </CardHeader>
        <CardContent>
          <CompactTable
            headers={["Smena", "Kasir", "Ukupno"]}
            rows={shiftTurnover.rows.map((row) => [
              `#${row.shiftId}`,
              row.cashierName,
              formatRsd(row.totalMinor),
            ])}
          />
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Kasiri</CardTitle>
          <CardDescription>Net promet po kasiru.</CardDescription>
        </CardHeader>
        <CardContent>
          <CompactTable
            headers={["Kasir", "Racuni", "Ukupno"]}
            rows={cashierTurnover.rows.map((row) => [
              row.cashierName,
              row.receiptCount.toString(),
              formatRsd(row.totalMinor),
            ])}
          />
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>Placanja</CardTitle>
          <CardDescription>Gotovina i kartica kroz racune.</CardDescription>
        </CardHeader>
        <CardContent>
          <CompactTable
            headers={["Nacin", "Racuni", "Ukupno"]}
            rows={paymentMethods.rows.map((row) => [
              paymentMethodLabel(row.paymentMethod),
              row.receiptCount.toString(),
              formatRsd(row.totalMinor),
            ])}
          />
        </CardContent>
      </Card>
    </div>
  );
}

function ProductSalesTable({ report }: { report: ProductSalesReport }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Prodaja artikala</CardTitle>
        <CardDescription>Marza je procena na osnovu nabavne cene.</CardDescription>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Artikal</TableHead>
              <TableHead>SKU</TableHead>
              <TableHead className="text-right">Kolicina</TableHead>
              <TableHead className="text-right">Promet</TableHead>
              <TableHead className="text-right">Popust</TableHead>
              <TableHead className="text-right">Procena marze</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {report.rows.map((row) => (
              <TableRow key={`${row.productId ?? "snapshot"}-${row.productSku}`}>
                <TableCell className="font-medium">{row.productName}</TableCell>
                <TableCell>{row.productSku}</TableCell>
                <TableCell className="text-right">
                  {formatQuantity(row.quantityMilli)}
                </TableCell>
                <TableCell className="text-right">{formatRsd(row.revenueMinor)}</TableCell>
                <TableCell className="text-right">{formatRsd(row.discountMinor)}</TableCell>
                <TableCell className="text-right">
                  {formatRsd(row.estimatedMarginMinor)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

function CategorySalesTable({ report }: { report: CategorySalesReport }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Prodaja kategorija</CardTitle>
        <CardDescription>Grupisano po trenutnoj kategoriji artikla.</CardDescription>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Kategorija</TableHead>
              <TableHead className="text-right">Kolicina</TableHead>
              <TableHead className="text-right">Promet</TableHead>
              <TableHead className="text-right">Popust</TableHead>
              <TableHead className="text-right">Procena marze</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {report.rows.map((row) => (
              <TableRow key={row.categoryId ?? row.categoryName}>
                <TableCell className="font-medium">{row.categoryName}</TableCell>
                <TableCell className="text-right">
                  {formatQuantity(row.quantityMilli)}
                </TableCell>
                <TableCell className="text-right">{formatRsd(row.revenueMinor)}</TableCell>
                <TableCell className="text-right">{formatRsd(row.discountMinor)}</TableCell>
                <TableCell className="text-right">
                  {formatRsd(row.estimatedMarginMinor)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

function LowStockTable({ report }: { report: LowStockReport }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Nizak lager</CardTitle>
        <CardDescription>Artikli koji su ispod minimalne zalihe.</CardDescription>
      </CardHeader>
      <CardContent>
        {report.rows.length === 0 ? (
          <ReportEmpty title="Nema artikala ispod minimuma" />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Artikal</TableHead>
                <TableHead>SKU</TableHead>
                <TableHead className="text-right">Trenutno</TableHead>
                <TableHead className="text-right">Minimum</TableHead>
                <TableHead className="text-right">Razlika</TableHead>
                <TableHead>Poslednja promena</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {report.rows.map((row) => (
                <TableRow key={row.productId}>
                  <TableCell className="font-medium">
                    {row.productName}
                  </TableCell>
                  <TableCell>{row.productSku}</TableCell>
                  <TableCell className="text-right">
                    {formatQuantity(row.currentStockMilli)}
                  </TableCell>
                  <TableCell className="text-right">
                    {formatQuantity(row.minimumStockMilli)}
                  </TableCell>
                  <TableCell className="text-right">
                    <Badge variant="destructive">
                      {formatQuantity(row.differenceMilli)}
                    </Badge>
                  </TableCell>
                  <TableCell>{row.lastMovementAt ?? "-"}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  );
}

function ExportAction({
  title,
  description,
  buttonLabel,
  onExport,
}: {
  title: string;
  description: string;
  buttonLabel: string;
  onExport: () => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent>
        <Button type="button" variant="outline" onClick={onExport}>
          <DownloadIcon data-icon="inline-start" />
          {buttonLabel}
        </Button>
      </CardContent>
    </Card>
  );
}

function MetricCard({ title, value }: { title: string; value: string }) {
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription>{title}</CardDescription>
        <CardTitle>{value}</CardTitle>
      </CardHeader>
    </Card>
  );
}

function CompactTable({
  headers,
  rows,
}: {
  headers: string[];
  rows: string[][];
}) {
  if (rows.length === 0) {
    return <ReportEmpty title="Nema podataka" />;
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          {headers.map((header) => (
            <TableHead key={header}>{header}</TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((row) => (
          <TableRow key={row.join("|")}>
            {row.map((cell, index) => (
              <TableCell key={`${cell}-${index}`}>{cell}</TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function ReportEmpty({ title }: { title: string }) {
  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <PackageSearchIcon aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>Promenite filtere ili proverite dnevni rad.</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

function ReportsLoading() {
  return (
    <div className="grid gap-3 md:grid-cols-3">
      <Skeleton className="h-28 rounded-md" />
      <Skeleton className="h-28 rounded-md" />
      <Skeleton className="h-28 rounded-md" />
    </div>
  );
}

function todayQuery(): ReportDateQuery {
  const today = new Date().toISOString().slice(0, 10);
  return { from: today, to: today };
}

function toProductSalesQuery(query: ReportDateQuery): ProductSalesQuery {
  return {
    ...query,
    categoryId: null,
    productId: null,
  };
}

function formatQuantity(quantityMilli: number): string {
  const quantity = quantityMilli / 1000;
  return `${Number.isInteger(quantity) ? quantity.toString() : quantity.toFixed(3)} kom`;
}

function paymentMethodLabel(paymentMethod: string): string {
  return paymentMethod === "cash" ? "Gotovina" : "Kartica";
}

function errorToMessage(error: unknown): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return "Izvestaj trenutno nije dostupan.";
}

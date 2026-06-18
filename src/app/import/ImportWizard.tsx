import {
  AlertCircleIcon,
  CheckIcon,
  FileTextIcon,
  HistoryIcon,
  UploadIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { ChangeEvent } from "react";

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import {
  Progress,
  ProgressLabel,
  ProgressValue,
} from "@/components/ui/progress";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { PosServices } from "@/services/ports";
import type {
  ImportHeaders,
  ImportJob,
  ImportMapping,
  ImportType,
  ImportValidationResult,
} from "@/services/types";

interface ImportWizardProps {
  services: PosServices;
}

const IMPORT_TYPE_LABELS: Record<ImportType, string> = {
  products: "Artikli",
  categories: "Kategorije",
  initial_stock: "Pocetno stanje",
};

const MAPPING_FIELDS: Record<
  ImportType,
  Array<{ key: string; label: string; required: boolean }>
> = {
  products: [
    { key: "name", label: "Naziv", required: true },
    { key: "sale_price", label: "Prodajna cena", required: true },
    { key: "vat_rate", label: "PDV stopa", required: true },
    { key: "sku", label: "SKU/sifra", required: false },
    { key: "barcode", label: "Barcode", required: false },
    { key: "category", label: "Kategorija", required: false },
    { key: "purchase_price", label: "Nabavna cena", required: false },
    { key: "minimum_stock", label: "Minimalna zaliha", required: false },
    { key: "unit_of_measure", label: "Jedinica mere", required: false },
    { key: "initial_stock", label: "Pocetna zaliha", required: false },
  ],
  categories: [
    { key: "name", label: "Naziv", required: true },
    { key: "active", label: "Aktivna", required: false },
  ],
  initial_stock: [
    { key: "sku", label: "SKU/sifra", required: false },
    { key: "barcode", label: "Barcode", required: false },
    { key: "quantity", label: "Kolicina", required: true },
  ],
};

const FIELD_ALIASES: Record<string, string[]> = {
  name: ["naziv", "name", "artikal", "kategorija"],
  sale_price: ["cena", "prodajna cena", "prodajna_cena", "price"],
  vat_rate: ["pdv", "pdv stopa", "vat", "vat rate"],
  sku: ["sku", "sifra", "sifra artikla", "code", "kod"],
  barcode: ["barcode", "bar kod", "barkod", "ean"],
  category: ["kategorija", "category"],
  purchase_price: ["nabavna cena", "nabavna_cena", "purchase price"],
  minimum_stock: ["minimalna zaliha", "minimum stock", "min"],
  unit_of_measure: ["jedinica mere", "jm", "unit"],
  initial_stock: ["pocetna zaliha", "zaliha", "stanje"],
  quantity: ["kolicina", "kolicina stanje", "quantity", "zaliha"],
  active: ["aktivan", "active"],
};

export function ImportWizard({ services }: ImportWizardProps) {
  const [importType, setImportType] = useState<ImportType>("products");
  const [csvText, setCsvText] = useState("");
  const [fileName, setFileName] = useState("");
  const [headers, setHeaders] = useState<ImportHeaders | null>(null);
  const [mapping, setMapping] = useState<ImportMapping>({});
  const [validation, setValidation] = useState<ImportValidationResult | null>(
    null,
  );
  const [history, setHistory] = useState<ImportJob[]>([]);
  const [committedJob, setCommittedJob] = useState<ImportJob | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [isBusy, setIsBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fields = MAPPING_FIELDS[importType];
  const canValidate = Boolean(headers && requiredMappingComplete(importType, mapping));
  const canCommit = Boolean(validation && validation.errorCount === 0);
  const progressValue = committedJob ? 100 : validation ? 75 : headers ? 45 : 15;

  const loadHistory = useCallback(async () => {
    const jobs = await services.imports.listImportJobs();
    setHistory(jobs);
  }, [services.imports]);

  useEffect(() => {
    let active = true;

    services.imports
      .listImportJobs()
      .then((jobs) => {
        if (active) {
          setHistory(jobs);
        }
      })
      .catch(() => {
        if (active) {
          setError("Istorija importa nije dostupna.");
        }
      });

    return () => {
      active = false;
    };
  }, [services.imports]);

  const mappedLabels = useMemo(
    () =>
      fields.map((field) => ({
        ...field,
        header: mapping[field.key] ?? "",
      })),
    [fields, mapping],
  );

  async function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];

    if (!file) {
      return;
    }

    setIsBusy(true);
    setError(null);
    setValidation(null);
    setCommittedJob(null);

    try {
      const text = await file.text();
      const request = {
        importType,
        fileName: file.name,
        csvText: text,
      };
      const nextHeaders = await services.imports.readImportHeaders(request);

      setCsvText(text);
      setFileName(file.name);
      setHeaders(nextHeaders);
      setMapping(autoMapHeaders(importType, nextHeaders.headers));
    } catch (caught) {
      setError(messageFromError(caught));
    } finally {
      setIsBusy(false);
    }
  }

  async function validateCsv() {
    if (!headers) {
      return;
    }

    setIsBusy(true);
    setError(null);
    setCommittedJob(null);

    try {
      const result = await services.imports.validateImport({
        importType,
        fileName,
        csvText,
        mapping,
      });
      setValidation(result);
    } catch (caught) {
      setError(messageFromError(caught));
    } finally {
      setIsBusy(false);
    }
  }

  async function commitCsv() {
    setIsBusy(true);
    setError(null);

    try {
      const job = await services.imports.commitImport({
        importType,
        fileName,
        csvText,
        mapping,
      });
      setCommittedJob(job);
      setConfirmOpen(false);
      await loadHistory();
    } catch (caught) {
      setError(messageFromError(caught));
    } finally {
      setIsBusy(false);
    }
  }

  function changeImportType(nextType: ImportType) {
    setImportType(nextType);
    setCsvText("");
    setFileName("");
    setHeaders(null);
    setMapping({});
    setValidation(null);
    setCommittedJob(null);
    setError(null);
  }

  return (
    <div className="flex flex-1 flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            Import podataka
          </CardTitle>
          <CardDescription>
            CSV migracija artikala, kategorija i pocetnog stanja.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <Progress value={progressValue}>
            <ProgressLabel>Napredak importa</ProgressLabel>
            <ProgressValue>{() => `${progressValue}%`}</ProgressValue>
          </Progress>

          {error ? (
            <Alert variant="destructive">
              <AlertCircleIcon aria-hidden="true" />
              <AlertTitle>Import nije uspeo</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          {committedJob ? (
            <Alert>
              <CheckIcon aria-hidden="true" />
              <AlertTitle>Import zavrsen</AlertTitle>
              <AlertDescription>
                {committedJob.fileName} je upisan sa {committedJob.totalRows} redova.
              </AlertDescription>
            </Alert>
          ) : null}

          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="import-type">Tip importa</FieldLabel>
              <NativeSelect
                id="import-type"
                value={importType}
                onChange={(event) =>
                  changeImportType(event.currentTarget.value as ImportType)
                }
              >
                {Object.entries(IMPORT_TYPE_LABELS).map(([value, label]) => (
                  <NativeSelectOption key={value} value={value}>
                    {label}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
              <FieldDescription>
                Izaberite podatke koje CSV donosi pre izbora fajla.
              </FieldDescription>
            </Field>

            <Field>
              <FieldLabel htmlFor="import-csv-file">CSV fajl</FieldLabel>
              <Input
                id="import-csv-file"
                type="file"
                accept=".csv,text/csv"
                onChange={handleFileChange}
              />
              <FieldDescription>
                Fajl se cita lokalno i salje backend validaciji pre upisa.
              </FieldDescription>
            </Field>
          </FieldGroup>
        </CardContent>
      </Card>

      {headers ? (
        <Card>
          <CardHeader>
            <CardTitle>Mapiranje kolona</CardTitle>
            <CardDescription>
              {fileName} ima {headers.totalRows} redova za proveru.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
              {mappedLabels.map((field) => (
                <Field key={field.key} data-invalid={field.required && !field.header}>
                  <FieldLabel htmlFor={`mapping-${field.key}`}>
                    {field.label}
                  </FieldLabel>
                  <NativeSelect
                    id={`mapping-${field.key}`}
                    value={field.header}
                    aria-invalid={field.required && !field.header}
                    onChange={(event) =>
                      setMapping((current) => ({
                        ...current,
                        [field.key]: event.currentTarget.value,
                      }))
                    }
                  >
                    <NativeSelectOption value="">Ne mapira se</NativeSelectOption>
                    {headers.headers.map((header) => (
                      <NativeSelectOption key={header} value={header}>
                        {header}
                      </NativeSelectOption>
                    ))}
                  </NativeSelect>
                  <FieldDescription>
                    {field.label} -&gt; {field.header || "Nije mapirano"}
                  </FieldDescription>
                </Field>
              ))}
            </div>
          </CardContent>
          <CardFooter className="gap-2">
            <Button
              type="button"
              onClick={validateCsv}
              disabled={!canValidate || isBusy}
            >
              <FileTextIcon data-icon="inline-start" />
              Validiraj CSV
            </Button>
            <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
              <Button
                type="button"
                variant="outline"
                disabled={!canCommit || isBusy}
                onClick={() => setConfirmOpen(true)}
              >
                <UploadIcon data-icon="inline-start" />
                Upisi import
              </Button>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Potvrdi upis</AlertDialogTitle>
                  <AlertDialogDescription>
                    Upis ce biti izvrsen u jednoj transakciji. Ako bilo koji red
                    ne prodje proveru, podaci se ne upisuju.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Odustani</AlertDialogCancel>
                  <AlertDialogAction onClick={commitCsv} disabled={isBusy}>
                    Potvrdi upis
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </CardFooter>
        </Card>
      ) : null}

      {validation ? <ValidationPanel validation={validation} /> : null}

      <ImportHistory history={history} />
    </div>
  );
}

function ValidationPanel({
  validation,
}: {
  validation: ImportValidationResult;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Dry run rezultat</CardTitle>
        <CardDescription>
          Dry run: {validation.summary.create} za kreiranje
          {validation.summary.update ? `, ${validation.summary.update} za azuriranje` : ""}
          {validation.summary.skip ? `, ${validation.summary.skip} za preskakanje` : ""}
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {validation.errorCount > 0 ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>Import ima greske</AlertTitle>
            <AlertDescription>
              Ispravite oznacene redove pre upisa.
            </AlertDescription>
          </Alert>
        ) : (
          <Alert>
            <CheckIcon aria-hidden="true" />
            <AlertTitle>CSV je spreman za upis</AlertTitle>
            <AlertDescription>
              Backend validacija nije pronasla greske.
            </AlertDescription>
          </Alert>
        )}

        {validation.rows.length ? (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Red</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Akcija</TableHead>
                <TableHead>Poruka</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {validation.rows.map((row) => (
                <TableRow key={row.rowNumber}>
                  <TableCell>Red {row.rowNumber}</TableCell>
                  <TableCell>
                    <Badge variant={row.status === "error" ? "destructive" : "secondary"}>
                      {statusLabel(row.status)}
                    </Badge>
                  </TableCell>
                  <TableCell>{actionLabel(row.action ?? "skip")}</TableCell>
                  <TableCell>{row.message || "-"}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <CheckIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>Nema redova sa greskom</EmptyTitle>
              <EmptyDescription>Svi redovi su spremni za upis.</EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}
      </CardContent>
    </Card>
  );
}

function ImportHistory({ history }: { history: ImportJob[] }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Istorija importa</CardTitle>
        <CardDescription>Poslednji upisi podataka u lokalnu bazu.</CardDescription>
      </CardHeader>
      <CardContent>
        {history.length ? (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Fajl</TableHead>
                <TableHead>Tip</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Redovi</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {history.map((job) => (
                <TableRow key={job.id}>
                  <TableCell>{job.fileName}</TableCell>
                  <TableCell>{IMPORT_TYPE_LABELS[job.importType]}</TableCell>
                  <TableCell>
                    <Badge variant={job.status === "failed" ? "destructive" : "outline"}>
                      {job.status === "completed" ? "Zavrsen" : job.status}
                    </Badge>
                  </TableCell>
                  <TableCell>{job.totalRows}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HistoryIcon aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>Nema prethodnih import poslova</EmptyTitle>
              <EmptyDescription>
                Prvi uspesan upis ce se pojaviti ovde.
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent />
          </Empty>
        )}
      </CardContent>
    </Card>
  );
}

function autoMapHeaders(importType: ImportType, headers: string[]) {
  const mapping: ImportMapping = {};

  for (const field of MAPPING_FIELDS[importType]) {
    const aliases = FIELD_ALIASES[field.key] ?? [];
    const match = headers.find((header) =>
      aliases.includes(normalizeHeader(header)),
    );

    if (match) {
      mapping[field.key] = match;
    }
  }

  return mapping;
}

function requiredMappingComplete(importType: ImportType, mapping: ImportMapping) {
  if (importType === "products") {
    return Boolean(
      mapping.name &&
        mapping.sale_price &&
        mapping.vat_rate &&
        (mapping.sku || mapping.barcode),
    );
  }

  if (importType === "initial_stock") {
    return Boolean(mapping.quantity && (mapping.sku || mapping.barcode));
  }

  return Boolean(mapping.name);
}

function normalizeHeader(value: string) {
  return value.trim().toLowerCase();
}

function statusLabel(status: string) {
  if (status === "error") {
    return "Greska";
  }

  if (status === "warning") {
    return "Upozorenje";
  }

  if (status === "imported") {
    return "Upisano";
  }

  return "Ispravno";
}

function actionLabel(action: string) {
  if (action === "update") {
    return "Azuriranje";
  }

  if (action === "skip") {
    return "Preskace se";
  }

  return "Kreiranje";
}

function messageFromError(error: unknown) {
  if (typeof error === "object" && error && "message" in error) {
    return String((error as { message: unknown }).message);
  }

  return "Import trenutno nije moguc.";
}

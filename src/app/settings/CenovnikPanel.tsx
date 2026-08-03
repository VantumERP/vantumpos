import {
  AlertCircleIcon,
  FileTextIcon,
  FolderOpenIcon,
  SaveIcon,
} from "lucide-react";
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
import { Empty, EmptyContent, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
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
import type { CenovnikService } from "@/services/ports";
import type {
  CenovnikPublishTarget,
  CenovnikSnapshot,
  CenovnikSnapshotDetail,
  LegalNotice,
} from "@/services/types";

/**
 * „Cenovnik“ — where the shop's published price list goes, and what it has
 * published so far (ZZP čl. 6, SW-12 reqs. 11, 14, 15, 18).
 *
 * **Four things this panel must not do.**
 *
 * 1. **Never say the shop is in breach.** Čl. 6 st. 2 says *„na svojoj internet
 *    stranici“* and no ZZP provision obliges a trader to have one, so for a shop
 *    without a site the question is open (§2b) — and the fine only applies from
 *    the čl. 220 date, which čl. 210 was not carved out of. „Mesto objave nije
 *    podešeno“ is the whole of what this program knows.
 * 2. **Never author a figure.** Every statutory sum lives in
 *    `src-tauri/src/legal.rs`; this panel renders `notice.penalty` whole, and an
 *    unset legal form (`penalty === null`) points at Podešavanja → Profil rather
 *    than quoting a tier. A preduzetnik must never read the pravno-lice sum.
 * 3. **Never promise the file is on the internet.** A folder on this machine is
 *    where the file lands. Whether the shop's site serves that folder is outside
 *    this program, and čl. 6 st. 5's fetchability contract lives in the publish
 *    target's own doc comment — not in a sentence on a settings screen.
 * 4. **Never offer „objavi sada“.** Čl. 6 st. 3 wants the file to match the
 *    outlet's current prices *„u realnom vremenu“*, so publication rides on the
 *    write that moved a price. A button here would be a second answer to „when
 *    did this shop last publish“, and the one thing an operator could then do
 *    wrong is believe it.
 * 5. **Never say a cenovnik is made on every price change without checking that
 *    one can be.** `publish_current` returns `Ok(None)` and archives NOTHING
 *    while `commands::cenovnik::outlet` is `None` — čl. 6 st. 2 publishes
 *    *„posebno za svaki prodajni objekat“*, so with no outlet there is nothing
 *    to key a lineage on. A fresh install sits exactly there until Podešavanja
 *    → Radnja is saved, so the unqualified sentence would promise a file that
 *    never appears.
 */
interface CenovnikPanelProps {
  cenovnik: CenovnikService;
}

type LoadState =
  | { status: "loading" }
  | {
      status: "ready";
      target: CenovnikPublishTarget;
      snapshots: CenovnikSnapshot[];
      notice: LegalNotice;
      /** `null` while nothing is generated, archived or published at all. */
      outlet: string | null;
    }
  | { status: "error"; message: string };

export function CenovnikPanel({ cenovnik }: CenovnikPanelProps) {
  const [state, setState] = useState<LoadState>({ status: "loading" });
  const [folderDraft, setFolderDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [opened, setOpened] = useState<CenovnikSnapshotDetail | null>(null);

  useEffect(() => {
    let ignore = false;

    Promise.all([
      cenovnik.getPublishTarget(),
      cenovnik.listSnapshots(),
      cenovnik.getNotice(),
      cenovnik.getOutlet(),
    ])
      .then(([target, snapshots, notice, outlet]) => {
        if (!ignore) {
          setState({ status: "ready", target, snapshots, notice, outlet });
          setFolderDraft(target.kind === "localFolder" ? target.folder : "");
        }
      })
      .catch((error) => {
        if (!ignore) {
          setState({
            status: "error",
            message: errorMessage(error, "Podaci o cenovniku nisu učitani."),
          });
        }
      });

    return () => {
      ignore = true;
    };
  }, [cenovnik]);

  async function saveTarget(next: CenovnikPublishTarget) {
    setSaveError(null);
    setSaving(true);
    try {
      const target = await cenovnik.setPublishTarget(next);
      setState((current) =>
        current.status === "ready" ? { ...current, target } : current,
      );
      setFolderDraft(target.kind === "localFolder" ? target.folder : "");
      toast.success("Mesto objave cenovnika je sačuvano.");
    } catch (error) {
      setSaveError(errorMessage(error, "Mesto objave nije sačuvano."));
    } finally {
      setSaving(false);
    }
  }

  async function openSnapshot(snapshotId: number) {
    setSaveError(null);
    try {
      const detail = await cenovnik.getSnapshot(snapshotId);
      setOpened(detail);
      if (!detail) {
        // Purged between the list and the click. Rendering nothing would leave
        // the operator watching a button that does not work.
        setSaveError(
          "Taj cenovnik nije više u arhivi — istekao je rok čuvanja. " +
            "Osvežite stranicu da vidite arhivu kakva je sada.",
        );
      }
    } catch (error) {
      setSaveError(errorMessage(error, "Objavljeni cenovnik nije pročitan."));
    }
  }

  if (state.status === "loading") {
    return (
      <Badge variant="outline" className="w-fit">
        <Spinner data-icon="inline-start" aria-hidden="true" />
        Učitavanje cenovnika
      </Badge>
    );
  }

  if (state.status === "error") {
    return (
      <Alert variant="destructive">
        <AlertCircleIcon aria-hidden="true" />
        <AlertTitle>Cenovnik nije dostupan</AlertTitle>
        <AlertDescription>{state.message}</AlertDescription>
      </Alert>
    );
  }

  const { target, snapshots, notice, outlet } = state;

  return (
    <div className="flex flex-col gap-6">
      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            Cenovnik na internet stranici
          </CardTitle>
          <CardDescription>
            {outlet === null
              ? // Nothing is generated at all in this state, so the plain
                // sentence below would promise a file that never appears.
                "Cenovnik se pravi posebno za svaki prodajni objekat, pa ga program ne pravi dok ne zna koji je to objekat — pogledajte „Mesto objave“ ispod. Kada ga bude znao, praviće ga i obnavljati posle svake izmene cene: ništa po rasporedu i ništa na dugme."
              : `Program pravi mašinski čitljiv cenovnik prodajnog objekta „${outlet}“ i ponovo ga objavljuje posle svake izmene cene. Ništa se ne objavljuje po rasporedu i ništa se ne objavljuje na dugme — cenovnik prati cenu koju kasa naplaćuje.`}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <p className="max-w-prose text-sm">{notice.summary}</p>
          {notice.penalty ? (
            <p className="max-w-prose text-sm">{notice.penalty}</p>
          ) : (
            <p className="max-w-prose text-sm text-muted-foreground">
              Izaberite pravnu formu u Podešavanja → Profil da bi iznos kazne bio
              prikazan. Pogrešan iznos je gori od nikakvog.
            </p>
          )}
          <p className="max-w-prose text-xs text-muted-foreground">
            {notice.citation}
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            Mesto objave
          </CardTitle>
          <CardDescription>
            Folder u koji program upisuje novi cenovnik čim se neka cena promeni
            — pod uslovom da zna za koji prodajni objekat ga pravi. Najčešće je
            to folder koji vaš sajt objavljuje, folder koji sinhronizuje neki
            servis, ili folder iz kojeg fajl šaljete ručno.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {/*
            The precondition for everything below it. `publish_current` returns
            `Ok(None)` and archives nothing while the shop has named neither an
            address nor a shop name, and `load_json_setting` never persists the
            default company row — so a fresh install stays here until the owner
            saves Podešavanja → Radnja, with no other signal anywhere in the
            program that its cenovnik is not being made.
          */}
          {outlet === null ? (
            <Alert>
              <AlertCircleIcon aria-hidden="true" />
              <AlertTitle>Prodajni objekat još nije identifikovan</AlertTitle>
              <AlertDescription>
                <div className="flex flex-col gap-2">
                  <p>
                    Cenovnik se objavljuje posebno za svaki prodajni objekat
                    (čl. 6 st. 2), pa program ne pravi nijedan dok ne zna koji je
                    to objekat. Upišite naziv ili adresu radnje u Podešavanja →
                    Radnja.
                  </p>
                  <p>
                    Do tada se pri izmeni cene ne pravi nijedan cenovnik i arhiva
                    ostaje prazna. Prodaja i izmena cena rade normalno — čl. 6
                    nije razlog da kasa ne radi.
                  </p>
                </div>
              </AlertDescription>
            </Alert>
          ) : null}

          {target.kind === "notConfigured" ? (
            <Alert>
              <FolderOpenIcon aria-hidden="true" />
              <AlertTitle>Nije podešeno mesto objave</AlertTitle>
              <AlertDescription>
                <div className="flex flex-col gap-2">
                  <p>
                    {outlet === null
                      ? "Kada prodajni objekat bude upisan, cenovnik će se praviti i čuvati u arhivi pri svakoj izmeni cene, ali neće odlaziti nigde sa ovog računara."
                      : "Cenovnik se i dalje pravi i čuva u arhivi kad god se cena promeni, ali ne odlazi nigde sa ovog računara."}
                  </p>
                  <p>
                    Zakon nigde ne propisuje obavezu trgovca da ima internet
                    stranicu, pa za trgovca koji je nema nije razjašnjeno da li je
                    dužan da je izradi. Program zato ne donosi zaključak o tome
                    kako se ova obaveza primenjuje na vašu radnju — samo pokazuje
                    šta jeste, a šta nije podešeno.
                  </p>
                </div>
              </AlertDescription>
            </Alert>
          ) : (
            <Alert>
              <FolderOpenIcon aria-hidden="true" />
              <AlertTitle>
                {outlet === null
                  ? "Folder je podešen, ali se u njega još ništa ne upisuje"
                  : "Cenovnik se upisuje u folder"}
              </AlertTitle>
              <AlertDescription>
                <div className="flex flex-col gap-2">
                  <p className="font-medium break-all">{target.folder}</p>
                  {outlet === null ? (
                    <p>
                      Dok prodajni objekat nije identifikovan, cenovnik se ne
                      pravi, pa u ovaj folder ne stiže nijedan fajl.
                    </p>
                  ) : null}
                  <p>
                    Folder na ovom računaru sam po sebi nije objava na internetu.
                    Čl. 6 st. 2 traži cenovnik na internet stranici radnje, pa ovaj
                    folder ispunjava tu obavezu samo ako je to folder koji vaš sajt
                    zaista objavljuje.
                  </p>
                </div>
              </AlertDescription>
            </Alert>
          )}

          {saveError ? (
            <Alert variant="destructive">
              <AlertCircleIcon aria-hidden="true" />
              <AlertDescription>{saveError}</AlertDescription>
            </Alert>
          ) : null}

          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="cenovnik-folder">
                Folder za objavu cenovnika
              </FieldLabel>
              <Input
                id="cenovnik-folder"
                value={folderDraft}
                onChange={(event) => setFolderDraft(event.target.value)}
              />
              <FieldDescription>
                Puna putanja, na primer „/Users/ana/sajt/cenovnik“.
              </FieldDescription>
            </Field>
          </FieldGroup>

          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              disabled={saving}
              onClick={() =>
                void saveTarget({ kind: "localFolder", folder: folderDraft.trim() })
              }
            >
              {saving ? (
                <Spinner data-icon="inline-start" aria-hidden="true" />
              ) : (
                <SaveIcon data-icon="inline-start" aria-hidden="true" />
              )}
              Sačuvaj mesto objave
            </Button>
            {target.kind === "localFolder" ? (
              <Button
                type="button"
                variant="outline"
                disabled={saving}
                onClick={() => void saveTarget({ kind: "notConfigured" })}
              >
                Ukloni mesto objave
              </Button>
            ) : null}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            Objavljeni cenovnici
          </CardTitle>
          <CardDescription>
            Svaka objava se čuva onakva kakva je bila, pa se ranije objavljene cene
            mogu uporediti sa cenama koje su objavljene sada (čl. 6 st. 5). Važeći
            cenovnik prodajnog objekta je uvek najnoviji i ne uklanja se.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {snapshots.length === 0 ? (
            <Empty>
              <EmptyHeader>
                <EmptyTitle>Nijedan cenovnik još nije napravljen</EmptyTitle>
              </EmptyHeader>
              <EmptyContent>
                {outlet === null
                  ? "Cenovnik se ne pravi dok prodajni objekat nije identifikovan: upišite naziv ili adresu radnje u Podešavanja → Radnja. Posle toga prvi cenovnik nastaje čim se promeni neka cena — u šifarniku, kroz akciju, kroz uvoz ili nivelacijom."
                  : "Prvi cenovnik nastaje čim se promeni neka cena — u šifarniku, kroz akciju, kroz uvoz ili nivelacijom."}
              </EmptyContent>
            </Empty>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Napravljen</TableHead>
                  <TableHead>Artikala</TableHead>
                  <TableHead>Objava</TableHead>
                  <TableHead>Otisak fajla</TableHead>
                  <TableHead>Fajl</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {snapshots.map((row) => (
                  <TableRow key={row.id}>
                    <TableCell className="whitespace-nowrap">
                      {formatInstant(row.generatedAt)}
                      {row.current ? (
                        <Badge variant="outline" className="ml-2">
                          Važeći
                        </Badge>
                      ) : null}
                    </TableCell>
                    <TableCell>{row.rowCount}</TableCell>
                    <TableCell className="text-xs">
                      {row.publishedAt
                        ? `${formatInstant(row.publishedAt)} — ${row.publishedTarget ?? ""}`
                        : "Nije objavljen — arhiviran"}
                    </TableCell>
                    <TableCell className="font-mono text-xs break-all">
                      {row.contentHash.slice(0, 12)}
                    </TableCell>
                    <TableCell>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        // The date rides on the accessible name, not on the
                        // visible label: the archive holds many rows and „Prikaži“
                        // alone would name none of them for a screen reader.
                        aria-label={`Prikaži cenovnik napravljen ${formatInstant(row.generatedAt)}`}
                        onClick={() => void openSnapshot(row.id)}
                      >
                        <FileTextIcon data-icon="inline-start" aria-hidden="true" />
                        Prikaži
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}

          {opened ? (
            <div className="flex flex-col gap-2">
              <p className="text-xs text-muted-foreground">
                Cenovnik napravljen {formatInstant(opened.snapshot.generatedAt)} —
                fajl je prikazan tačno onakav kakav je arhiviran.
              </p>
              <pre
                aria-label="Sadržaj objavljenog cenovnika"
                className="max-h-64 overflow-auto rounded-md border bg-muted p-3 font-mono text-xs"
              >
                {opened.body}
              </pre>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}

/**
 * An RFC3339 instant as a Serbian operator reads it. The archive's identity is
 * the stamp the backend recorded; this is display only, and nothing here decides
 * anything from the formatted string.
 */
function formatInstant(value: string): string {
  const parsed = new Date(value);

  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("sr-Latn-RS", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(parsed);
}

/**
 * A backend refusal, verbatim. „Putanja do foldera mora biti puna putanja …“ is
 * the whole answer the operator needs, and a generic sentence would hide the one
 * fact that would let them fix it.
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

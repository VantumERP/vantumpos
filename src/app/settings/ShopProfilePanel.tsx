import {
  ExternalLinkIcon,
  PlusIcon,
  SaveIcon,
  Trash2Icon,
  TriangleAlertIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Separator } from "@/components/ui/separator";
import type {
  EsirElement,
  EsirTip,
  LegalNotice,
  PravnaForma,
  ShopProfile,
} from "@/services/types";

/** Uredba 32/2021 čl. 11 st. 1 — the registry the čl. 6 st. 8 check is made against. */
const REGISTAR_URL =
  "https://www.purs.gov.rs/sr/eFiskalizacija/registar-odobrenih-elemenata-efu.html";

/** Approval attaches to a version string, so a one-time check decays. */
const RECHECK_AFTER_MS = 365 * 24 * 60 * 60 * 1000;

/**
 * Mirrors `legal::lpfr_required` in wording and citation only, and is used only
 * when the backend notice has not been supplied.
 *
 * `penalty` is `null` and always will be: every statutory fine figure lives in
 * `src-tauri/src/legal.rs` and nowhere else, so a second copy here could
 * silently drift out of tier. A `null` penalty is the one answer that can never
 * be the wrong one — and it must never turn the panel silent, because silence
 * after a „ne“ is exactly what ZF čl. 6 st. 4 must not be met with.
 */
const LPFR_NOTICE_FALLBACK: LegalNotice = {
  summary:
    "U svakom poslovnom prostoru i poslovnoj prostoriji mora da radi najmanje " +
    "jedan lokalni procesor fiskalnih računa (L-PFR) — uređaj koji izdaje račun " +
    "i bez interneta. Zakon izuzima samo obveznika koji promet na malo obavlja " +
    "isključivo putem interneta i obveznika koji obavlja promet na malo " +
    "sopstvenih korišćenih pokretnih materijalnih sredstava.",
  penalty: null,
  citation: "Zakon o fiskalizaciji, čl. 6 st. 4; prekršaj: čl. 15 st. 1 tač. 4.",
  isLegalDuty: true,
};

interface ShopProfilePanelProps {
  profile: ShopProfile;
  onSave: (request: ShopProfile) => Promise<void> | void;
  /**
   * Opens the registry in the system browser. A `target="_blank"` anchor is a
   * no-op inside the Tauri webview, so the link must go through the opener.
   */
  onOpenRegistry?: (url: string) => Promise<void> | void;
  /**
   * `settings_lpfr_notice` — the ZF čl. 6 st. 4 duty with the penalty resolved
   * against the **stored** legal form. The panel never derives a figure; it
   * decides only *whether* the duty is engaged, from the live answers.
   */
  lpfrNotice?: LegalNotice | null;
}

/** `nije` is a real answer state, never a silent `false`. */
type TriState = "da" | "ne" | "nije";

function toTriState(value: boolean | null): TriState {
  if (value === null) {
    return "nije";
  }

  return value ? "da" : "ne";
}

function fromTriState(value: TriState): boolean | null {
  if (value === "nije") {
    return null;
  }

  return value === "da";
}

function isRecheckDue(checkedOn: string | null) {
  if (!checkedOn) {
    return false;
  }

  const checkedAt = Date.parse(checkedOn);
  if (Number.isNaN(checkedAt)) {
    return false;
  }

  return Date.now() - checkedAt > RECHECK_AFTER_MS;
}

function formatDate(value: string) {
  const parsed = Date.parse(value);
  if (Number.isNaN(parsed)) {
    return value;
  }

  return new Intl.DateTimeFormat("sr-Latn-RS", { dateStyle: "short" }).format(
    new Date(parsed),
  );
}

function errorMessage(error: unknown, fallback: string) {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  if (error instanceof Error && error.message) {
    return error.message;
  }

  return fallback;
}

export function ShopProfilePanel({
  profile,
  onSave,
  onOpenRegistry,
  lpfrNotice,
}: ShopProfilePanelProps) {
  const [form, setForm] = useState<ShopProfile>(profile);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  /**
   * ZF čl. 6 st. 4. The duty stands unless a carve-out is affirmatively
   * claimed: `!== true` keeps it standing for an unanswered carve-out as well
   * as a denied one, because silence is not one of the two exemptions the
   * statute grants.
   */
  const lpfrDutyEngaged =
    form.lpfrInPremises === false &&
    form.lpfrCarveOutInternetOnly !== true &&
    form.lpfrCarveOutOwnUsedAssets !== true;

  const notice = lpfrNotice ?? LPFR_NOTICE_FALLBACK;
  /**
   * The figure was resolved for the legal form as **stored**. While an unsaved
   * radio says something else, quoting it would put the other tier's range in
   * front of the operator — the one mistake this module exists to prevent.
   */
  const lpfrPenalty =
    form.pravnaForma === profile.pravnaForma ? notice.penalty : null;

  useEffect(() => {
    setForm(profile);
  }, [profile]);

  function updateElement(index: number, patch: Partial<EsirElement>) {
    setForm((current) => ({
      ...current,
      esirElements: current.esirElements.map((element, position) =>
        position === index ? { ...element, ...patch } : element,
      ),
    }));
  }

  async function handleOpenRegistry() {
    setError(null);
    try {
      await onOpenRegistry?.(REGISTAR_URL);
    } catch (openError) {
      setError(
        errorMessage(
          openError,
          "Registar nije otvoren. Otvorite adresu ručno u pregledaču.",
        ),
      );
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    const incomplete = form.esirElements.some(
      (element) =>
        !element.naziv.trim() || !element.verzija.trim() || !element.ib.trim(),
    );
    if (incomplete) {
      setError("Naziv, verzija i IB elementa su obavezni.");
      return;
    }

    setSaving(true);
    try {
      await onSave({
        pravnaForma: form.pravnaForma,
        pdvObveznik: form.pdvObveznik,
        distanceSelling: form.distanceSelling,
        lpfrInPremises: form.lpfrInPremises,
        lpfrCarveOutInternetOnly: form.lpfrCarveOutInternetOnly,
        lpfrCarveOutOwnUsedAssets: form.lpfrCarveOutOwnUsedAssets,
        esirElements: form.esirElements,
      });
    } catch (saveError) {
      setError(errorMessage(saveError, "Profil radnje nije sačuvan."));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form className="flex flex-col gap-6" onSubmit={handleSubmit}>
      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            Profil radnje
          </CardTitle>
          <CardDescription>
            Odgovori koji određuju koje obaveze važe za ovu radnju. Ništa se ne
            pretpostavlja — dok odgovor nije unet, program ga tretira kao
            neodgovoren.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            {error ? <FieldError>{error}</FieldError> : null}

            <FieldSet>
              <FieldLegend>Pravna forma</FieldLegend>
              <FieldDescription>
                Iznosi kazni se ne prikazuju dok se ne unese pravna forma —
                pogrešan iznos je gori od nikakvog.
              </FieldDescription>
              <RadioGroup
                aria-label="Pravna forma"
                value={form.pravnaForma ?? ""}
                onValueChange={(value) =>
                  setForm((current) => ({
                    ...current,
                    pravnaForma: (value as PravnaForma | "") || null,
                  }))
                }
              >
                <Field orientation="horizontal">
                  <RadioGroupItem
                    id="shop-profile-forma-preduzetnik"
                    value="preduzetnik"
                  />
                  <FieldContent>
                    <FieldLabel
                      htmlFor="shop-profile-forma-preduzetnik"
                      className="font-normal"
                    >
                      Preduzetnik
                    </FieldLabel>
                    <FieldDescription>
                      Fizičko lice registrovano za obavljanje delatnosti.
                    </FieldDescription>
                  </FieldContent>
                </Field>
                <Field orientation="horizontal">
                  <RadioGroupItem
                    id="shop-profile-forma-pravno-lice"
                    value="pravno_lice"
                  />
                  <FieldContent>
                    <FieldLabel
                      htmlFor="shop-profile-forma-pravno-lice"
                      className="font-normal"
                    >
                      Pravno lice
                    </FieldLabel>
                    <FieldDescription>
                      Privredno društvo (d.o.o., a.d., ortačko ili komanditno
                      društvo).
                    </FieldDescription>
                  </FieldContent>
                </Field>
              </RadioGroup>
            </FieldSet>

            <Separator />

            <TriStateField
              idPrefix="shop-profile-pdv"
              legend="PDV obveznik"
              description="Da li je radnja u sistemu PDV-a?"
              ariaPrefix="PDV obveznik"
              unansweredLabel="Nije upisano"
              value={toTriState(form.pdvObveznik)}
              onChange={(value) =>
                setForm((current) => ({
                  ...current,
                  pdvObveznik: fromTriState(value),
                }))
              }
            />

            <Separator />

            <TriStateField
              idPrefix="shop-profile-daljina"
              legend="Prodaja na daljinu"
              description="Da li radnja prodaje i van prodajnog objekta — veb prodavnica, Instagram, Viber porudžbine, katalog? Odgovor menja koji su podaci o proizvodu obavezni pre kupovine (ZoT čl. 34 st. 5)."
              ariaPrefix="Prodaja na daljinu"
              unansweredLabel="Nije odgovoreno"
              value={toTriState(form.distanceSelling)}
              onChange={(value) =>
                setForm((current) => ({
                  ...current,
                  distanceSelling: fromTriState(value),
                }))
              }
            />

            {form.distanceSelling === null ? (
              <Alert>
                <TriangleAlertIcon aria-hidden="true" />
                <AlertTitle>Odgovorite na pitanje o prodaji na daljinu</AlertTitle>
                <AlertDescription>
                  Dok odgovor izostaje, program ne može da zna da li su podaci o
                  proizvođaču, uvozniku i zemlji porekla obavezni ili samo
                  preporučeni. Ćutanje se ne računa kao „ne”.
                </AlertDescription>
              </Alert>
            ) : null}

            <Separator />

            <TriStateField
              idPrefix="shop-profile-lpfr"
              legend="Lokalni PFR u ovom poslovnom prostoru"
              description="Da li u ovom poslovnom prostoru radi najmanje jedan lokalni PFR — uređaj koji izdaje račun i bez interneta (ZF čl. 6 st. 3 i st. 4)?"
              ariaPrefix="Lokalni PFR"
              unansweredLabel="Nije provereno"
              value={toTriState(form.lpfrInPremises)}
              onChange={(value) =>
                setForm((current) => ({
                  ...current,
                  lpfrInPremises: fromTriState(value),
                }))
              }
            />

            <TriStateField
              idPrefix="shop-profile-lpfr-internet"
              legend="Prodaja isključivo preko interneta"
              description="Da li ova radnja obavlja promet na malo isključivo putem interneta? Samo tada ZF čl. 6 st. 4 ne traži lokalni PFR u prostoru."
              ariaPrefix="Prodaja isključivo preko interneta"
              unansweredLabel="Bez odgovora"
              value={toTriState(form.lpfrCarveOutInternetOnly)}
              onChange={(value) =>
                setForm((current) => ({
                  ...current,
                  lpfrCarveOutInternetOnly: fromTriState(value),
                }))
              }
            />

            <TriStateField
              idPrefix="shop-profile-lpfr-sopstvena"
              legend="Prodaja sopstvenih korišćenih sredstava"
              description="Da li je promet ove radnje promet na malo sopstvenih korišćenih pokretnih materijalnih sredstava? To je drugi izuzetak iz ZF čl. 6 st. 4."
              ariaPrefix="Prodaja sopstvenih korišćenih sredstava"
              unansweredLabel="Bez odgovora"
              value={toTriState(form.lpfrCarveOutOwnUsedAssets)}
              onChange={(value) =>
                setForm((current) => ({
                  ...current,
                  lpfrCarveOutOwnUsedAssets: fromTriState(value),
                }))
              }
            />

            {lpfrDutyEngaged ? (
              <Alert variant="destructive">
                <TriangleAlertIcon aria-hidden="true" />
                <AlertTitle>
                  Bez lokalnog PFR-a u ovom prostoru — obaveza iz ZF čl. 6 st. 4
                </AlertTitle>
                <AlertDescription>
                  <div className="flex flex-col gap-2">
                    <p>{notice.summary}</p>
                    {lpfrPenalty ? (
                      <p>{lpfrPenalty}</p>
                    ) : (
                      <p>
                        Izaberite pravnu formu iznad i sačuvajte profil da bi
                        iznos kazne bio prikazan.
                      </p>
                    )}
                    <p className="text-xs">{notice.citation}</p>
                  </div>
                </AlertDescription>
              </Alert>
            ) : null}
          </FieldGroup>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle role="heading" aria-level={2}>
            ZF čl. 6 st. 8: proveriti pre otpočinjanja korišćenja
          </CardTitle>
          <CardDescription>
            Ovo je interna beleška o proveri. Nijedan propis ne zahteva da radnja
            čuva ove podatke.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <p className="text-xs text-muted-foreground">
            U registru Poreske uprave uporedite naziv, verziju i IB elementa sa
            onim što uređaj prijavljuje i proverite da je polje „Broj i datum
            rešenja o ukidanju odobrenja” prazno.
          </p>
          <div>
            <Button
              type="button"
              variant="link"
              size="sm"
              className="px-0"
              onClick={handleOpenRegistry}
            >
              Registar odobrenih elemenata EFU (Poreska uprava)
              <ExternalLinkIcon aria-hidden="true" />
            </Button>
          </div>
          <p className="text-[0.625rem] break-all text-muted-foreground">
            {REGISTAR_URL}
          </p>

          {form.esirElements.length === 0 ? (
            <p className="text-xs text-muted-foreground">
              Nijedan element još nije zabeležen.
            </p>
          ) : null}

          {form.esirElements.map((element, index) => (
            <div
              key={index}
              className="flex flex-col gap-3 rounded-md border p-3"
              aria-label={`Element ${index + 1}`}
              role="group"
            >
              <FieldGroup className="grid gap-3 md:grid-cols-2">
                <Field>
                  <FieldLabel htmlFor={`esir-naziv-${index}`}>Naziv</FieldLabel>
                  <Input
                    id={`esir-naziv-${index}`}
                    value={element.naziv}
                    onChange={(event) =>
                      updateElement(index, { naziv: event.target.value })
                    }
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor={`esir-verzija-${index}`}>
                    Verzija
                  </FieldLabel>
                  <Input
                    id={`esir-verzija-${index}`}
                    value={element.verzija}
                    onChange={(event) =>
                      updateElement(index, { verzija: event.target.value })
                    }
                  />
                  <FieldDescription>
                    Odobrenje važi za tačnu verziju iz registra.
                  </FieldDescription>
                </Field>
                <Field>
                  <FieldLabel htmlFor={`esir-ib-${index}`}>
                    IB elementa
                  </FieldLabel>
                  <Input
                    id={`esir-ib-${index}`}
                    value={element.ib}
                    onChange={(event) =>
                      updateElement(index, { ib: event.target.value })
                    }
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor={`esir-tip-${index}`}>Tip</FieldLabel>
                  <NativeSelect
                    id={`esir-tip-${index}`}
                    className="w-full"
                    value={element.tip}
                    onChange={(event) =>
                      updateElement(index, {
                        tip: event.target.value as EsirTip,
                      })
                    }
                  >
                    <NativeSelectOption value="ESIR">ESIR</NativeSelectOption>
                    <NativeSelectOption value="LPFR">LPFR</NativeSelectOption>
                  </NativeSelect>
                </Field>
              </FieldGroup>

              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-xs text-muted-foreground">
                  {element.checkedOn
                    ? `Poslednja provera: ${formatDate(element.checkedOn)}`
                    : "Provera još nije potvrđena."}
                </span>
                <div className="flex gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      updateElement(index, {
                        checkedOn: new Date().toISOString(),
                      })
                    }
                  >
                    Potvrdi proveru danas
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      setForm((current) => ({
                        ...current,
                        esirElements: current.esirElements.filter(
                          (_, position) => position !== index,
                        ),
                      }))
                    }
                  >
                    <Trash2Icon aria-hidden="true" />
                    Ukloni element
                  </Button>
                </div>
              </div>

              {isRecheckDue(element.checkedOn) ? (
                <Alert>
                  <TriangleAlertIcon aria-hidden="true" />
                  <AlertTitle>Provera je starija od godinu dana</AlertTitle>
                  <AlertDescription>
                    Odobrenje se vezuje za verziju, pa tiha nadogradnja može da
                    ga poništi. Uporedite podatke sa registrom ponovo.
                  </AlertDescription>
                </Alert>
              ) : null}
            </div>
          ))}

          <div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() =>
                setForm((current) => ({
                  ...current,
                  esirElements: [
                    ...current.esirElements,
                    {
                      naziv: "",
                      verzija: "",
                      ib: "",
                      tip: "ESIR",
                      checkedOn: null,
                    },
                  ],
                }))
              }
            >
              <PlusIcon aria-hidden="true" />
              Dodaj element
            </Button>
          </div>
        </CardContent>
      </Card>

      <div>
        <Button type="submit" disabled={saving}>
          <SaveIcon aria-hidden="true" />
          Sačuvaj profil
        </Button>
      </div>
    </form>
  );
}

function TriStateField({
  idPrefix,
  legend,
  description,
  ariaPrefix,
  unansweredLabel,
  value,
  onChange,
}: {
  idPrefix: string;
  legend: string;
  description: string;
  ariaPrefix: string;
  unansweredLabel: string;
  value: TriState;
  onChange: (value: TriState) => void;
}) {
  return (
    <FieldSet>
      <FieldLegend>{legend}</FieldLegend>
      <FieldDescription>{description}</FieldDescription>
      <RadioGroup
        aria-label={legend}
        value={value}
        onValueChange={(next) => onChange(next as TriState)}
      >
        {[
          { value: "da" as const, label: "Da" },
          { value: "ne" as const, label: "Ne" },
          { value: "nije" as const, label: unansweredLabel },
        ].map((option) => (
          <Field key={option.value} orientation="horizontal">
            <RadioGroupItem
              id={`${idPrefix}-${option.value}`}
              value={option.value}
            />
            <FieldLabel
              htmlFor={`${idPrefix}-${option.value}`}
              className="font-normal"
            >
              <span className="sr-only">{`${ariaPrefix}:`}</span>
              {` ${option.label}`}
            </FieldLabel>
          </Field>
        ))}
      </RadioGroup>
    </FieldSet>
  );
}

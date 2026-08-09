import { nazivPerioda } from "@/lib/period";

import type { PosServices } from "./ports";
import type {
  AmlAssessment,
  AuditEvent,
  AuditQuery,
  AuditSearchResult,
  AuthSession,
  BackupJob,
  BackupSettings,
  Breach,
  BreachDraft,
  CampaignInput,
  CampaignItemView,
  CampaignView,
  CashDepositCalendar,
  CashDepositReport,
  CategorySummary,
  CenovnikPublishTarget,
  CenovnikSnapshot,
  CompanySettings,
  CreateBackupRequest,
  DeclarationGapReason,
  DeclarationGapRow,
  DeclarationWarning,
  DepositBucket,
  EmployeeProfile,
  EurRate,
  EurRateStatus,
  ImportJob,
  IzvestajElementId,
  IzvestajNarativ,
  IzvestajZbir,
  KomisijaClanView,
  LegalNotice,
  InventoryAdjustmentRequest,
  KepClosure,
  KepClosureView,
  KepEntryView,
  NivelacijaObavestenje,
  PopisLineView,
  PopisLista,
  PopisPodesavanja,
  PopisSessionView,
  PopisSignatureView,
  PopisStatus,
  PopisVrsta,
  PrethodnaCenaDto,
  PriceDivergence,
  ProcessingActivity,
  ProductLedgerMovement,
  ProductListQuery,
  ProductLookupSuggestion,
  ProductSummary,
  ReceiptDetail,
  ReceiptSettings,
  ReklamacijaInput,
  ReklamacijaRegime,
  ReklamacijaView,
  RestoreBackupRequest,
  RetentionPolicy,
  SaleDraftRequest,
  SalePreview,
  ShiftSummary,
  ShopProfile,
  StockListItem,
  SupportSession,
  TaxRate,
  UserAccount,
  TaxRateSummary,
  AbsenceCategory,
  SaveWorkTimeEntryRequest,
  WorkTimeEntryView,
  WorkTimeMinutes,
  WorkTimeMonth,
} from "./types";

const now = "2026-06-18T10:00:00Z";
/**
 * The day this double judges rate staleness against. Frozen to `now` rather
 * than read off the wall clock so `isStale` is reproducible — the real backend
 * uses `commands::settings::today_utc`.
 */
const MOCK_TODAY = now.slice(0, 10);

/** An employee nobody has profiled yet — every čl. 87–91 input unset. */
const prazanProfilZaposlenog: EmployeeProfile = {
  datumRodjenja: null,
  datumRodjenjaNajmladjegDeteta: null,
  samohraniRoditelj: null,
  deteTezakInvalid: null,
  trudnocaIliDojenje: null,
  trudnocaIliDojenjeOd: null,
  radiUPreraspodeli: false,
  ugovorenoRadnoVremeMinutaNedeljno: null,
  zanimanjeSifra: null,
  kvalifikacijaSifra: null,
  saglasnostPrekovremeniOd: null,
};

const AML_CAP_EUR = 10_000;
const AML_SOFT_RATIO_PERCENT = 80;
/** `aml.rs::AML_FALLBACK_RATE_MINOR` — a 100,00 RSD/EUR floor, in para. */
const AML_FALLBACK_RATE_MINOR = 10_000;

/** The demo rate, dated to `now` so the till shows no staleness warning. */
const DEFAULT_MOCK_EUR_RATE: EurRate = {
  rateMinor: 11723,
  rateDate: MOCK_TODAY,
  source: "nbs",
};

/**
 * `commands::settings::MANUAL_RATE_BAND_PARA` — 50–500 RSD/EUR in para. A typo
 * guard, **not a legal figure**: no statute names it. It catches the extra
 * digit, which is the dangerous direction, because a tenfold rate multiplies
 * the AML čl. 46 st. 1 dinar threshold by ten and lets an unlawful cash amount
 * through unwarned.
 */
const MANUAL_RATE_MIN_PARA = 5_000;
const MANUAL_RATE_MAX_PARA = 50_000;

/**
 * A test double, not demo copy: the wording a shop actually reads is composed
 * by `crate::popis::nivelacija_obavestenje`. What is kept here is the shape
 * plus the two facts a UI test may assert on — that the popis duty rides back
 * with the price change (SW-16 req. 33) and that the narrowed scope is labelled
 * a **preporuka**, never an obaveza.
 *
 * One constant and not two copies: the KEP hook and the popis module read the
 * same object, exactly as they read one backend string.
 */
const MOCK_NIVELACIJA_OBAVESTENJE: NivelacijaObavestenje = {
  obaveza:
    "Promena prodajnih cena u maloprodajnom objektu traži popis (ZoRač čl. 21, PoP čl. 3).",
  pravniOsnov: "ZoRač čl. 21, PoP čl. 3",
  rokDana: 30,
  rokObjasnjenje:
    "Izveštaj o popisu po nivelaciji sastavlja se najkasnije 30 dana po izvršenom popisu (PoP čl. 13 st. 2).",
  obuhvat: [
    {
      obuhvat: "samo_nivelisani",
      naziv: "samo artikli obuhvaćeni nivelacijom",
      pravniStatus: "preporuka — nije zakonska obaveza",
      obrazlozenje:
        "Sužavanje obima je preporuka i nije zakonska obaveza — obim slobodno proširite.",
      podrazumevani: true,
    },
    {
      obuhvat: "ceo_objekat",
      naziv: "ceo maloprodajni objekat",
      pravniStatus: "najšire tumačenje — ni ono nije propisano",
      obrazlozenje: "Popis celog objekta ne izostavlja ništa.",
      podrazumevani: false,
    },
  ],
  napomena: "Aplikacija ne otvara popis umesto vas (ZoRač čl. 20 st. 3).",
};

/** One popis as this double stores it, before the čl. 8 st. 5 filter runs. */
interface MockPopis {
  id: number;
  vrsta: PopisVrsta;
  prodajnoMesto: string;
  datumPopisa: string;
  periodFrom: string | null;
  periodTo: string | null;
  status: PopisStatus;
  planRadaJson: string | null;
  planRadaOdobrio: string | null;
  planRadaOdobrenoAt: string | null;
  odlukaRef: string | null;
  odlukaDonetaAt: string | null;
  perpetualOdlukaRef: string | null;
  uskladjivanjePotvrdjenoAt: string | null;
  postedAt: string | null;
  komisija: KomisijaClanView[];
  potpisi: PopisSignatureView[];
  linije: PopisLineView[];
}

/** The six req. 36 liste with the provision that requires each. */
const POPIS_LISTE: { vrsta: PopisLista; naziv: string; pravniOsnov: string }[] = [
  { vrsta: "roba", naziv: "roba u objektu", pravniOsnov: "PoP čl. 9 st. 1 t. 1" },
  {
    vrsta: "ostecena",
    naziv: "oštećena, zastarela i neupotrebljiva roba",
    pravniOsnov: "PoP čl. 10 st. 3",
  },
  {
    vrsta: "van_objekta",
    naziv: "roba van objekta (na popravci i kod trećeg lica)",
    pravniOsnov: "PoP čl. 10 st. 4",
  },
  { vrsta: "gotovina", naziv: "gotovina po apoenima", pravniOsnov: "PoP čl. 11 st. 1" },
  {
    vrsta: "potrazivanja",
    naziv: "nedokumentovana potraživanja i obaveze",
    pravniOsnov: "PoP čl. 12 st. 2",
  },
  {
    vrsta: "konsignacija",
    naziv: "konsignaciona i druga tuđa roba",
    pravniOsnov: "PoP čl. 2 st. 5",
  },
];

/**
 * The eight PoP čl. 13 st. 1 elements. `polje` is empty for the three the popis
 * itself answers — they are read out of the liste and never typed.
 */
const POPIS_IZVESTAJ_ELEMENTI: {
  element: IzvestajElementId;
  naziv: string;
  pravniOsnov: string;
  uputstvo: string;
  narativni: boolean;
  polje: string;
}[] = [
  {
    element: "stvarno_stanje",
    naziv: "stvarno stanje utvrđeno popisom",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo:
      "Iz popisnih listi. Izveštaj iskazuje vrednost prebrojanog stanja i broj stavki; prebrojane količine stoje po stavkama na popisnim listama.",
    narativni: false,
    polje: "",
  },
  {
    element: "knjigovodstveno_stanje",
    naziv: "knjigovodstveno stanje",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo:
      "Iz knjiga, tek posle potpisa stvarnog stanja (PoP čl. 8 st. 5). Izveštaj iskazuje vrednost knjigovodstvenog stanja; knjigovodstvene količine stoje po stavkama na popisnim listama.",
    narativni: false,
    polje: "",
  },
  {
    element: "razlike",
    naziv: "razlike između stvarnog i knjigovodstvenog stanja",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo:
      "Iz obračuna. Izveštaj iskazuje vrednosnu razliku i broj stavki sa viškom odnosno manjkom; naturalne razlike stoje po stavkama na popisnim listama.",
    narativni: false,
    polje: "",
  },
  {
    element: "uzroci_neslaganja",
    naziv: "uzroci neslaganja stvarnog i knjigovodstvenog stanja",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo: "Navedite zbog čega se stvarno i knjigovodstveno stanje razlikuju.",
    narativni: true,
    polje: "uzrociNeslaganja",
  },
  {
    element: "predlozi_za_likvidaciju_razlika",
    naziv: "predlozi za likvidaciju utvrđenih razlika",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo:
      "Obuhvata prebijanje manjkova i viškova po osnovu zamena, način naknađivanja manjkova i prihodovanja viškova, otpis zastarelih potraživanja i prihodovanje zastarelih obaveza.",
    narativni: true,
    polje: "predloziZaLikvidacijuRazlika",
  },
  {
    element: "nacin_knjizenja",
    naziv: "način knjiženja razlika",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo: "Navedite kako se utvrđene razlike knjiže.",
    narativni: true,
    polje: "nacinKnjizenja",
  },
  {
    element: "primedbe_lica_koja_rukuju_vrednostima",
    naziv: "primedbe i objašnjenja lica koja rukuju vrednostima",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo:
      "Unesite primedbe i objašnjenja lica koja rukuju vrednostima. Ako ih nema, upišite i to.",
    narativni: true,
    polje: "primedbeLicaKojaRukujuVrednostima",
  },
  {
    element: "ostale_primedbe_i_predlozi",
    naziv: "ostale primedbe i predlozi",
    pravniOsnov: "PoP čl. 13 st. 1",
    uputstvo: "Ostale primedbe i predlozi. Ako ih nema, upišite i to.",
    narativni: true,
    polje: "ostalePrimedbeIPredlozi",
  },
];

/** `crate::popis::konsignacija_rok` — čl. 2 st. 6, ten calendar days. */
function popisPlusDana(datum: string, dana: number): string {
  const parsed = Date.parse(`${datum}T00:00:00Z`);
  if (Number.isNaN(parsed)) {
    return datum;
  }
  return new Date(parsed + dana * 86_400_000).toISOString().slice(0, 10);
}

/** `nbs_rate::is_iso_date` — `YYYY-MM-DD`, month 1–12, day 1–31. */
function isIsoDate(value: string): boolean {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) {
    return false;
  }
  const month = Number(match[2]);
  const day = Number(match[3]);

  return month >= 1 && month <= 12 && day >= 1 && day <= 31;
}

/** Verbatim `commands::inventory::DECLARATION_ADVISORY`. One sentence, one
 *  wording: two copies would let one surface soften what the other hardened. */
const DECLARATION_ADVISORY =
  "U sistemu nisu evidentirani podaci sa deklaracije. Ako roba fizički nosi " +
  "ispravnu deklaraciju, prekršaja nema — unesite podatke sa deklaracije ili " +
  "evidentirajte proveru deklaracije (olakšavajuća okolnost, čl. 69a).";

/**
 * Mirrors `legal::declaration_missing` in wording and citation only.
 *
 * `penalty` is deliberately `null` whatever the legal form: every statutory
 * fine figure lives in `src-tauri/src/legal.rs` and nowhere else, so a second
 * copy in this double could silently drift out of tier. A `null` penalty is the
 * one answer that can never be the wrong one.
 */
/**
 * Mirrors `legal::lpfr_required` in wording and citation only.
 *
 * `penalty` is `null` whatever the legal form, for the same reason it is null
 * in every other double here: `src-tauri/src/legal.rs` is the only place a fine
 * figure may be decided, and a second copy could silently drift out of tier.
 */
const lpfrRequiredNotice: LegalNotice = {
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

const declarationMissingNotice: LegalNotice = {
  summary:
    "Prodaja robe bez deklaracije. Deklaraciju obezbeđuje proizvođač, " +
    "odnosno uvoznik, ali za prodaju takve robe odgovara trgovac.",
  penalty: null,
  citation: "Zakon o trgovini, čl. 34 st. 1–2, čl. 68 st. 1 tač. 9.",
  isLegalDuty: true,
};

/** Mirrors `legal::overtime_record_missing`. `penalty` null for the same reason. */
const overtimeRecordMissingNotice: LegalNotice = {
  summary:
    "Poslodavac je dužan da vodi dnevnu evidenciju o prekovremenom radu zaposlenih.",
  penalty: null,
  citation:
    "Zakon o radu, čl. 55 st. 6. Nadzor: inspektor rada. " +
    "Ovi članovi ne propisuju zaštitnu meru.",
  isLegalDuty: true,
};

/** Mirrors `legal::overtime_caps_exceeded`. `penalty` null for the same reason. */
const overtimeCapsExceededNotice: LegalNotice = {
  summary:
    "Prekovremeni rad ne može trajati duže od osam časova nedeljno, " +
    "niti ukupno radno vreme sa prekovremenim duže od 12 časova dnevno.",
  penalty: null,
  citation:
    "Zakon o radu, čl. 53 st. 2 i st. 3. Nadzor: inspektor rada. " +
    "Ovi članovi ne propisuju zaštitnu meru.",
  isLegalDuty: true,
};

/** Mirrors `legal::preraspodela_caps_exceeded`. `penalty` null for the same reason. */
const preraspodelaCapsExceededNotice: LegalNotice = {
  summary:
    "U slučaju preraspodele radnog vremena, radno vreme ne može da traje duže " +
    "od 60 časova nedeljno. Časovi ostvareni u preraspodeli ne smatraju se " +
    "prekovremenim radom (čl. 58).",
  penalty: null,
  citation:
    "Zakon o radu, čl. 57 st. 5. Nadzor: inspektor rada. " +
    "Ovi članovi ne propisuju zaštitnu meru.",
  isLegalDuty: true,
};

/**
 * Mirrors `legal::breach_notification_missing` at the preduzetnik tier — the
 * **notification** exposure, never a figure for the documentation-only failure
 * under čl. 52 st. 6 (req. 50).
 */
const breachNotificationNotice: LegalNotice = {
  summary:
    "Rukovalac je dužan da o povredi podataka o ličnosti koja može da proizvede rizik po " +
    "prava i slobode fizičkih lica obavesti Poverenika bez nepotrebnog odlaganja, a " +
    "najkasnije u roku od 72 časa od saznanja za povredu. Ako ne postupi u tom roku, dužan " +
    "je da obrazloži zašto.",
  penalty:
    "Prekršaj: novčana kazna od 20.000 do 500.000 dinara " +
    "(čl. 95 st. 1 tač. 24 u vezi sa st. 4).",
  citation:
    "Zakon o zaštiti podataka o ličnosti, čl. 52 st. 1 i st. 2; interna dokumentacija: " +
    "čl. 52 st. 6 i st. 7. Rok i obrazac: Pravilnik 40/2019, čl. 3 i čl. 2. " +
    "Nadzor: Poverenik.",
  isLegalDuty: true,
};

/** `crate::commands::worktime::EVIDENCIJA_ZAGLAVLJE`, verbatim (§4 req. 22). */
const EVIDENCIJA_ZAGLAVLJE =
  "Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac.";

/** `crate::commands::worktime::ADVISORY_TAG` (§4 req. 4). */
const ADVISORY_TAG = "izračunato radi provere usklađenosti";

/** `crate::worktime::WEEKLY_OVERTIME_CAP_MINUTES` / `DAILY_TOTAL_CAP_MINUTES`. */
const WEEKLY_OVERTIME_CAP_MINUTES = 8 * 60;
const DAILY_TOTAL_CAP_MINUTES = 12 * 60;

function emptyMinutes(): WorkTimeMinutes {
  return {
    moguciMinuta: 0,
    ukupnoOstvareniMinuta: 0,
    efektivnoIzvrseniMinuta: 0,
    casoviCekanjaIZastojaMinuta: 0,
    obustavaRadaStrajkMinuta: 0,
    ukupnoNeizvrseniMinuta: 0,
    godisnjiOdmorMinuta: 0,
    praznikOdmorMinuta: 0,
    odsustvoUzNaknaduMinuta: 0,
    strucnoOsposobljavanjeMinuta: 0,
    sprecenostPoslodavacMinuta: 0,
    naknadaDrugiPoslodavciMinuta: 0,
    sprecenostRfzoMinuta: 0,
    porodiljskoMinuta: 0,
    neplacenoOdsustvoMinuta: 0,
    prekovremeniMinuta: 0,
    nocniMinuta: 0,
    radNaPraznikMinuta: 0,
  };
}

/**
 * `kategorija_odsustva` → its own minute bucket, DERIVED from the category name
 * exactly as `crate::commands::worktime::book_absence` derives it. A
 * hand-maintained map is the rejected design: one wrong line would post an
 * absence into the wrong statutory letter with nothing able to notice.
 */
function absenceBucket(kategorija: AbsenceCategory): keyof WorkTimeMinutes {
  const parts = kategorija.split("_");
  const camel = parts
    .map((part, index) =>
      index === 0 ? part : part.charAt(0).toUpperCase() + part.slice(1),
    )
    .join("");
  return `${camel}Minuta` as keyof WorkTimeMinutes;
}

/** `crate::commands::worktime::derive_totals` — b) and v) are sums of their indents. */
function deriveTotals(minuti: WorkTimeMinutes): void {
  minuti.ukupnoOstvareniMinuta =
    minuti.efektivnoIzvrseniMinuta +
    minuti.casoviCekanjaIZastojaMinuta +
    minuti.obustavaRadaStrajkMinuta;
  minuti.ukupnoNeizvrseniMinuta =
    minuti.godisnjiOdmorMinuta +
    minuti.praznikOdmorMinuta +
    minuti.odsustvoUzNaknaduMinuta +
    minuti.strucnoOsposobljavanjeMinuta +
    minuti.sprecenostPoslodavacMinuta +
    minuti.naknadaDrugiPoslodavciMinuta +
    minuti.sprecenostRfzoMinuta +
    minuti.porodiljskoMinuta +
    minuti.neplacenoOdsustvoMinuta;
}

function buildWorkTimeMinutes(
  request: SaveWorkTimeEntryRequest,
): WorkTimeMinutes {
  const minuti = emptyMinutes();
  minuti.moguciMinuta = request.moguciMinuta;
  minuti.efektivnoIzvrseniMinuta = request.efektivnoIzvrseniMinuta;
  minuti.casoviCekanjaIZastojaMinuta = request.casoviCekanjaIZastojaMinuta;
  minuti.prekovremeniMinuta = request.prekovremeniMinuta;
  minuti.nocniMinuta = request.nocniMinuta;
  minuti.radNaPraznikMinuta = request.radNaPraznikMinuta;

  if (request.kategorijaOdsustva) {
    minuti[absenceBucket(request.kategorijaOdsustva)] = request.odsustvoMinuta;
  }

  deriveTotals(minuti);
  return minuti;
}

/** The Monday of `dan`'s calendar week, as `YYYY-MM-DD`. */
function mondayOf(dan: string): string {
  const date = new Date(`${dan}T00:00:00Z`);

  if (Number.isNaN(date.getTime())) {
    return dan;
  }

  const weekday = (date.getUTCDay() + 6) % 7;
  date.setUTCDate(date.getUTCDate() - weekday);
  return date.toISOString().slice(0, 10);
}

/** Marks every row a later `verzija` for the same day supersedes. */
function withSupersedes(entries: WorkTimeEntryView[]): WorkTimeEntryView[] {
  return entries.map((entry) => ({
    ...entry,
    zamenjen: entries.some(
      (other) =>
        other.userId === entry.userId &&
        other.dan === entry.dan &&
        other.verzija > entry.verzija,
    ),
  }));
}

/** `catalog::is_valid_gtin` — modulo-10 over GTIN-8/12/13/14. Runs only for a
 *  code the shop asserted to be a GTIN. */
function isValidGtin(code: string): boolean {
  if (![8, 12, 13, 14].includes(code.length) || !/^\d+$/.test(code)) {
    return false;
  }

  const digits = [...code].map(Number);
  const check = digits[digits.length - 1];
  const sum = digits
    .slice(0, -1)
    .reverse()
    .reduce((total, digit, index) => total + (index % 2 === 0 ? digit * 3 : digit), 0);

  return (10 - (sum % 10)) % 10 === check;
}

/** The two ZoT čl. 34 st. 1 identity fields the Rust gate, the goods-receipt
 *  warning and the gaps report all agree on. */
function missingDeclarationFields(product: ProductSummary): string[] {
  return (
    [
      [product.manufacturerName, "manufacturerName"],
      [product.countryOfOrigin, "countryOfOrigin"],
    ] as const
  )
    .filter(([value]) => !(value ?? "").trim())
    .map(([, field]) => field);
}

/** `commands::inventory::collect_declaration_warnings`. Warns, never blocks. */
function declarationWarningsFor(product: ProductSummary): DeclarationWarning[] {
  const missingFields = missingDeclarationFields(product);
  if (missingFields.length === 0) {
    return [];
  }

  return [
    {
      productId: product.id,
      productName: product.name,
      missingFields,
      advisory: DECLARATION_ADVISORY,
      notice: declarationMissingNotice,
    },
  ];
}

/** `commands::catalog::declaration_gaps`. Active articles only; a row with no
 *  reason is not a gap and is not returned. */
function declarationGapsFor(products: ProductSummary[]): DeclarationGapRow[] {
  return products
    .filter((product) => product.active)
    .map((product) => {
      const missingFields = missingDeclarationFields(product);
      const barcodeKind = product.barcodeKind ?? null;
      const scannable = (product.barcode ?? "").trim() || null;
      const barcodeUnclassified = scannable !== null && barcodeKind === null;
      const gtinCheckDigitInvalid =
        barcodeKind === "gtin" && scannable !== null && !isValidGtin(scannable);

      const reasons: DeclarationGapReason[] = [];
      if (missingFields.length > 0) {
        reasons.push("missingIdentityData");
      }
      if (barcodeUnclassified) {
        reasons.push("barcodeUnclassified");
      }
      if (gtinCheckDigitInvalid) {
        reasons.push("gtinCheckDigitInvalid");
      }

      return {
        productId: product.id,
        sku: product.sku,
        name: product.name,
        barcode: product.barcode,
        barcodeKind,
        missingFields,
        barcodeUnclassified,
        gtinCheckDigitInvalid,
        reasons,
        // Both hang off the identity gap alone: §4 item 11 lets no figure near
        // a bare barcode defect.
        advisory: missingFields.length > 0 ? DECLARATION_ADVISORY : null,
        notice: missingFields.length > 0 ? declarationMissingNotice : null,
      } satisfies DeclarationGapRow;
    })
    .filter((row) => row.reasons.length > 0);
}

export function createMockServices(): PosServices {
  let categories: CategorySummary[] = [
    { id: 1, name: "Mlecni proizvodi", active: true },
    { id: 2, name: "Kafa", active: true },
  ];
  let taxRates: TaxRate[] = [
    { id: 1, name: "PDV 20", rateBasisPoints: 2000, active: true },
    { id: 2, name: "PDV 10", rateBasisPoints: 1000, active: true },
  ];
  const products: ProductSummary[] = [
    {
      id: 1,
      name: "Mleko 1 l",
      sku: "MLEKO-1L",
      barcode: "8600000000010",
      categoryId: 1,
      categoryName: "Mlecni proizvodi",
      unitOfMeasure: "kom",
      salePriceMinor: 15999,
      purchasePriceMinor: 12000,
      taxRateId: 1,
      taxRateName: "PDV 20",
      taxRateBasisPoints: 2000,
      minimumStockMilli: 4000,
      currentStockMilli: 3000,
      allowNegativeStock: false,
      active: true,
      perishable: false,
      perishableJustification: null,
      externalSource: null,
    },
    {
      id: 2,
      name: "Kafa 200 g",
      sku: "KAFA-200",
      barcode: "8600000000027",
      categoryId: 2,
      categoryName: "Kafa",
      unitOfMeasure: "kom",
      salePriceMinor: 50000,
      purchasePriceMinor: 32000,
      taxRateId: 1,
      taxRateName: "PDV 20",
      taxRateBasisPoints: 2000,
      minimumStockMilli: 1000,
      currentStockMilli: 3000,
      allowNegativeStock: false,
      active: true,
      perishable: false,
      perishableJustification: null,
      externalSource: null,
    },
  ];
  const ledgerMovements = new Map<number, ProductLedgerMovement[]>();
  const campaigns: CampaignView[] = [];
  const reklamacije: ReklamacijaView[] = [];
  // A single seeded receipt zaduženje (50 kom x 156,00 retail incl PDV).
  const kepEntries: KepEntryView[] = [
    {
      redniBroj: 1,
      datum: "04.07",
      opis: "Prijem robe",
      zaduzenjeMinor: 780000,
      razduzenjeMinor: null,
      kind: "receipt",
    },
  ];
  const kepClosures: KepClosureView[] = [];
  // Mock fidelity only: the real register is `work_time_entries`, append-only,
  // with the live row for a day being MAX(verzija). This double keeps the same
  // shape in memory — a correction appends, it never mutates.
  const workTimeEntries: WorkTimeEntryView[] = [];
  const workTimePeriods: { userId: number; godina: number; mesec: number; closedAt: string }[] =
    [];
  const receipt = createReceiptDetail();
  let users: UserAccount[] = [
    {
      id: 1,
      username: "admin",
      displayName: "Administrator",
      role: "admin",
      active: true,
      createdAt: now,
      updatedAt: now,
      lastLoginAt: now,
    },
    {
      id: 2,
      username: "marko",
      displayName: "Marko Markovic",
      role: "cashier",
      active: true,
      createdAt: now,
      updatedAt: now,
      lastLoginAt: null,
    },
  ];
  // Every čl. 87–91 input starts empty: an employee nobody has profiled yet is
  // not an employee with no protections, and the guards read it that way.
  const employeeProfiles = new Map<number, EmployeeProfile>();
  // The ZZPL trio. The double keeps the shapes and the ordering honest; the
  // hash chain itself is the backend's business, so the fixture rows carry
  // plausible digests rather than computed ones and the verdict is stated, not
  // derived — exactly as the real panel receives it.
  let supportSessions: SupportSession[] = [];
  let auditEvents: AuditEvent[] = [
    {
      id: 1,
      at: "2026-06-18T09:30:00Z",
      actorUserId: 1,
      actorName: "Administrator",
      action: "uvid",
      actionLabel: "Uvid",
      objectType: "personnel_record",
      objectTypeLabel: "Evidencija o zaposlenom",
      objectId: "2",
      reasonCode: "interni_nadzor",
      reasonLabel: "Interni nadzor",
      recipient: null,
      recipientLabel: null,
      supportSessionId: null,
      prevHash: "0".repeat(64),
      hash: "1".repeat(64),
    },
  ];
  let breaches: Breach[] = [];
  let processingActivities: ProcessingActivity[] = [
    {
      id: 1,
      kljuc: "evidencija_zaposlenih",
      rukovalacNaziv: "Vantum Market",
      rukovalacKontakt: "Bulevar 1, Beograd",
      svrhaObrade:
        "Vođenje evidencije o zaposlenim licima i ispunjenje obaveza iz radnog zakonodavstva.",
      vrstaLica: "Zaposleni i radno angažovana lica kod rukovaoca.",
      vrstaPodataka: "Identitet i matični broj, radno mesto, radno vreme.",
      vrstaPrimalaca: "Nadležni državni organi kada zakon to nalaže; knjigovođa.",
      prenosUDrugeDrzave:
        "Nije utvrđen prenos u druge države ni u međunarodne organizacije.",
      mereZastitePrenosa: null,
      rokCuvanja: "Podaci se čuvaju trajno. Rok se ne podešava.",
      retentionRecordClass: "personnel",
      opisMeraZastite:
        "Evidencija je u zasebnoj tabeli, odvojenoj od naloga za prijavu.",
      updatedAt: now,
    },
  ];
  /**
   * The shared retention table as `crate::retention::seed_retention_policies`
   * writes it on the day this mock's clock stands at: the trajno classes with no
   * end date, the bounded ones with the seeding day plus their documented
   * default (three years for the standalone overtime register, two for the
   * evidencija pristupa, the seeding day itself where the discarding event is
   * not a calendar one).
   *
   * `napomena` is abridged here — the shipped strings live in `retention.rs` and
   * are guarded there; a second copy of a legal sentence is a second thing to
   * get wrong.
   */
  const retentionPolicies: RetentionPolicy[] = [
    {
      recordClass: "worktime_classification",
      naziv: "Izvedena mesečna klasifikacija časova",
      retainUntil: null,
      legalHold: false,
      neverPurge: true,
      adjustable: false,
      napomena:
        "Izvedena mesečna klasifikacija časova. Čuva se trajno (ZEOR čl. 7 st. 2 i čl. 25 st. 3).",
      updatedAt: now,
    },
    {
      recordClass: "worktime_overtime_log",
      naziv: "Samostalna evidencija prekovremenog rada",
      retainUntil: "2029-06-18",
      legalHold: false,
      neverPurge: false,
      adjustable: true,
      napomena:
        "Samostalna evidencija prekovremenog rada (ZoR čl. 55 st. 6). Zakon ne propisuje rok; " +
        "primenjuje se odbrambeni minimum od tri godine, koji se pomera samo unapred.",
      updatedAt: now,
    },
    {
      recordClass: "worktime_draft",
      naziv: "Radne verzije unosa i pomoćni podaci o vremenu",
      retainUntil: "2026-06-18",
      legalHold: false,
      neverPurge: false,
      adjustable: true,
      napomena:
        "Radne verzije unosa i pomoćni podaci o vremenu. Brišu se tek pošto je period zatvoren " +
        "i klasifikacija izvedena (ZZPL čl. 5 st. 1 tač. 5).",
      updatedAt: now,
    },
    {
      recordClass: "personnel",
      naziv: "Evidencija o zaposlenim licima",
      retainUntil: null,
      legalHold: false,
      neverPurge: true,
      adjustable: false,
      napomena:
        "Evidencija o zaposlenim licima (ZEOR čl. 5). Čuva se trajno (ZEOR čl. 7 st. 2) i rok " +
        "se ne podešava.",
      updatedAt: now,
    },
    {
      recordClass: "credentials",
      naziv: "PIN i lozinka (heš vrednosti)",
      retainUntil: "2026-06-18",
      legalHold: false,
      neverPurge: false,
      adjustable: true,
      napomena:
        "PIN i lozinka (samo heš vrednosti). Uklanjaju se danom prestanka radnog odnosa, a ne " +
        "po isteku roka (ZZPL čl. 5 st. 1 tač. 5 i čl. 42 st. 2).",
      updatedAt: now,
    },
    {
      recordClass: "access_log",
      naziv: "Evidencija pristupa podacima o ličnosti",
      retainUntil: "2028-06-18",
      legalHold: false,
      neverPurge: false,
      adjustable: true,
      napomena:
        "Evidencija pristupa podacima o ličnosti, koju rukovalac vodi kao sopstvenu meru. " +
        "Zakon ne propisuje rok; primenjuje se podrazumevani rok od dve godine. Rok se pomera " +
        "samo unapred. Ova evidencija se ne čuva trajno.",
      updatedAt: now,
    },
    {
      recordClass: "processing_register",
      naziv: "Evidencija o radnjama obrade",
      retainUntil: null,
      legalHold: false,
      neverPurge: true,
      adjustable: false,
      napomena:
        "Evidencija o radnjama obrade (ZZPL čl. 47 st. 1). Čuva se trajno (čl. 47 st. 7) i rok " +
        "se ne podešava.",
      updatedAt: now,
    },
    {
      recordClass: "cenovnik_archive",
      naziv: "Arhiva objavljenih cenovnika",
      retainUntil: "2028-06-18",
      legalHold: false,
      neverPurge: false,
      adjustable: true,
      napomena:
        "Arhiva objavljenih cenovnika (ZZP čl. 6 st. 5). Podrazumevani rok je dve godine — " +
        "zastarelost prekršajnog gonjenja iz ZZP čl. 213; rok se pomera samo unapred. " +
        "Automatsko čišćenje nikada ne uklanja važeći cenovnik prodajnog objekta.",
      updatedAt: now,
    },
    {
      recordClass: "popis_dokumentacija",
      naziv: "Popisne liste i dokumentacija o popisu",
      // The one class on the business-year clock (ZoRač čl. 28 st. 9), so the
      // seeded floor is 31 December of the mock's year plus five — not the
      // seeding day's anniversary the way every other bounded row above is.
      retainUntil: "2031-12-31",
      legalHold: false,
      neverPurge: false,
      adjustable: true,
      // Abridged from `RecordClass::PopisDokumentacija::napomena`, but never in
      // the direction of claiming more than the crate does: the withdrawn
      // „štampa se na zahtev“ clause about the izveštaj lived on here through
      // the sixth-incident fix, because the guard beside it reads the Rust
      // string and nothing read this one.
      napomena:
        "Popisne liste, sastav komisije i potpisi. Rok čuvanja je pet godina, računato od " +
        "poslednjeg dana poslovne godine na koju se popis odnosi (ZoRač čl. 28 st. 7 i st. 9); " +
        "rok se pomera samo unapred. Proknjižen popis se ne menja — ispravka ide kroz novi popis " +
        "(PoP čl. 14 st. 3; ZoRač čl. 8 st. 4). Popisne liste, odluku o popisu i plan rada " +
        "program sastavlja i izvozi u datoteku za štampu — potpisan primerak sastavlja i čuva " +
        "sam obveznik, a izvezena datoteka nije potpisana isprava. Izveštaj o popisu se ne čuva " +
        "u aplikaciji: sastavlja se na zahtev i prikazuje na ekranu, a program ga ne štampa i ne " +
        "izvozi. Automatsko brisanje popisne dokumentacije ne postoji.",
      updatedAt: now,
    },
  ];
  let currentShift: ShiftSummary | null = {
    id: 1,
    userId: 1,
    cashierName: "Administrator",
    openedAt: now,
    closedAt: null,
    openingCashMinor: 500000,
    expectedCashMinor: 500000,
    countedCashMinor: null,
    cashSalesMinor: 0,
    cardSalesMinor: 0,
    paidInMinor: 0,
    paidOutMinor: 0,
    differenceMinor: null,
    status: "open",
    openingNote: "Jutarnja smena",
    closingNote: null,
  };
  let session: AuthSession | null = {
    user: users[0],
    currentShift,
  };
  let importJobs: ImportJob[] = [
    {
      id: 1,
      importType: "products" as const,
      fileName: "prethodni-products.csv",
      status: "completed" as const,
      totalRows: 2,
      errorRows: 0,
      createdAt: now,
      completedAt: now,
    },
  ];
  let companySettings: CompanySettings = {
    shopName: "Vantum Market",
    address: "Bulevar 1, Beograd",
    pib: "",
    registrationNumber: "87654321",
    phone: "+381 11 123 456",
    logoPath: null,
    currency: "RSD",
  };
  /**
   * Where the shop has said its cenovnik goes. A fresh install has said nothing,
   * and that is an honest state — never „the shop is in breach“ (§2b).
   */
  let cenovnikTarget: CenovnikPublishTarget = { kind: "notConfigured" };
  /**
   * The outlet's archive (ZZP čl. 6 st. 5), newest LAST in this array and
   * reversed on read, exactly as `cenovnik_list_snapshots` orders it. Two
   * publications are seeded so the archive surface has a prior file to compare
   * the current one against, which is the whole point of st. 5.
   *
   * Bodies are the real rendered shape — BOM, `;`, CRLF, prices with a dot —
   * because a double that published a prettier file than the backend would let
   * a reader-side defect through.
   */
  const cenovnikArchive: { snapshot: CenovnikSnapshot; body: string }[] = [
    mockSnapshot(1, "2026-06-17T08:30:00Z", 15499),
    mockSnapshot(2, "2026-06-18T09:45:00Z", 15999),
  ];
  let receiptSettings: ReceiptSettings = {
    prefix: "VP-",
    nextSequenceNumber: 1,
    resetPolicy: "none",
  };
  let salesSettings = { allowOverselling: false };
  // Every answer starts UNSET. A fresh install must ask, never assume — see
  // SW11-SW15-VERIFIED-RULES §3 req 31 / §5 Q-8.
  let shopProfile: ShopProfile = {
    pravnaForma: null,
    pdvObveznik: null,
    distanceSelling: null,
    lpfrInPremises: null,
    lpfrCarveOutInternetOnly: null,
    lpfrCarveOutOwnUsedAssets: null,
    esirElements: [],
  };
  /**
   * The cached EUR middle rate — mutable, so a test can drive the whole
   * round trip (absent -> manual -> fresh) instead of reading one frozen
   * constant. `null` is the real shape of a fresh install: no rate has ever
   * been fetched, so the AML check cannot run at all.
   */
  let eurRate: EurRate | null = { ...DEFAULT_MOCK_EUR_RATE };
  // Saturday counts by default: „radni dan" is statutorily undefined and
  // counting Saturdays yields the earlier, conservative deadline.
  let cashDepositCalendar: CashDepositCalendar = {
    saturdayIsWorking: true,
    days: [
      { day: "2026-01-01", label: "Nova godina" },
      { day: "2026-01-02", label: "Nova godina" },
      { day: "2026-01-07", label: "Božić" },
      { day: "2026-02-15", label: "Dan državnosti Srbije" },
      { day: "2026-02-16", label: "Dan državnosti Srbije" },
      { day: "2026-05-01", label: "Praznik rada" },
      { day: "2026-05-02", label: "Praznik rada" },
      { day: "2026-11-11", label: "Dan primirja u Prvom svetskom ratu" },
    ],
    horizonYear: 2027,
  };
  /** Pologe recorded through `shiftCashMovement`, in para. */
  let mockDepositedMinor = 0;
  let backupSettings: BackupSettings = {
    backupFolder: "mock://backups",
    automaticBackupEnabled: true,
  };
  let backupJobs: BackupJob[] = [];
  const popisSessions: MockPopis[] = [];
  let popisPodesavanja: PopisPodesavanja = { rokPredajeFi: null };

  /**
   * Mirrors `commands::settings::eur_rate_status`. An absent rate is stale —
   * never "fine" — because the AML threshold is derived from it, so silence
   * has to read as „nepoznato".
   */
  function eurRateStatus(): EurRateStatus {
    return {
      rate: eurRate === null ? null : { ...eurRate },
      isStale: eurRate === null || eurRate.rateDate !== MOCK_TODAY,
      checkedFor: MOCK_TODAY,
    };
  }

  /** Mirrors `commands::auth::require_admin` — the only admin gate. */
  function requireAdmin() {
    if (session?.user.role !== "admin") {
      throw {
        code: "forbidden",
        message: "Samo administrator može da izvrši ovu akciju.",
      };
    }
  }

  function findProduct(productId: number) {
    const product = products.find((item) => item.id === productId);

    if (!product) {
      throw { code: "not_found", message: "Artikal nije pronađen." };
    }

    return product;
  }

  function applyStock(
    request: InventoryAdjustmentRequest,
    movementType: "receive" | "correction" | "write_off",
  ) {
    const product = findProduct(request.productId);
    const previousQuantityMilli = product.currentStockMilli;
    const delta =
      movementType === "write_off" ? -request.quantityMilli : request.quantityMilli;
    const newQuantityMilli = previousQuantityMilli + delta;

    if (newQuantityMilli < 0 && !product.allowNegativeStock) {
      throw { code: "insufficient_stock", message: "Nema dovoljno zaliha." };
    }

    product.currentStockMilli = newQuantityMilli;
    const movement = {
      id: Date.now(),
      movementType,
      quantityMilli: delta,
      resultingQuantityMilli: newQuantityMilli,
      reason: request.reason ?? null,
      referenceType: request.referenceType ?? null,
      referenceId: request.referenceId ?? null,
      createdAt: now,
    };
    ledgerMovements.set(product.id, [
      movement,
      ...(ledgerMovements.get(product.id) ?? []),
    ]);

    return {
      productId: product.id,
      movementId: movement.id,
      movementType,
      quantityMilli: request.quantityMilli,
      previousQuantityMilli,
      newQuantityMilli,
      createdAt: now,
      // Nothing new arrives through a correction or a write-off, so only the
      // receipt can raise the čl. 34 warning.
      declarationWarnings:
        movementType === "receive" ? declarationWarningsFor(product) : [],
    };
  }

  const services: PosServices & { initialSession: AuthSession | null } = {
    initialSession: session,
    settings: {
      async getHealth() {
        return {
          backend: "local",
          appVersion: "test",
          databasePath: "mock://vantumpos.sqlite3",
          migrated: true,
        };
      },
      async getCompanySettings() {
        return companySettings;
      },
      async updateCompanySettings(request) {
        if (request.pib && !/^\d{9}$/.test(request.pib)) {
          throw { code: "validation_error", message: "PIB mora imati 9 cifara." };
        }
        if (request.currency !== "RSD") {
          throw {
            code: "validation_error",
            message: "Valuta za lokalni MVP mora biti RSD.",
          };
        }

        companySettings = { ...request };
        return companySettings;
      },
      async listTaxRates() {
        return taxRates;
      },
      async seedTaxRates(inVatSystem: boolean) {
        if (taxRates.length === 0) {
          taxRates = inVatSystem
            ? [
                { id: 1, name: "PDV 20%", rateBasisPoints: 2000, active: true },
                { id: 2, name: "PDV 10%", rateBasisPoints: 1000, active: true },
              ]
            : [{ id: 1, name: "Bez PDV-a", rateBasisPoints: 0, active: true }];
        }
        return taxRates;
      },
      async saveTaxRate(request) {
        if (!request.name.trim()) {
          throw { code: "validation_error", message: "Naziv PDV stope je obavezan." };
        }

        const saved = {
          id: request.id ?? Math.max(0, ...taxRates.map((item) => item.id)) + 1,
          name: request.name,
          rateBasisPoints: request.rateBasisPoints,
          active: request.active,
        };
        taxRates = request.id
          ? taxRates.map((item) => (item.id === request.id ? saved : item))
          : [...taxRates, saved];

        return saved;
      },
      async getReceiptSettings() {
        return receiptSettings;
      },
      async updateReceiptSettings(request) {
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }

        receiptSettings = { ...request, resetPolicy: "none" };
        return receiptSettings;
      },
      async getSalesSettings() {
        return salesSettings;
      },
      async updateSalesSettings(request) {
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }

        salesSettings = { ...request };
        return salesSettings;
      },
      async getShopProfile() {
        return shopProfile;
      },
      async getLpfrNotice() {
        return lpfrRequiredNotice;
      },
      async updateShopProfile(request) {
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }

        for (const element of request.esirElements) {
          if (!element.naziv.trim() || !element.verzija.trim() || !element.ib.trim()) {
            throw {
              code: "validation_error",
              message: "Naziv, verzija i IB elementa su obavezni.",
            };
          }
        }

        shopProfile = {
          ...request,
          esirElements: request.esirElements.map((element) => ({
            ...element,
            naziv: element.naziv.trim(),
            verzija: element.verzija.trim(),
            ib: element.ib.trim(),
          })),
        };
        return shopProfile;
      },
      async getEurRate() {
        return eurRateStatus();
      },
      async refreshEurRate() {
        requireAdmin();
        // A reachable NBS. An unreachable one is modelled by overriding this
        // method in the test — it must resolve with the cached rate, never
        // reject, because a dead network may not block the till.
        eurRate = { ...DEFAULT_MOCK_EUR_RATE };
        return eurRateStatus();
      },
      async setManualEurRate(rateMinor, rateDate) {
        requireAdmin();
        // `settings_set_manual_eur_rate` trims before validating.
        const trimmedDate = rateDate.trim();

        if (!Number.isInteger(rateMinor) || rateMinor <= 0) {
          throw {
            code: "validation_error",
            message: "Kurs mora biti veći od nule.",
          };
        }
        if (rateMinor < MANUAL_RATE_MIN_PARA || rateMinor > MANUAL_RATE_MAX_PARA) {
          throw {
            code: "validation_error",
            message: "Kurs mora biti između 50 i 500 dinara za 1 evro.",
          };
        }
        if (!isIsoDate(trimmedDate)) {
          throw {
            code: "validation_error",
            message: "Datum kursa mora biti u obliku GGGG-MM-DD.",
          };
        }

        eurRate = { rateMinor, rateDate: trimmedDate, source: "manual" };
        return eurRateStatus();
      },
      async getCashDepositCalendar() {
        requireAdmin();
        return cashDepositCalendar;
      },
      async setSaturdayIsWorking(counts) {
        requireAdmin();
        cashDepositCalendar = {
          ...cashDepositCalendar,
          saturdayIsWorking: counts,
        };
        return cashDepositCalendar;
      },
      async saveNonWorkingDay(day, label) {
        requireAdmin();

        if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) {
          throw {
            code: "validation_error",
            message: "Datum neradnog dana mora biti u obliku gggg-MM-dd.",
          };
        }
        if (!label.trim()) {
          throw {
            code: "validation_error",
            message: "Unesite naziv neradnog dana.",
          };
        }

        const days = cashDepositCalendar.days
          .filter((entry) => entry.day !== day)
          .concat({ day, label: label.trim() })
          .sort((left, right) => left.day.localeCompare(right.day));
        cashDepositCalendar = { ...cashDepositCalendar, days };
        return cashDepositCalendar;
      },
      async deleteNonWorkingDay(day) {
        requireAdmin();
        cashDepositCalendar = {
          ...cashDepositCalendar,
          days: cashDepositCalendar.days.filter((entry) => entry.day !== day),
        };
        return cashDepositCalendar;
      },
    },
    backup: {
      async getBackupStatus() {
        const lastSuccessfulBackup =
          backupJobs.find(
            (job) =>
              job.status === "completed" &&
              (job.backupType === "manual" || job.backupType === "automatic"),
          ) ?? null;
        const lastFailedBackup =
          backupJobs.find((job) => job.status === "failed") ?? null;

        return {
          ...backupSettings,
          stale: !lastSuccessfulBackup,
          encryptionConfigured: false,
          lastSuccessfulBackup,
          lastFailedBackup,
        };
      },
      async updateBackupSettings(request) {
        backupSettings = request;
        return backupSettings;
      },
      async createBackup(request: CreateBackupRequest) {
        if (request.backupFolder) {
          backupSettings = {
            ...backupSettings,
            backupFolder: request.backupFolder,
          };
        }

        const job = createBackupJob(
          "manual",
          `${backupSettings.backupFolder}/vantumpos-manual-${backupJobs.length + 1}.sqlite3`,
        );
        backupJobs = [job, ...backupJobs];
        return job;
      },
      async restoreBackup(request: RestoreBackupRequest) {
        if (request.confirmationText !== "VRATI PODATKE") {
          throw {
            code: "validation_error",
            message: "Potvrdite restore unosom teksta VRATI PODATKE.",
          };
        }

        const job = createBackupJob("restore", request.path);
        backupJobs = [job, ...backupJobs];
        return job;
      },
      async listBackupJobs() {
        return backupJobs;
      },
      async setBackupPassphrase() {},
      async resetTradingData(confirmationText: string) {
        if (confirmationText !== "OBRISI PODATKE") {
          throw {
            code: "validation_error",
            message: "Potvrdite brisanje unosom teksta OBRISI PODATKE.",
          };
        }
      },
    },
    auth: {
      async getSession() {
        return session;
      },
      async login(request) {
        const user = users.find(
          (item) => item.username === request.username && item.active,
        );

        if (!user || request.credential !== "1234") {
          throw {
            code: "invalid_credentials",
            message: "Korisničko ime ili lozinka nisu ispravni.",
          };
        }

        session = {
          user: { ...user, lastLoginAt: now },
          currentShift: null,
        };

        return session;
      },
      async logout() {
        session = null;
      },
    },
    users: {
      async listUsers() {
        return users;
      },
      async createUser(request) {
        const user = {
          id: Math.max(0, ...users.map((item) => item.id)) + 1,
          username: request.username,
          displayName: request.displayName,
          role: request.role,
          active: request.active,
          createdAt: now,
          updatedAt: now,
          lastLoginAt: null,
        };
        users = [...users, user];
        if (request.profile) {
          employeeProfiles.set(user.id, request.profile);
        }
        return user;
      },
      async updateUser(id, request) {
        const current = users.find((user) => user.id === id);

        if (!current) {
          throw { code: "not_found", message: "Korisnik nije pronađen." };
        }

        const updated = {
          ...current,
          username: request.username,
          displayName: request.displayName,
          role: request.role,
          active: request.active,
          updatedAt: now,
        };
        users = users.map((user) => (user.id === id ? updated : user));
        // A save that carries no profile leaves the čl. 87–91 inputs alone —
        // the same rule the command enforces.
        if (request.profile) {
          employeeProfiles.set(id, request.profile);
        }
        return updated;
      },
      async deactivateUser(id) {
        users = users.map((user) =>
          user.id === id ? { ...user, active: false, updatedAt: now } : user,
        );
      },
      async getEmployeeProfile(id) {
        if (!users.some((user) => user.id === id)) {
          throw { code: "not_found", message: "Korisnik nije pronađen." };
        }

        return employeeProfiles.get(id) ?? { ...prazanProfilZaposlenog };
      },
    },
    shifts: {
      async getCurrentShift() {
        return session?.currentShift ?? null;
      },
      async openShift(request) {
        if (!session) {
          throw { code: "unauthorized", message: "Prijavite se za rad." };
        }

        const openedShift: ShiftSummary = {
          id: 2,
          userId: session.user.id,
          cashierName: session.user.displayName,
          openedAt: now,
          closedAt: null,
          openingCashMinor: request.openingCashMinor,
          expectedCashMinor: request.openingCashMinor,
          countedCashMinor: null,
          cashSalesMinor: 0,
          cardSalesMinor: 0,
          paidInMinor: 0,
          paidOutMinor: 0,
          differenceMinor: null,
          status: "open",
          openingNote: request.note ?? null,
          closingNote: null,
        };
        currentShift = openedShift;
        session = { ...session, currentShift };
        return openedShift;
      },
      async closeShift(request) {
        if (!session?.currentShift || session.currentShift.id !== request.shiftId) {
          throw { code: "not_found", message: "Smena nije pronađena." };
        }

        const closed = {
          ...session.currentShift,
          closedAt: now,
          countedCashMinor: request.countedCashMinor,
          differenceMinor: request.countedCashMinor - session.currentShift.expectedCashMinor,
          status: "closed" as const,
          closingNote: request.note ?? null,
        };
        currentShift = null;
        session = { ...session, currentShift: null };
        return closed;
      },
      async adminCloseShift(request) {
        if (session?.currentShift && session.currentShift.id === request.shiftId) {
          const closed = {
            ...session.currentShift,
            closedAt: now,
            countedCashMinor: request.countedCashMinor,
            differenceMinor:
              request.countedCashMinor - session.currentShift.expectedCashMinor,
            status: "closed" as const,
            closingNote: request.note ?? null,
          };
          currentShift = null;
          session = { ...session, currentShift: null };
          return closed;
        }

        return {
          id: request.shiftId,
          userId: 2,
          cashierName: "Marko Markovic",
          openedAt: now,
          closedAt: now,
          openingCashMinor: 0,
          expectedCashMinor: request.countedCashMinor,
          countedCashMinor: request.countedCashMinor,
          cashSalesMinor: 0,
          cardSalesMinor: 0,
          paidInMinor: 0,
          paidOutMinor: 0,
          differenceMinor: 0,
          status: "closed" as const,
          openingNote: null,
          closingNote: request.note ?? null,
        };
      },
      async shiftCashMovement(request) {
        if (!session?.currentShift) {
          throw { code: "shift_required", message: "Smena nije otvorena." };
        }

        if (request.amountMinor <= 0) {
          throw {
            code: "validation_error",
            message: "Iznos mora biti veći od nule.",
          };
        }

        // A podizanje sa računa fills the drawer like a pay_in; a polog empties
        // it like a pay_out — the same mapping `commands::shifts` uses.
        const fillsDrawer =
          request.direction === "pay_in" ||
          request.direction === "bank_withdrawal";
        const delta = fillsDrawer ? request.amountMinor : -request.amountMinor;
        const updated: ShiftSummary = {
          ...session.currentShift,
          expectedCashMinor: session.currentShift.expectedCashMinor + delta,
          paidInMinor:
            session.currentShift.paidInMinor +
            (fillsDrawer ? request.amountMinor : 0),
          paidOutMinor:
            session.currentShift.paidOutMinor +
            (fillsDrawer ? 0 : request.amountMinor),
        };
        if (request.direction === "bank_deposit") {
          mockDepositedMinor += request.amountMinor;
        }
        currentShift = updated;
        session = { ...session, currentShift: updated };
        return updated;
      },
    },
    catalog: {
      async listProducts(query) {
        const items = filterProducts(products, query);
        return {
          items,
          categories,
          taxRates,
          total: items.length,
        };
      },
      async searchProducts(query) {
        return {
          items: filterProducts(products, {
            search: query.search,
            active: query.active ?? true,
          }).slice(0, query.limit ?? 20),
          categories,
          taxRates,
          total: products.length,
        };
      },
      async getProduct(id) {
        return products.find((item) => item.id === id) ?? null;
      },
      async createProduct(request) {
        rejectDuplicateProduct(products, request.sku, request.barcode);
        const taxRate = findTaxRate(taxRates, request.taxRateId);
        const category = request.categoryId
          ? categories.find((item) => item.id === request.categoryId)
          : null;
        const product = {
          id: products.length + 1,
          ...request,
          categoryName: category?.name ?? null,
          taxRateName: taxRate.name,
          taxRateBasisPoints: taxRate.rateBasisPoints,
          currentStockMilli: 0,
          externalSource: request.externalSource ?? null,
        };
        products.push(product);
        republishCenovnik();
        return product;
      },
      async updateProduct(id, request) {
        const index = products.findIndex((item) => item.id === id);
        rejectDuplicateProduct(products, request.sku, request.barcode, id);
        const taxRate = findTaxRate(taxRates, request.taxRateId);
        const category = request.categoryId
          ? categories.find((item) => item.id === request.categoryId)
          : null;
        const product = {
          id,
          ...request,
          categoryName: category?.name ?? null,
          taxRateName: taxRate.name,
          taxRateBasisPoints: taxRate.rateBasisPoints,
          currentStockMilli: products[index]?.currentStockMilli ?? 0,
          externalSource: request.externalSource ?? null,
        };
        const before = products[index];
        products[index] = product;
        if (movesAPublishedPrice(before, product)) {
          republishCenovnik();
        }
        return product;
      },
      async setProductActive(id, active) {
        const product = findProduct(id);
        const changed = product.active !== active;
        product.active = active;
        // An article going off the shelf, or coming back, changes what the shop
        // offers — the same reading `record_offered_price_change` takes.
        if (changed) {
          republishCenovnik();
        }
        return product;
      },
      async lookupProductByBarcode(barcode) {
        return mockBarcodeLookup(barcode);
      },
      async listCategories() {
        return categories;
      },
      async saveCategory(request) {
        const saved = {
          id: request.id ?? Math.max(0, ...categories.map((item) => item.id)) + 1,
          name: request.name,
          active: request.active ?? true,
        };

        categories = request.id
          ? categories.map((category) =>
              category.id === request.id ? saved : category,
            )
          : [...categories, saved];

        return saved;
      },
      async getPrethodnaCena() {
        return {
          status: "computed",
          priceMinor: 499000,
          windowDays: 30,
          windowFrom: "2026-06-20T00:00:00Z",
          windowTo: "2026-07-20T00:00:00Z",
          truncated: false,
          reason: null,
          ageDays: null,
        } satisfies PrethodnaCenaDto;
      },
      async declarationGaps() {
        // `catalog_declaration_gaps` is `require_admin` — the double models the
        // gate so the report's permission branch is testable.
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }

        return declarationGapsFor(products);
      },
    },
    sales: {
      async createSalePreview(request) {
        return createSalePreview(products, request);
      },
      async completeSale(request) {
        const preview = createSalePreview(products, request);
        const cashReceivedMinor =
          request.payments.find((payment) => payment.method === "cash")
            ?.amountMinor ?? 0;

        return {
          id: 1,
          localReceiptNumber: "VP-000001",
          createdAt: now,
          cashierName: "Administrator",
          fiscalStatus: "not_fiscalized",
          ...preview,
          payments: request.payments.map((payment) =>
            payment.method === "cash"
              ? { ...payment, amountMinor: preview.totalMinor }
              : payment,
          ),
          cashReceivedMinor,
          changeDueMinor: Math.max(cashReceivedMinor - preview.totalMinor, 0),
        };
      },
      async assessCashPayment(cashMinor) {
        return assessCashPayment(cashMinor, eurRate, shopProfile);
      },
      /**
       * Mirrors `commands::sales::assess_price_integrity`: the comparison is
       * against the **unit price the article is offered at**, never the line
       * total after a discount — a discount is a reduction granted on the
       * prodajna cena, not a different price for the article, and a discount
       * that cancelled the warning would be the obvious way to ring above the
       * published cenovnik unremarked. Only above matters; one entry per
       * article; no published file means no guard and never a blocked sale.
       */
      async assessPriceIntegrity(request) {
        const published = currentCenovnik();
        if (!published) {
          return [];
        }

        const prices = publishedPrices(published.body);
        const divergences: PriceDivergence[] = [];
        for (const item of request.items) {
          const product = products.find(
            (candidate) => candidate.id === item.productId,
          );
          if (!product) {
            continue;
          }
          const publishedUnitPriceMinor = prices.get(product.sku);
          if (
            publishedUnitPriceMinor === undefined ||
            product.salePriceMinor <= publishedUnitPriceMinor ||
            divergences.some((row) => row.productId === product.id)
          ) {
            continue;
          }

          divergences.push({
            productId: product.id,
            productName: product.name,
            productSku: product.sku,
            chargedUnitPriceMinor: product.salePriceMinor,
            publishedUnitPriceMinor,
            snapshotId: published.snapshot.id,
            snapshotGeneratedAt: published.snapshot.generatedAt,
            snapshotContentHash: published.snapshot.contentHash,
          });
        }

        return divergences;
      },
    },
    inventory: {
      async listStock(query) {
        const items = products.map(toStockItem);
        const filtered = items.filter((item) => {
          const matchesSearch =
            !query.search ||
            [item.productName, item.sku, item.barcode ?? ""].some((value) =>
              value.toLowerCase().includes(query.search!.toLowerCase()),
            );
          const matchesCategory =
            query.categoryId == null || item.categoryId === query.categoryId;
          const matchesState =
            query.stockState === "low"
              ? item.lowStock
              : query.stockState === "zero"
                ? item.currentQuantityMilli === 0
                : query.stockState === "negative"
                  ? item.currentQuantityMilli < 0
                  : true;

          return matchesSearch && matchesCategory && matchesState;
        });

        return { items: filtered };
      },
      async receiveStock(request) {
        return applyStock(request, "receive");
      },
      async correctStock(request) {
        return applyStock(request, "correction");
      },
      async writeOffStock(request) {
        return applyStock(request, "write_off");
      },
      async getProductLedger(productId) {
        const product = findProduct(productId);
        return {
          productId,
          product: toStockItem(product),
          movements: ledgerMovements.get(productId) ?? [],
        };
      },
      // No role gate: `inventory_mark_declaration_checked` is session-gated, not
      // admin-gated — the čl. 69a tač. 4 record belongs to whoever stands at the
      // pallet, and `inventory_receive` admits the cashier too.
      async markDeclarationChecked(productId) {
        findProduct(productId);
      },
    },
    receipts: {
      async searchReceipts(query) {
        const matchesProduct =
          !query.product ||
          receipt.items.some((item) =>
            item.productName.toLowerCase().includes(query.product!.toLowerCase()),
          );

        return {
          receipts: matchesProduct ? [receipt] : [],
          total: matchesProduct ? 1 : 0,
        };
      },
      async getReceipt() {
        return receipt;
      },
      async voidReceipt() {
        receipt.status = "voided";
        receipt.canVoid = false;
        receipt.canReturn = false;
        receipt.linkedDocuments = [
          {
            id: 2,
            documentType: "void",
            receiptNumber: "STO-R-2026-0001-1",
            status: "voided",
            createdAt: now,
            totalMinor: receipt.totalMinor,
          },
        ];
        receipt.linkedDocumentCount = receipt.linkedDocuments.length;
        return receipt;
      },
      async returnItems(request) {
        receipt.status = "refunded";
        for (const returned of request.items) {
          const item = receipt.items.find(
            (receiptItem) => receiptItem.id === returned.saleItemId,
          );
          if (item) {
            item.returnedQuantityMilli = returned.quantityMilli;
          }
        }
        receipt.linkedDocuments = [
          {
            id: 3,
            documentType: "return",
            receiptNumber: "POV-R-2026-0001-1",
            status: "refunded",
            createdAt: now,
            totalMinor: 50000,
          },
        ];
        receipt.linkedDocumentCount = receipt.linkedDocuments.length;
        receipt.canVoid = false;
        return receipt;
      },
      async setEsirNumber(_receiptId, esirReceiptNumber) {
        const trimmed = esirReceiptNumber.trim();
        receipt.esirReceiptNumber = trimmed.length > 0 ? trimmed : null;
        return receipt;
      },
    },
    imports: {
      async readImportHeaders(request) {
        const parsed = parseMockCsv(request.csvText);

        return {
          importType: request.importType,
          fileName: request.fileName,
          delimiter: parsed.delimiter,
          headers: parsed.headers,
          totalRows: parsed.rows.length,
        };
      },
      async validateImport(request) {
        const parsed = parseMockCsv(request.csvText);
        const priceHeader = request.mapping.sale_price;
        const priceIndex = parsed.headers.indexOf(priceHeader);
        const rows = parsed.rows
          .map((row, index) => {
            const price = priceIndex >= 0 ? row[priceIndex] : "";

            if (/neispravno|nije/i.test(price)) {
              return {
                rowNumber: index + 2,
                status: "error" as const,
                action: "skip" as const,
                message: "Cena nije ispravna.",
                values: rowToValues(parsed.headers, row),
              };
            }

            return {
              rowNumber: index + 2,
              status: "valid" as const,
              action: "create" as const,
              message: "",
              values: rowToValues(parsed.headers, row),
            };
          });
        const errorCount = rows.filter((row) => row.status === "error").length;

        return {
          importType: request.importType,
          fileName: request.fileName,
          totalRows: parsed.rows.length,
          validCount: rows.length - errorCount,
          warningCount: 0,
          errorCount,
          summary: { create: rows.length - errorCount, update: 0, skip: errorCount },
          rows,
        };
      },
      async commitImport(request) {
        const parsed = parseMockCsv(request.csvText);
        const job = {
          id: Math.max(0, ...importJobs.map((item) => item.id)) + 1,
          importType: request.importType,
          fileName: request.fileName,
          status: "completed" as const,
          totalRows: parsed.rows.length,
          errorRows: 0,
          createdAt: now,
          completedAt: now,
        };
        importJobs = [job, ...importJobs];

        return job;
      },
      async listImportJobs() {
        return importJobs;
      },
      async getImportJob(id) {
        const job = importJobs.find((item) => item.id === id);

        if (!job) {
          return null;
        }

        return {
          ...job,
          rows: [
            {
              rowNumber: 2,
              status: "imported" as const,
              message: null,
              values: { Naziv: "Hleb", Cena: "120,00" },
            },
          ],
        };
      },
    },
    reports: {
      async getDailyTurnover() {
        return {
          summary: {
            totalMinor: 12000,
            cashMinor: 8000,
            cardMinor: 4000,
            bankTransferMinor: 0,
            receiptCount: 2,
            averageReceiptMinor: 6000,
          },
          rows: [
            {
              day: "2026-06-17",
              receiptCount: 2,
              cashMinor: 8000,
              cardMinor: 4000,
              bankTransferMinor: 0,
              totalMinor: 12000,
              refundsOrVoidsMinor: -3000,
              refundsOrVoidsCount: 1,
            },
          ],
        };
      },
      async getShiftTurnover() {
        return {
          rows: [
            {
              shiftId: 1,
              openedAt: "2026-06-17T07:30:00Z",
              closedAt: null,
              cashierName: "Mira Kasir",
              receiptCount: 2,
              cashMinor: 8000,
              cardMinor: 4000,
              bankTransferMinor: 0,
              totalMinor: 12000,
            },
          ],
        };
      },
      async getCashierTurnover() {
        return {
          rows: [
            {
              cashierId: 1,
              cashierName: "Mira Kasir",
              receiptCount: 2,
              totalMinor: 12000,
            },
          ],
        };
      },
      async getPaymentMethodTurnover() {
        return {
          rows: [
            { paymentMethod: "cash" as const, receiptCount: 2, totalMinor: 8000 },
            { paymentMethod: "card" as const, receiptCount: 1, totalMinor: 4000 },
          ],
        };
      },
      async getProductSales() {
        return {
          rows: [
            {
              productId: 1,
              productName: "Kafa 200g",
              productSku: "KAF-200",
              quantityMilli: 2000,
              revenueMinor: 12000,
              discountMinor: 200,
              estimatedMarginMinor: 11160,
            },
          ],
        };
      },
      async getCategorySales() {
        return {
          rows: [
            {
              categoryId: 1,
              categoryName: "Pica",
              quantityMilli: 3000,
              revenueMinor: 12000,
              discountMinor: 200,
              estimatedMarginMinor: 10990,
            },
          ],
        };
      },
      async getLowStock() {
        return {
          rows: [
            {
              productId: 1,
              productName: "Kafa 200g",
              productSku: "KAF-200",
              currentStockMilli: 3000,
              minimumStockMilli: 5000,
              differenceMilli: -2000,
              lastMovementAt: "2026-06-17T12:00:00Z",
            },
          ],
        };
      },
      async listShifts() {
        return [
          {
            id: 1,
            openedAt: "2026-06-17T07:30:00Z",
            closedAt: null,
            cashierName: "Mira Kasir",
          },
        ];
      },
      async exportReportCsv(request) {
        return {
          fileName: `${request.reportType}-2026-06-17-2026-06-17.csv`,
          path: `mock://exports/${request.reportType}-2026-06-17-2026-06-17.csv`,
          mimeType: "text/csv" as const,
          rowCount: 1,
        };
      },
      async getCashDepositReport(asOf) {
        requireAdmin();
        return buildMockCashDepositReport(
          asOf,
          mockDepositedMinor,
          cashDepositCalendar,
          shopProfile,
        );
      },
      async exportCashDepositCsv(asOf) {
        requireAdmin();
        return {
          fileName: `nedeponovani-gotov-novac-${asOf}.csv`,
          path: `mock://exports/nedeponovani-gotov-novac-${asOf}.csv`,
          mimeType: "text/csv" as const,
          rowCount: 1,
        };
      },
    },
    campaigns: {
      async listCampaigns() {
        return campaigns.map((campaign) => ({
          id: campaign.id,
          campaignType: campaign.campaignType,
          status: campaign.status,
          startsOn: campaign.startsOn,
          endsOn: campaign.endsOn,
          marketingLabel: campaign.marketingLabel,
          itemCount: campaign.items.length,
          overdue: campaign.overdue,
        }));
      },
      async getCampaign(id) {
        return findCampaign(campaigns, id);
      },
      // Mock fidelity only: the real report is computed in `crate::campaigns`
      // against the price log. An empty report here is the absence of a
      // verdict, not a finding that the campaign is lawful.
      async validateCampaign() {
        return { hard: [], warnings: [], anchors: [] };
      },
      async createCampaign(input) {
        const campaign = mockCampaignView(
          Math.max(0, ...campaigns.map((item) => item.id)) + 1,
          input,
          products,
        );
        campaigns.push(campaign);
        return campaign;
      },
      async updateCampaign(id, input) {
        const existing = findCampaign(campaigns, id);
        if (existing.status !== "draft") {
          throw new Error("Samo nacrt kampanje može da se izmeni.");
        }
        const updated = mockCampaignView(id, input, products);
        campaigns[campaigns.indexOf(existing)] = updated;
        return updated;
      },
      async activateCampaign(id) {
        const campaign = findCampaign(campaigns, id);
        campaign.status = "active";
        campaign.activatedAt = now;
        return campaign;
      },
      async adjustItemPrice(campaignId, productId, newPriceMinor) {
        const campaign = findCampaign(campaigns, campaignId);
        const item = campaign.items.find(
          (candidate) => candidate.productId === productId,
        );
        if (!item) {
          throw new Error("Artikal nije u kampanji.");
        }
        item.campaignPriceMinor = newPriceMinor;
        return campaign;
      },
      async endCampaign(id, overrides) {
        const campaign = findCampaign(campaigns, id);
        for (const override of overrides) {
          const product = products.find(
            (candidate) => candidate.id === override.productId,
          );
          if (product) {
            product.salePriceMinor = override.returnPriceMinor;
          }
        }
        campaign.status = "ended";
        campaign.endedAt = now;
        campaign.overdue = false;
        return campaign;
      },
      async cancelCampaign(id) {
        const campaign = findCampaign(campaigns, id);
        campaign.status = "cancelled";
        return campaign;
      },
      async exportEvidence(campaignId) {
        return {
          fileName: `dokaz-cene-kampanja-${campaignId}.html`,
          path: `mock://exports/dokaz-cene-kampanja-${campaignId}.html`,
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
      async exportLabels(campaignId) {
        return {
          fileName: `etikete-kampanja-${campaignId}.html`,
          path: `mock://exports/etikete-kampanja-${campaignId}.html`,
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
      async correctionReport() {
        return { rows: [] };
      },
      async exportCorrectionReport() {
        return {
          fileName: "ispravke-etiketa.html",
          path: "mock://exports/ispravke-etiketa.html",
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
    },
    // Mock fidelity only: the real deadline engine lives in `crate::reklamacije`
    // and derives every date from the event log per-read. This stub mirrors the
    // lifecycle — create assigns id + register number and freezes the regime;
    // each transition appends an event and re-derives a plausible DeadlineState.
    reklamacije: {
      async list() {
        return reklamacije.map((view) => ({
          id: view.id,
          registerNumber: view.registerNumber,
          regime: view.regime,
          status: view.status,
          podnosilacImePrezime: view.podnosilacImePrezime,
          filedAt: view.filedAt,
          answerDue: view.deadlines.answerDue,
          resolutionDue: view.deadlines.resolutionDue,
          answerOverdue: view.deadlines.answerOverdue,
          resolutionOverdue: view.deadlines.resolutionOverdue,
          purgeEligible: view.purgeEligible,
        }));
      },
      async get(id) {
        return findReklamacija(reklamacije, id);
      },
      async create(input) {
        const view = mockReklamacijaView(
          Math.max(0, ...reklamacije.map((item) => item.id)) + 1,
          Math.max(0, ...reklamacije.map((item) => item.registerNumber)) + 1,
          input,
        );
        reklamacije.push(view);
        return view;
      },
      async logAnswer(id, input) {
        const view = findReklamacija(reklamacije, id);
        assertNoFeeAttested(view, input.noFeeAttested);
        view.events.push({
          eventType: "answer_given",
          eventDate: input.eventDate,
          detailJson: null,
          consumerConsent: false,
        });
        view.status = "answered";
        recomputeMockDeadlines(view);
        return view;
      },
      async consumerReceived(id, eventDate) {
        const view = findReklamacija(reklamacije, id);
        view.events.push({
          eventType: "consumer_received_answer",
          eventDate,
          detailJson: null,
          consumerConsent: false,
        });
        view.status = "awaiting_consumer";
        recomputeMockDeadlines(view);
        return view;
      },
      async consumerResponded(id, eventDate) {
        const view = findReklamacija(reklamacije, id);
        view.events.push({
          eventType: "consumer_responded",
          eventDate,
          detailJson: null,
          consumerConsent: false,
        });
        view.status = "answered";
        recomputeMockDeadlines(view);
        return view;
      },
      async grantExtension(id, newDeadline, consumerConsent, reason, eventDate) {
        const view = findReklamacija(reklamacije, id);
        if (view.deadlines.oneExtensionUsed) {
          throw {
            code: "validation_error",
            message: "Rok je već jednom produžen.",
          };
        }
        view.events.push({
          eventType: "extension_granted",
          eventDate,
          detailJson: JSON.stringify({ newDeadline, reason }),
          consumerConsent,
        });
        recomputeMockDeadlines(view);
        return view;
      },
      async resolve(id, nacin, eventDate, noFeeAttested) {
        const view = findReklamacija(reklamacije, id);
        assertNoFeeAttested(view, noFeeAttested);
        view.events.push({
          eventType: "resolved",
          eventDate,
          detailJson: JSON.stringify({ nacin }),
          consumerConsent: false,
        });
        view.status = "resolved";
        recomputeMockDeadlines(view);
        return view;
      },
      async exportPotvrda(id) {
        const view = findReklamacija(reklamacije, id);
        return {
          fileName: `potvrda-reklamacija-${view.registerNumber}.html`,
          path: `mock://exports/potvrda-reklamacija-${view.registerNumber}.html`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      async exportNotice() {
        return {
          fileName: "obavestenje-reklamacije.html",
          path: "mock://exports/obavestenje-reklamacije.html",
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
    },
    // Mock fidelity only: the real ledger is derived per-read in `crate::kep`
    // from the append-only `kep_entries`. This stub keeps an in-memory list,
    // recomputes the saldo on read, and appends a razduženje per posted day.
    kep: {
      async ledger(bookYear) {
        const openingSaldoMinor = 0;
        const saldoMinor =
          openingSaldoMinor +
          kepEntries.reduce(
            (sum, entry) =>
              sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
            0,
          );
        return { bookYear, entries: [...kepEntries], openingSaldoMinor, saldoMinor };
      },
      async postDailySales(date, overrideAmountMinor) {
        const entry: KepEntryView = {
          redniBroj: kepEntries.length + 1,
          datum: `${date.slice(8, 10)}.${date.slice(5, 7)}`,
          opis: `Dnevni promet ${date}`,
          zaduzenjeMinor: null,
          razduzenjeMinor: overrideAmountMinor ?? 234000,
          kind: "daily_sales",
        };
        kepEntries.push(entry);
        return entry;
      },
      async status() {
        return { overdueSalesDays: [], unbookedReceiptCount: 0 };
      },
      // Mock fidelity only: the real kalkulacija/nivelacija/storno surface lives
      // in `crate::kep_kalkulacija` / `crate::kep_storno`; these stubs keep the
      // KEP module's UI tests deterministic without touching the ledger.
      async listKalkulacije() {
        return [];
      },
      async exportKalkulacija(id) {
        return {
          fileName: `kalkulacija-${id}.html`,
          path: `mock://exports/kalkulacija-${id}.html`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      // The KEP hook and the popis module read ONE notice
      // (`MOCK_NIVELACIJA_OBAVESTENJE`) for the reason the backend composes one:
      // the duty has a single wording, and two copies would let one surface
      // soften what the other hardened.
      async nivelacija() {
        return MOCK_NIVELACIJA_OBAVESTENJE;
      },
      async postAdjustment() {},
      async correctEntry() {},
      async closePreview(bookYear) {
        const already = kepClosures.some((c) => c.bookYear === bookYear);
        const saldoMinor = kepEntries.reduce(
          (sum, entry) =>
            sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
          0,
        );
        return {
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          alreadyClosed: already,
        };
      },
      async closeYear(bookYear, confirmation) {
        if (confirmation !== "ZAKLJUČI KNJIGU") {
          throw new Error("Potvrda nije ispravna.");
        }
        if (kepClosures.some((c) => c.bookYear === bookYear)) {
          throw new Error("Godina je već zaključena.");
        }
        const saldoMinor = kepEntries.reduce(
          (sum, entry) =>
            sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
          0,
        );
        const closure: KepClosure = {
          bookYear,
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          closedAt: "2027-01-05T09:00:00Z",
          closedBy: 1,
        };
        kepClosures.push({
          bookYear,
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          closedAt: closure.closedAt,
          purgeEligible: false,
          retentionUntil: `${bookYear + 5}-12-31`,
        });
        return closure;
      },
      async listClosures() {
        return [...kepClosures];
      },
      async exportClose(bookYear) {
        return {
          fileName: `kep-zakljucenje-${bookYear}.html`,
          path: `mock://exports/kep-zakljucenje-${bookYear}.html`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      async exportBook(bookYear) {
        return {
          fileName: `kep-knjiga-${bookYear}.html`,
          path: `mock://exports/kep-knjiga-${bookYear}.html`,
          mimeType: "text/html" as const,
          rowCount: kepEntries.length,
        };
      },
    },
    // Mock fidelity only: `crate::commands::worktime` is the source of truth.
    // Reproduced here because the register's UI turns on behaviour a naive stub
    // would flatten — a cap breach that ASKS for a ground instead of refusing,
    // and a correction that appends a new verzija instead of overwriting.
    worktime: {
      async listMonth(userId, godina, mesec) {
        return buildMonth(userId, godina, mesec, razlogOdsustvaDostupan());
      },
      async saveEntry(request) {
        return writeWorkTimeEntry(request, null);
      },
      async correctEntry(request) {
        const { korekcijaRazlog, ...entry } = request;
        return writeWorkTimeEntry(entry, korekcijaRazlog);
      },
      async closePeriod(userId, godina, mesec) {
        if (
          workTimePeriods.some(
            (period) =>
              period.userId === userId &&
              period.godina === godina &&
              period.mesec === mesec,
          )
        ) {
          throw {
            code: "period_closed",
            message: `Period ${nazivPerioda(godina, mesec)}. je zaključen i više se ne može menjati. Zaključenje je konačno.`,
          };
        }

        workTimePeriods.push({ userId, godina, mesec, closedAt: now });
        // `false` whatever the nalog says, like `close_period`: the frozen Class A
        // classification is minutes and a day count and has never carried the
        // category, so the read that feeds it has no business selecting one.
        const month = buildMonth(userId, godina, mesec, false);

        return {
          userId,
          godina,
          mesec,
          closedAt: now,
          closedBy: session?.user.id ?? 1,
          klasifikacija: {
            userId,
            godina,
            mesec,
            danaSaUnosom: month.entries.filter((entry) => !entry.zamenjen).length,
            minuti: month.ukupno,
            izvedenoU: now,
          },
        };
      },
      async exportCsv(userId, godina, mesec) {
        // The export masks because it goes through the register read: a file that
        // carried what the screen withholds would outlive the nalog.
        const month = buildMonth(userId, godina, mesec, razlogOdsustvaDostupan());
        const fileName = `evidencija-radnog-vremena-${userId}-${godina}-${String(mesec).padStart(2, "0")}.csv`;

        return {
          fileName,
          path: `mock://exports/${fileName}`,
          mimeType: "text/csv" as const,
          rowCount: month.entries.filter((entry) => !entry.zamenjen).length,
        };
      },
      async myHours(godina, mesec) {
        // `true` whatever the nalog says, like `my_hours`: the viewer is the data
        // subject, and req. 28 is about the vendor rather than about the person
        // whose month it is. Withholding here would take a certain ZZPL čl. 26
        // harm to prevent a disclosure the mask cannot stop anyway — the operator
        // watching the screen sees the pixels, not the JSON.
        return buildMonth(session?.user.id ?? 1, godina, mesec, true);
      },
      async notices() {
        return {
          recordMissing: overtimeRecordMissingNotice,
          capsExceeded: overtimeCapsExceededNotice,
          preraspodelaCapsExceeded: preraspodelaCapsExceededNotice,
        };
      },
    },
    privacy: {
      async grantSupportAccess(scope, durationMinutes) {
        const obim = scope.trim();
        if (!obim) {
          throw {
            code: "validation_error",
            message:
              "Nalog mora da navede obim pristupa koji se odobrava (ZZPL čl. 46).",
          };
        }
        if (!Number.isInteger(durationMinutes) || durationMinutes <= 0) {
          throw {
            code: "validation_error",
            message:
              "Trajanje naloga mora da bude izraženo u punim minutima i veće od nule.",
          };
        }

        const granted: SupportSession = {
          id: supportSessions.length + 1,
          grantedBy: session?.user.id ?? 1,
          grantedByName: session?.user.displayName ?? "Administrator",
          grantedAt: now,
          scope: obim,
          expiresAt: plusMinutes(now, durationMinutes),
          startedAt: null,
          endedAt: null,
          revokedAt: null,
          // Masked is the default and no nalog is ever born disclosed.
          odsustvoOtkrivenoAt: null,
        };
        supportSessions = [...supportSessions, granted];
        return granted;
      },
      async enterSupportSession() {
        const live = liveSupportSession();
        if (!live) {
          throw {
            code: "support_bez_naloga",
            message:
              "Pristup tehničke podrške nije odobren. Vlasnik mora prvo da izda nalog " +
              "sa obimom i rokom (ZZPL čl. 46).",
          };
        }
        live.startedAt = live.startedAt ?? now;
        return { ...live };
      },
      async endSupportSession() {
        const live = liveSupportSession();
        if (!live) {
          throw {
            code: "support_nema_sesije",
            message: "Nema otvorenog naloga za pristup tehničke podrške.",
          };
        }
        // A nalog that was entered is ended; one that never was is revoked.
        if (live.startedAt) {
          live.endedAt = now;
        } else {
          live.revokedAt = now;
        }
        return { ...live };
      },
      async activeSupportSession() {
        const live = liveSupportSession();
        return live ? { ...live } : null;
      },
      async revealAbsenceReason() {
        const live = liveSupportSession();
        if (!live) {
          throw {
            code: "support_bez_naloga_za_otkrivanje",
            message:
              "Razlog odsustva može da se otkrije samo dok važi nalog za pristup " +
              "tehničke podrške. Bez naloga nema kome da se otkrije (ZZPL čl. 46).",
          };
        }
        // Idempotent, like the backend: one disclosure per nalog, and the first
        // instant survives — a second stamp would put a second čl. 48 line in the
        // log for a category that was already open.
        live.odsustvoOtkrivenoAt = live.odsustvoOtkrivenoAt ?? now;
        return { ...live };
      },
      async searchAudit(query) {
        const events = filterAudit(query);
        return {
          events,
          chain: {
            verdict: "intact",
            intact: true,
            // The verdict covers the whole log, never the filtered slice.
            checkedRows: auditEvents.length,
            label: "Potvrđena — lanac otisaka je neprekinut.",
          },
        } satisfies AuditSearchResult;
      },
      async exportAuditCsv(query) {
        const events = filterAudit(query);
        const fileName = "izvod-evidencija-pristupa.csv";

        return {
          fileName,
          path: `mock://exports/${fileName}`,
          mimeType: "text/csv" as const,
          rowCount: events.length,
        };
      },
      async listBreaches() {
        return breaches
          .map((breach) => ({ ...breach }))
          .sort((left, right) => right.saznanjeAt.localeCompare(left.saznanjeAt));
      },
      async recordBreach(draft) {
        const recorded = derivedBreach(
          { ...draft, id: breaches.length + 1, createdAt: now, updatedAt: now },
        );
        breaches = [...breaches, recorded];
        return { ...recorded };
      },
      async updateBreach(id, draft) {
        const existing = breaches.find((breach) => breach.id === id);
        if (!existing) {
          throw { code: "not_found", message: "Povreda nije pronađena." };
        }
        if (draft.saznanjeAt !== existing.saznanjeAt) {
          throw {
            code: "povreda_saznanje_nepromenljivo",
            message:
              "Vreme saznanja za povredu je nepromenljivo — od njega teče rok od 72 časa " +
              "(ZZPL čl. 52 st. 1). Ako je uneto pogrešno, evidentirajte novu povredu.",
          };
        }

        const updated = derivedBreach({
          ...draft,
          id,
          createdAt: existing.createdAt,
          updatedAt: now,
        });
        breaches = breaches.map((breach) => (breach.id === id ? updated : breach));
        return { ...updated };
      },
      async breachNotice() {
        return breachNotificationNotice;
      },
      async exportBreachObrazac(id) {
        const fileName = `obrazac-povreda-podataka-${id}.html`;

        return {
          fileName,
          path: `mock://exports/${fileName}`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      async listProcessingActivities() {
        return processingActivities.map((activity) => ({ ...activity }));
      },
      async generateProcessingActivities() {
        processingActivities = processingActivities.map((activity) => ({
          ...activity,
          updatedAt: now,
        }));
        return processingActivities.map((activity) => ({ ...activity }));
      },
      async exportProcessingActivities() {
        const fileName = "evidencija-radnji-obrade.html";

        return {
          fileName,
          path: `mock://exports/${fileName}`,
          mimeType: "text/html" as const,
          rowCount: processingActivities.length,
        };
      },
    },
    retention: {
      async listPolicies() {
        return retentionPolicies.map((policy) => ({ ...policy }));
      },
      async extendPolicy(recordClass, retainUntil) {
        const policy = retentionPolicies.find(
          (row) => row.recordClass === recordClass,
        );
        if (!policy) {
          throw {
            code: "not_found",
            message: `Klasa čuvanja „${recordClass}“ nije upisana u tabelu rokova.`,
          };
        }
        if (!policy.adjustable) {
          throw {
            code: "validation_error",
            message:
              `Rok za „${policy.naziv}“ se ne podešava: ova evidencija se čuva trajno, ` +
              "a trajno je odsustvo roka — svaki datum bi ga skratio.",
          };
        }
        if (!/^\d{4}-\d{2}-\d{2}$/.test(retainUntil)) {
          throw {
            code: "validation_error",
            message: "Rok čuvanja mora biti u obliku gggg-MM-dd.",
          };
        }
        // The one direction a rok may move. Refused, never clamped — a silent
        // max() would hide the caller that tried to shorten it.
        if (policy.retainUntil && retainUntil < policy.retainUntil) {
          throw {
            code: "validation_error",
            message:
              `Rok čuvanja se pomera samo unapred: „${policy.retainUntil}“ ` +
              `se ne skraćuje na „${retainUntil}“.`,
          };
        }

        policy.retainUntil = retainUntil;
        policy.updatedAt = now;
        return { ...policy };
      },
    },
    cenovnik: {
      async listSnapshots() {
        // Newest first, and „current“ derived rather than stored — the same
        // reading `cenovnik_list_snapshots` takes, so a stale pointer cannot
        // exist here either.
        return [...cenovnikArchive]
          .reverse()
          .map(({ snapshot }, index) => ({ ...snapshot, current: index === 0 }));
      },
      async getSnapshot(snapshotId) {
        const found = cenovnikArchive.find(
          (row) => row.snapshot.id === snapshotId,
        );
        if (!found) {
          return null;
        }

        return {
          snapshot: {
            ...found.snapshot,
            current:
              found.snapshot.id ===
              cenovnikArchive[cenovnikArchive.length - 1]?.snapshot.id,
          },
          body: found.body,
        };
      },
      /**
       * `commands::cenovnik::outlet`: the frozen archive key once anything has
       * been published, otherwise what the company settings identify the shop
       * as — address first, then the name. The seeded shop has published, so
       * this is the frozen key and an edit to the address does not move it.
       *
       * `null` is the state in which nothing is generated, archived or
       * published at all, and a double that always answered a name would hide
       * the branch the panel exists to show.
       */
      async getOutlet() {
        if (cenovnikArchive.length > 0) {
          return MOCK_OUTLET;
        }

        return (
          [companySettings.address, companySettings.shopName]
            .map((candidate) => candidate.trim())
            .find((candidate) => candidate.length > 0) ?? null
        );
      },
      async getPublishTarget() {
        return cenovnikTarget;
      },
      async setPublishTarget(target) {
        // `cenovnik_set_publish_target` is `require_admin`: where the shop's
        // published prices go is what čl. 6 st. 4 then binds it to.
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }
        if (target.kind === "localFolder") {
          const folder = target.folder.trim();
          if (!folder) {
            throw {
              code: "validation_error",
              message: "Folder za objavu cenovnika je obavezan.",
            };
          }
          // The backend refuses a relative path: it would resolve against
          // whatever directory the app was launched from, so the shop would be
          // told the cenovnik is published and be unable to say where.
          if (!/^([/\\]|[A-Za-z]:[/\\])/.test(folder)) {
            throw {
              code: "validation_error",
              message:
                "Putanja do foldera mora biti puna putanja, na primer " +
                "„/Users/ana/cenovnik“.",
            };
          }
          cenovnikTarget = { kind: "localFolder", folder };
        } else {
          cenovnikTarget = { kind: "notConfigured" };
        }

        return cenovnikTarget;
      },
      /**
       * `penalty` is deliberately `null` whatever the legal form, for the same
       * reason `assessCashPayment` leaves it null: every statutory fine figure
       * lives in `src-tauri/src/legal.rs` and nowhere else, and a second copy
       * here could silently drift out of tier. The panel renders a null penalty
       * as a pointer to Podešavanja → Profil, never as a figure.
       */
      async getNotice() {
        return {
          summary:
            "Trgovac je dužan da na svojoj internet stranici, posebno za svaki " +
            "prodajni objekat, objavi cenovnik u digitalnom obliku pogodnom za " +
            "automatsku obradu i da ga ažurira u realnom vremenu. U cenovniku " +
            "se, kao i na prodajnom mestu, ističu prodajna i jedinična cena. " +
            "Zakon nigde ne propisuje obavezu trgovca da ima internet stranicu, " +
            "pa za trgovca koji je nema nije razjašnjeno da li je dužan da je " +
            "izradi.",
          penalty: null,
          citation:
            "Zakon o zaštiti potrošača (Sl. glasnik RS, br. 35/2026), čl. 6 " +
            "st. 1–3; prekršaj: čl. 210. Nadzor: tržišna inspekcija.",
          isLegalDuty: true,
        } satisfies LegalNotice;
      },
    },
    // A test double, not a second implementation. The rules that decide what a
    // popis may do live in `crate::popis` and `crate::commands::popis`; what is
    // kept here is the shape plus the four properties a UI test would otherwise
    // be able to pass vacuously against: the čl. 20 st. 3 gate (req. 39), the
    // čl. 8 st. 5 blind read (req. 29), the five arrows of the state machine,
    // and the posting lock (req. 41).
    popis: {
      async list() {
        return popisSessions.map((session) => ({
          id: session.id,
          vrsta: session.vrsta,
          prodajnoMesto: session.prodajnoMesto,
          datumPopisa: session.datumPopisa,
          status: session.status,
          postedAt: session.postedAt,
          brojLinija: session.linije.length,
        }));
      },
      async get(id) {
        return popisView(popisById(id));
      },
      async proveraListi(id, prijavljene) {
        const session = popisById(id);
        const nedostaju = POPIS_LISTE.filter(
          (lista) =>
            prijavljene.includes(lista.vrsta) &&
            !session.linije.some((linija) => linija.listaVrsta === lista.vrsta),
        ).map((lista) => ({ ...lista, brojStavki: 0 }));

        return {
          spremno: nedostaju.length === 0,
          nedostaju,
          poruka:
            nedostaju.length === 0
              ? null
              : "Popisne liste nisu potpune: " +
                nedostaju.map((lista) => lista.naziv).join(", ") +
                ". Prijavljena kategorija bez ijedne stavke ne može u izveštaj.",
        };
      },
      async open(request) {
        // Req. 39 — ZoRač čl. 20 st. 3 legislates the ordering, so the double
        // refuses here exactly as the backend does. Without this a UI test
        // could „open“ a popis the real app would never have opened.
        if (!request.uskladjivanjePotvrdjeno) {
          throw new Error(
            "Popis se ne može otvoriti dok se ne potvrdi usklađivanje glavne " +
              "knjige sa dnevnikom i pomoćnih knjiga sa glavnom knjigom " +
              "(ZoRač čl. 20 st. 3).",
          );
        }

        const session: MockPopis = {
          id: popisSessions.length + 1,
          vrsta: request.vrsta,
          prodajnoMesto: request.prodajnoMesto,
          datumPopisa: request.datumPopisa,
          periodFrom: request.periodFrom,
          periodTo: request.periodTo,
          status: "draft",
          planRadaJson: request.planRadaJson,
          // Req. 35 / PoP čl. 8 st. 2 — opening a popis approves nothing, and
          // this double has no approval verb to set them with either.
          planRadaOdobrio: null,
          planRadaOdobrenoAt: null,
          odlukaRef: request.odlukaRef,
          odlukaDonetaAt: null,
          perpetualOdlukaRef: request.perpetualOdlukaRef,
          uskladjivanjePotvrdjenoAt: now,
          postedAt: null,
          komisija: request.komisija.map((clan, index) => ({
            id: index + 1,
            ime: clan.ime,
            uloga: clan.uloga,
            rukujeImovinom: clan.rukujeImovinom,
          })),
          potpisi: [],
          linije: [],
        };
        popisSessions.push(session);
        return popisView(session);
      },
      async saveLine(sessionId, lineId, input) {
        const session = popisById(sessionId);
        popisEnsureOtvoren(session);

        // Req. 30 — the two potpisi freeze two snapshots. A stavka that could
        // still be edited after the čl. 8 st. 5 signature would make that
        // signature attest to a state that no longer exists.
        if (session.status === "counted_signed") {
          throw new Error(
            "Stvarno stanje je potpisano (PoP čl. 8 st. 5) — popisne liste se više ne menjaju.",
          );
        }
        if (session.status === "computed_signed") {
          throw new Error(
            "Obračunate popisne liste su potpisane (PoP čl. 9 st. 3) i više se ne menjaju.",
          );
        }
        if (session.status === "computed" && lineId === null) {
          throw new Error(
            "Nova stavka se ne dodaje posle potpisa stvarnog stanja — nju niko nije prebrojao (PoP čl. 8 st. 5).",
          );
        }

        // Req. 29. The refusal, not a silent drop: a caller that had the field
        // discarded would believe it had stored book data.
        if (
          input.knjigovodstvenaKolicinaMilli !== null &&
          !popisKnjigovodstvoDostupno(session)
        ) {
          throw new Error(
            "Knjigovodstvena količina se ne unosi pre potpisa stvarnog stanja " +
              "(PoP čl. 8 st. 5).",
          );
        }

        const stored: PopisLineView = {
          id: lineId ?? session.linije.length + 1,
          listaVrsta: input.listaVrsta,
          sifra: input.sifra,
          naziv: input.naziv,
          vrsta: input.vrsta,
          jedinicaMere: input.jedinicaMere,
          stvarnaKolicinaMilli: input.stvarnaKolicinaMilli,
          bliziOpis: input.bliziOpis,
          knjigovodstvenaKolicinaMilli: input.knjigovodstvenaKolicinaMilli,
          razlikaMilli: null,
          cenaMinor: input.cenaMinor,
        };
        const existing = session.linije.findIndex((linija) => linija.id === lineId);
        if (existing >= 0) {
          session.linije[existing] = stored;
        } else {
          session.linije.push(stored);
        }

        return popisView(session);
      },
      async startCount(id) {
        return popisView(popisAdvanceSession(id, "draft", "counting"));
      },
      async signPhaseA(id, potpisnici) {
        const session = popisById(id);
        popisPotpis(session, "a", potpisnici, "counting");
        // The statutory order: the potpis first, the book quantities only
        // after it. The v20 write guard aborts the other way round.
        session.status = "counted_signed";
        for (const linija of session.linije) {
          if (linija.knjigovodstvenaKolicinaMilli === null) {
            linija.knjigovodstvenaKolicinaMilli = popisKnjigovodstvo(linija);
          }
        }
        return popisView(session);
      },
      async compute(id) {
        const session = popisAdvanceSession(id, "counted_signed", "computed");
        for (const linija of session.linije) {
          linija.razlikaMilli =
            linija.knjigovodstvenaKolicinaMilli === null
              ? null
              : linija.stvarnaKolicinaMilli - linija.knjigovodstvenaKolicinaMilli;
        }
        return popisView(session);
      },
      async signPhaseB(id, potpisnici) {
        const session = popisById(id);
        popisPotpis(session, "b", potpisnici, "computed");
        session.status = "computed_signed";
        return popisView(session);
      },
      async post(id) {
        const session = popisAdvanceSession(id, "computed_signed", "posted");
        session.postedAt = now;
        return popisView(session);
      },
      async getPodesavanja() {
        return { ...popisPodesavanja };
      },
      async setPodesavanja(podesavanja) {
        popisPodesavanja = {
          rokPredajeFi: podesavanja.rokPredajeFi?.trim() || null,
        };
        return { ...popisPodesavanja };
      },
      async nivelacijaPregled() {
        return {
          obaveze: [],
          obavestenje: MOCK_NIVELACIJA_OBAVESTENJE,
          izvor:
            "Prati se: nivelacije i izmene cene u katalogu artikala. Ne prate " +
            "se: akcijske cene iz kampanja, cene iz uvoza artikala.",
        };
      },
      async nivelacijaObuhvat(id, obuhvat) {
        const session = popisById(id);
        return {
          sessionId: session.id,
          datumPopisa: session.datumPopisa,
          obuhvat: obuhvat ?? "samo_nivelisani",
          obavestenje: MOCK_NIVELACIJA_OBAVESTENJE,
          // Four čl. 8 st. 4 fields and no količina among them — the scope list
          // is read before anything is counted (req. 29).
          artikli: products.map((product) => ({
            sifra: product.sku,
            naziv: product.name,
            vrsta: product.categoryName,
            jedinicaMere: product.unitOfMeasure,
          })),
          vecNaListama: 0,
        };
      },
      async izvestaj(id, request) {
        const session = popisById(id);
        if (!popisKnjigovodstvoDostupno(session)) {
          throw new Error(
            "Izveštaj o popisu se ne sastavlja pre potpisa stvarnog stanja " +
              "(PoP čl. 8 st. 5).",
          );
        }

        return {
          sessionId: session.id,
          vrsta: session.vrsta,
          status: session.status,
          obveznik: "Butik Primer pr Novi Pazar",
          pib: "100000001",
          maticniBroj: "60000001",
          prodajnoMesto: session.prodajnoMesto,
          datumPopisa: session.datumPopisa,
          periodFrom: session.periodFrom,
          periodTo: session.periodTo,
          komisija: session.komisija,
          potpisi: session.potpisi,
          elementi: POPIS_IZVESTAJ_ELEMENTI.map((element) => ({
            ...element,
            tekst: element.narativni
              ? request.narativ[element.polje as keyof IzvestajNarativ]
              : null,
          })),
          liste: POPIS_LISTE.map((lista) => ({
            vrsta: lista.vrsta,
            naziv: lista.naziv,
            pravniOsnov: lista.pravniOsnov,
            zbir: popisZbir(
              session.linije.filter((linija) => linija.listaVrsta === lista.vrsta),
            ),
          })),
          ukupno: popisZbir(session.linije),
          rok:
            session.vrsta === "nivelacioni"
              ? "30 dana po izvršenom popisu"
              : "30.01.2027",
          rokPravniOsnov: "PoP čl. 13 st. 2",
          odlukaOUsvajanju: {
            rok:
              session.vrsta === "nivelacioni"
                ? "30 dana po izvršenom popisu"
                : "30.01.2027",
            pravniOsnov: "PoP čl. 14 st. 2",
            donosilac: "preduzetnik lično",
            napomena:
              "Aplikacija ne evidentira odluku o usvajanju izveštaja — donesite " +
              "je i čuvajte uz izveštaj.",
          },
          upozorenja: [
            "Izveštaj o popisu se ne čuva u aplikaciji, a program ga ne štampa " +
              "i ne izvozi — štampani primerak sastavite sami i čuvajte ga uz " +
              "popisne liste. Popisne liste, odluku o popisu i plan rada program " +
              "izvozi u datoteku za štampu; izveštaj nije među njima.",
          ],
        };
      },
      /**
       * `crate::commands::popis::faza_stampe`. The phase is **derived**, and a
       * čl. 9 st. 3 sheet asked for before the čl. 8 st. 5 potpis is refused by
       * name rather than quietly downgraded — a caller handed the counted sheet
       * under the name it asked for would believe it had printed the obračun.
       * An explicit `"a"` is honoured in every state: over-withholding breaches
       * nothing, and čl. 2 st. 6 owes the owner of tuđa roba a primerak of the
       * signed counted lista after the potpis.
       */
      async exportLista(id, faza, lista) {
        const session = popisById(id);
        const dostupno = popisKnjigovodstvoDostupno(session);
        if (!dostupno && faza === "b") {
          throw new Error(
            "Obračunate popisne liste ne mogu da se odštampaju pre nego što se " +
              "stvarno stanje unese u popisne liste i pre nego što članovi " +
              "komisije potpišu te liste (PoP čl. 8 st. 5).",
          );
        }

        const primenjena = dostupno ? (faza ?? "b") : "a";
        // The čl. 2 st. 6 primerak: one lista, its own file name so it cannot
        // overwrite the bundle, and a row count of what that document carries
        // rather than of the whole popis.
        const ime =
          lista === null
            ? `popisne-liste-${id}-faza-${primenjena}.html`
            : `popisna-lista-${id}-${lista}-faza-${primenjena}.html`;
        return {
          fileName: ime,
          path: `mock://exports/${ime}`,
          mimeType: "text/html" as const,
          rowCount:
            lista === null
              ? session.linije.length
              : session.linije.filter((linija) => linija.listaVrsta === lista)
                  .length,
        };
      },
      async exportOdluka(id) {
        const session = popisById(id);
        return {
          fileName: `odluka-o-popisu-${id}.html`,
          path: `mock://exports/odluka-o-popisu-${id}.html`,
          mimeType: "text/html" as const,
          rowCount: session.komisija.length,
        };
      },
      async exportPlanRada(id) {
        const session = popisById(id);
        return {
          fileName: `plan-rada-${id}.html`,
          path: `mock://exports/plan-rada-${id}.html`,
          mimeType: "text/html" as const,
          rowCount: session.komisija.length,
        };
      },
      /**
       * `crate::commands::popis::odobri_plan_rada`, refusals and order both.
       *
       * The blank name is checked **before** the popis is looked up, because
       * that is where the backend checks it, and it is refused rather than
       * filled in from the shop's registered name: a server-side default would
       * be an auto-approval wearing a default's clothes. The čl. 14 st. 3 lock
       * is the second gate. Both columns are written together — v21 pairs them
       * in a CHECK, and half an approval is not an approval.
       */
      async odobriPlan(id, odobrio) {
        const ime = odobrio.trim();
        if (ime === "") {
          throw new Error(
            "Plan rada odobrava lice iz člana 4. stav 2. Pravilnika o popisu, " +
              "kod preduzetnika sam preduzetnik (PoP čl. 8 st. 2) — unesite ime " +
              "i prezime tog lica. Odobrenje bez imena se ne evidentira.",
          );
        }

        const session = popisById(id);
        popisEnsureOtvoren(session);
        session.planRadaOdobrio = ime;
        session.planRadaOdobrenoAt = now;
        return popisView(session);
      },
    },
    print: {
      async openForPrint() {},
      async openExternalUrl() {},
    },
  };

  /** The outlet's newest publication — the file čl. 6 st. 4 binds it to today. */
  function currentCenovnik() {
    return cenovnikArchive[cenovnikArchive.length - 1];
  }

  function popisById(id: number): MockPopis {
    const found = popisSessions.find((candidate) => candidate.id === id);
    if (!found) {
      throw new Error("Popis nije pronađen.");
    }
    return found;
  }

  /**
   * `crate::popis::book_quantities_released` — **both** limbs. The status limb
   * alone is module-private backend-side precisely so nothing can reach it: a
   * session can be born in `counted_signed` with no potpis behind it, and this
   * double must not be the one place where that shortcut works.
   */
  function popisKnjigovodstvoDostupno(session: MockPopis): boolean {
    return (
      session.status !== "draft" &&
      session.status !== "counting" &&
      session.potpisi.some((potpis) => potpis.faza === "a")
    );
  }

  /** Req. 41 — čl. 14 st. 3 with ZoRač čl. 8 st. 4: a correction is a new popis. */
  function popisEnsureOtvoren(session: MockPopis) {
    if (session.status === "posted") {
      throw new Error(
        "Popis je proknjižen i više se ne menja — ispravka se sprovodi novim popisom.",
      );
    }
  }

  function popisAdvanceSession(
    id: number,
    from: PopisStatus,
    to: PopisStatus,
  ): MockPopis {
    const session = popisById(id);
    popisEnsureOtvoren(session);
    if (session.status !== from) {
      throw new Error(
        `Prelaz nije dozvoljen iz stanja „${session.status}“ u „${to}“.`,
      );
    }
    session.status = to;
    return session;
  }

  function popisPotpis(
    session: MockPopis,
    faza: "a" | "b",
    potpisnici: string[],
    from: PopisStatus,
  ) {
    popisEnsureOtvoren(session);
    if (session.status !== from) {
      throw new Error(`Potpis nije moguć u stanju „${session.status}“.`);
    }
    if (potpisnici.length === 0) {
      throw new Error("Popisnu listu potpisuju članovi komisije.");
    }
    for (const potpisnik of potpisnici) {
      session.potpisi.push({
        id: session.potpisi.length + 1,
        faza,
        potpisnik,
        potpisanoAt: now,
        snapshotHash: `mock-${faza}-${session.id}-${session.potpisi.length + 1}`,
      });
    }
  }

  /**
   * The perpetual stanje, read at the moment the čl. 8 st. 5 potpis is taken —
   * the earliest moment the article permits reading it at all. A šifra that
   * resolves to nothing keeps its `null`: the gotovina and potraživanja liste
   * have no perpetual record behind them, and a fabricated zero would report a
   * manjak the shop does not have.
   */
  function popisKnjigovodstvo(linija: PopisLineView): number | null {
    const product = products.find((candidate) => candidate.sku === linija.sifra);
    return product ? product.currentStockMilli : null;
  }

  function popisZbir(linije: PopisLineView[]): IzvestajZbir {
    const zbir: IzvestajZbir = {
      brojStavki: linije.length,
      stavkeBezCene: 0,
      stavkeBezKnjigovodstvenogStanja: 0,
      stavkeSaViskom: 0,
      stavkeSaManjkom: 0,
      vrednostPoPopisuMinor: 0,
      vrednostPoKnjigamaMinor: 0,
      vrednosnaRazlikaMinor: 0,
      potpuno: true,
    };

    for (const linija of linije) {
      if (linija.cenaMinor === null) {
        zbir.stavkeBezCene += 1;
        zbir.potpuno = false;
      }
      if (linija.knjigovodstvenaKolicinaMilli === null) {
        zbir.stavkeBezKnjigovodstvenogStanja += 1;
        zbir.potpuno = false;
      }
      if (linija.razlikaMilli !== null && linija.razlikaMilli > 0) {
        zbir.stavkeSaViskom += 1;
      }
      if (linija.razlikaMilli !== null && linija.razlikaMilli < 0) {
        zbir.stavkeSaManjkom += 1;
      }
      if (linija.cenaMinor !== null) {
        zbir.vrednostPoPopisuMinor += Math.round(
          (linija.stvarnaKolicinaMilli * linija.cenaMinor) / 1000,
        );
        if (linija.knjigovodstvenaKolicinaMilli !== null) {
          zbir.vrednostPoKnjigamaMinor += Math.round(
            (linija.knjigovodstvenaKolicinaMilli * linija.cenaMinor) / 1000,
          );
        }
      }
    }

    zbir.vrednosnaRazlikaMinor =
      zbir.vrednostPoPopisuMinor - zbir.vrednostPoKnjigamaMinor;
    return zbir;
  }

  /**
   * Req. 29 lives here, not in the caller. While the release predicate is false
   * the view carries **no** book quantity and **no** razlika: this double drops
   * them on the way out, where the real backend never reads them in the first
   * place. A double that leaked them would let a UI test that shows an
   * „očekivano“ column pass.
   */
  function popisView(session: MockPopis): PopisSessionView {
    const dostupno = popisKnjigovodstvoDostupno(session);
    const upozorenja: string[] = [];

    for (const clan of session.komisija) {
      if (clan.rukujeImovinom) {
        upozorenja.push(
          `${clan.ime} rukuje imovinom koja se popisuje — PoP čl. 5 st. 1 to ` +
            "ne dozvoljava za člana komisije. Za jedno lice iz čl. 6 st. 1 " +
            "shodna primena čl. 5 st. 1 nije razjašnjena. Popis nije zaustavljen.",
        );
      }
    }

    if (session.linije.some((linija) => linija.listaVrsta === "konsignacija")) {
      upozorenja.push(
        "Potpisanu konsignacionu listu dostavite vlasniku robe u roku od 10 " +
          "dana od dana popisa (PoP čl. 2 st. 6). Aplikacija je ne dostavlja.",
      );
    }

    return {
      id: session.id,
      vrsta: session.vrsta,
      prodajnoMesto: session.prodajnoMesto,
      datumPopisa: session.datumPopisa,
      periodFrom: session.periodFrom,
      periodTo: session.periodTo,
      status: session.status,
      planRadaJson: session.planRadaJson,
      planRadaOdobrio: session.planRadaOdobrio,
      planRadaOdobrenoAt: session.planRadaOdobrenoAt,
      odlukaRef: session.odlukaRef,
      odlukaDonetaAt: session.odlukaDonetaAt,
      perpetualOdlukaRef: session.perpetualOdlukaRef,
      uskladjivanjePotvrdjenoAt: session.uskladjivanjePotvrdjenoAt,
      postedAt: session.postedAt,
      fazaAPotpisana: session.potpisi.some((potpis) => potpis.faza === "a"),
      fazaBPotpisana: session.potpisi.some((potpis) => potpis.faza === "b"),
      knjigovodstvoDostupno: dostupno,
      komisija: session.komisija.map((clan) => ({ ...clan })),
      potpisi: session.potpisi.map((potpis) => ({ ...potpis })),
      // The blind read, as `read_lines` performs it. `cenaMinor` is withheld with
      // the rest of the Phase B block **except** on the two liste where it is not
      // a cena: the čl. 11 st. 1 apoen and the čl. 12 st. 2 iznos are the
      // commission's own counted figures, so they stay readable while the lista
      // is being written — otherwise the field is write-only during the count.
      linije: session.linije.map((linija) => ({
        ...linija,
        knjigovodstvenaKolicinaMilli: dostupno
          ? linija.knjigovodstvenaKolicinaMilli
          : null,
        razlikaMilli: dostupno ? linija.razlikaMilli : null,
        cenaMinor:
          dostupno ||
          linija.listaVrsta === "gotovina" ||
          linija.listaVrsta === "potrazivanja"
            ? linija.cenaMinor
            : null,
      })),
      liste: POPIS_LISTE.map((lista) => ({
        ...lista,
        brojStavki: session.linije.filter(
          (linija) => linija.listaVrsta === lista.vrsta,
        ).length,
      })),
      konsignacijaRok: session.linije.some(
        (linija) => linija.listaVrsta === "konsignacija",
      )
        ? popisPlusDana(session.datumPopisa, 10)
        : null,
      upozorenja,
    };
  }

  /**
   * Req. 11: a price change republishes, and nothing else does. Čl. 6 st. 3
   * wants the published file to match the outlet's current prices *„u realnom
   * vremenu“*, so this rides on the write that moved them — mirroring
   * `commands::catalog::republish_cenovnik`, including the jedinična cena, which
   * is a published price under st. 1/st. 2 and moves with the package content
   * without touching `salePriceMinor`.
   *
   * A double that skipped this would warn at the till after every ordinary price
   * raise, where the real backend stays silent because it republished first.
   */
  function republishCenovnik() {
    const generatedAt = `${new Date(
      Date.parse(now) + cenovnikArchive.length * 1000,
    )
      .toISOString()
      .slice(0, 19)}Z`;
    // Inactive articles are not offered, so they have no price to publish.
    const rows = products
      .filter((product) => product.active)
      .map((product) => ({
        sifra: product.sku,
        barkod: product.barcode,
        naziv: product.name,
        jedinicaMere: product.unitOfMeasure,
        prodajnaCenaMinor: product.salePriceMinor,
        jedinicnaCenaMinor: unitPriceMinor(product),
        jedinicaZaJedinicnuCenu: product.jedinicnaCenaJedinica ?? null,
      }))
      .sort((left, right) => left.sifra.localeCompare(right.sifra));
    const body = renderMockCenovnik(rows, generatedAt);

    cenovnikArchive.push({
      snapshot: {
        id: cenovnikArchive.length + 1,
        prodajnoMesto: MOCK_OUTLET,
        generatedAt,
        rowCount: rows.length,
        contentHash: mockContentHash(body),
        // No target is configured in the seeded shop, so the file is archived
        // and goes nowhere — the honest state, not a failure.
        publishedAt: null,
        publishedTarget: null,
        current: false,
      },
      body,
    });
  }

  /** „Did this write move a published price?“ — the sale price or the pair. */
  function movesAPublishedPrice(
    before: ProductSummary | undefined,
    after: ProductSummary,
  ): boolean {
    return (
      before === undefined ||
      before.active !== after.active ||
      before.salePriceMinor !== after.salePriceMinor ||
      (before.jedinicnaCenaJedinica ?? null) !==
        (after.jedinicnaCenaJedinica ?? null) ||
      (before.jedinicnaCenaSadrzajMilli ?? null) !==
        (after.jedinicnaCenaSadrzajMilli ?? null)
    );
  }

  /** Integer minutes, like every other duration in this app. */
  function plusMinutes(instant: string, minutes: number): string {
    return `${new Date(new Date(instant).getTime() + minutes * 60_000)
      .toISOString()
      .slice(0, 19)}Z`;
  }

  /**
   * The newest nalog that has neither ended nor been revoked and whose expiry
   * has not passed. Expiry is exclusive — at `expiresAt` the nalog is spent.
   */
  function liveSupportSession(): SupportSession | undefined {
    return [...supportSessions]
      .reverse()
      .find(
        (candidate) =>
          !candidate.endedAt &&
          !candidate.revokedAt &&
          candidate.grantedAt <= now &&
          now < candidate.expiresAt,
      );
  }

  function filterAudit(query: AuditQuery): AuditEvent[] {
    return auditEvents
      .filter((event) => !query.from || event.at.slice(0, 10) >= query.from)
      .filter((event) => !query.to || event.at.slice(0, 10) <= query.to)
      .filter(
        (event) =>
          query.actorUserId === null ||
          query.actorUserId === undefined ||
          event.actorUserId === query.actorUserId,
      )
      .map((event) => ({ ...event }));
  }

  /**
   * The four answers computed from `now`. `notifiable` is derived from the risk
   * assessment and is `null` until one exists — it never decided whether the
   * record was written (req. 43).
   */
  function derivedBreach(
    stored: BreachDraft & { id: number; createdAt: string; updatedAt: string },
  ): Breach {
    const saznanje = new Date(stored.saznanjeAt).getTime();
    const rok = plusMinutes(stored.saznanjeAt, 72 * 60);
    const notifiable =
      stored.riskOutcome === null || stored.riskOutcome === undefined
        ? null
        : stored.riskOutcome !== "bez_rizika";

    return {
      ...stored,
      rokObavestavanjaIsticeAt: rok,
      notifiable,
      delayReasonRequired:
        notifiable === true &&
        !stored.poverenikNotifiedAt &&
        new Date(now).getTime() - saznanje > 72 * 60 * 60 * 1000,
      obavestavanjeLicaObavezno: stored.riskOutcome === "visok_rizik",
    };
  }

  /**
   * SW-14 req. 28, as the backend decides it: no live nalog masks nothing, a
   * live nalog masks unless the shop disclosed the category for that nalog.
   *
   * Read off the nalog and never by reading the audit log back — SW-10 keeps
   * that log a record and not an access control.
   */
  function razlogOdsustvaDostupan(): boolean {
    const live = liveSupportSession();
    return !live || live.odsustvoOtkrivenoAt !== null;
  }

  /**
   * `razlogDostupan` is the caller's answer, never this function's, because the
   * three read paths give three different ones — which is the whole shape of
   * `commands::worktime::load_entries` and the reason it is a dispatcher over two
   * statements rather than one statement with a flag.
   *
   * **The mock cannot copy the structural half and does not pretend to.** The
   * backend withholds by running a SELECT that never names `kategorija_odsustva`,
   * so while masked there is no value in the row to drop, to log or to print; an
   * in-memory array has no query to leave the column out of, so the double
   * reproduces the observable contract instead — absent category, the flag set,
   * and the absence and its minutes untouched.
   */
  function buildMonth(
    userId: number,
    godina: number,
    mesec: number,
    razlogDostupan: boolean,
  ): WorkTimeMonth {
    const prefix = `${godina}-${String(mesec).padStart(2, "0")}`;
    const entries = withSupersedes(
      workTimeEntries.filter((entry) => entry.userId === userId),
    )
      .filter((entry) => entry.dan.startsWith(prefix))
      .sort((left, right) =>
        left.dan === right.dan
          ? left.verzija - right.verzija
          : left.dan.localeCompare(right.dan),
      )
      .map((entry) =>
        razlogDostupan ? entry : { ...entry, kategorijaOdsustva: null },
      );

    const ukupno = emptyMinutes();
    for (const entry of entries.filter((candidate) => !candidate.zamenjen)) {
      for (const key of Object.keys(ukupno) as (keyof WorkTimeMinutes)[]) {
        ukupno[key] += entry.minuti[key];
      }
    }

    const closed = workTimePeriods.find(
      (period) =>
        period.userId === userId &&
        period.godina === godina &&
        period.mesec === mesec,
    );

    return {
      userId,
      zaposleni:
        users.find((user) => user.id === userId)?.displayName ?? "Zaposleni",
      godina,
      mesec,
      zatvoren: Boolean(closed),
      closedAt: closed?.closedAt ?? null,
      razlogOdsustvaSkriven: !razlogDostupan,
      entries,
      ukupno,
      napomena: EVIDENCIJA_ZAGLAVLJE,
      advisoryNapomena: ADVISORY_TAG,
    };
  }

  function writeWorkTimeEntry(
    request: SaveWorkTimeEntryRequest,
    korekcijaRazlog: string | null,
  ) {
    const godina = Number(request.dan.slice(0, 4));
    const mesec = Number(request.dan.slice(5, 7));

    if (
      workTimePeriods.some(
        (period) =>
          period.userId === request.userId &&
          period.godina === godina &&
          period.mesec === mesec,
      )
    ) {
      throw {
        code: "period_closed",
        message: `Period ${nazivPerioda(godina, mesec)}. je zaključen i više se ne može menjati. Zaključenje je konačno.`,
      };
    }

    const live = withSupersedes(workTimeEntries).find(
      (entry) =>
        entry.userId === request.userId &&
        entry.dan === request.dan &&
        !entry.zamenjen,
    );

    if (korekcijaRazlog && !live) {
      throw {
        code: "not_found",
        message: "Za ovaj dan ne postoji unos koji bi se ispravio.",
      };
    }
    if (!korekcijaRazlog && live) {
      throw {
        code: "entry_exists",
        message:
          "Za ovaj dan već postoji unos. Izmena se evidentira kao ispravka.",
      };
    }

    const minuti = buildWorkTimeMinutes(request);
    const week = mondayOf(request.dan);
    const weeklyOvertimeMinutes =
      minuti.prekovremeniMinuta +
      withSupersedes(workTimeEntries)
        .filter(
          (entry) =>
            entry.userId === request.userId &&
            !entry.zamenjen &&
            entry.dan !== request.dan &&
            mondayOf(entry.dan) === week,
        )
        .reduce((sum, entry) => sum + entry.minuti.prekovremeniMinuta, 0);
    const dailyTotalMinutes =
      minuti.efektivnoIzvrseniMinuta + minuti.prekovremeniMinuta;
    const weeklyCapExceeded = weeklyOvertimeMinutes > WEEKLY_OVERTIME_CAP_MINUTES;
    const dailyCapExceeded = dailyTotalMinutes > DAILY_TOTAL_CAP_MINUTES;
    const caps = {
      weeklyOvertimeMinutes,
      dailyTotalMinutes,
      weeklyTotalMinutes: dailyTotalMinutes,
      weeklyCapExceeded,
      dailyCapExceeded,
      preraspodelaWeeklyCapExceeded: false,
      requiresOverride: weeklyCapExceeded || dailyCapExceeded,
    };

    // Never a dead end: the day is recordable, it just has to say why.
    if (caps.requiresOverride && !request.capOverrideRazlog) {
      throw {
        code: "cap_override_required",
        message:
          "Prekoračen je zakonski limit iz ZoR čl. 53. Dan se može evidentirati, " +
          "ali morate izabrati razlog prekoračenja.",
        details: { caps },
      };
    }

    const entry: WorkTimeEntryView = {
      id: workTimeEntries.length + 1,
      userId: request.userId,
      dan: request.dan,
      verzija: live ? live.verzija + 1 : 1,
      zamenjen: false,
      supersedesId: live?.id ?? null,
      kategorijaOdsustva: request.kategorijaOdsustva,
      capOverrideRazlog: request.capOverrideRazlog,
      korekcijaRazlog,
      unioUserId: session?.user.id ?? 1,
      unioIme: session?.user.displayName ?? "Administrator",
      createdAt: now,
      updatedAt: now,
      minuti,
    };
    workTimeEntries.push(entry);

    return { entry, caps, protections: [] };
  }

  return services;
}

function findCampaign(campaigns: CampaignView[], id: number): CampaignView {
  const campaign = campaigns.find((candidate) => candidate.id === id);
  if (!campaign) {
    throw new Error("Kampanja nije pronađena.");
  }

  return campaign;
}

function mockCampaignView(
  id: number,
  input: CampaignInput,
  products: ProductSummary[],
): CampaignView {
  const items: CampaignItemView[] = input.items.map((item) => {
    const product = products.find(
      (candidate) => candidate.id === item.productId,
    );

    return {
      productId: item.productId,
      productName: product?.name ?? `Artikal ${item.productId}`,
      sku: product?.sku ?? "",
      campaignPriceMinor: item.campaignPriceMinor,
      prethodnaCenaMinor:
        item.manualPrethodnaMinor ?? product?.salePriceMinor ?? null,
      anchorStatus: item.manualPrethodnaMinor == null ? "computed" : "manual",
      anchorWindowDays: item.manualPrethodnaMinor == null ? 30 : null,
      anchorTruncated: false,
      anchorReason: null,
      anchorJustification: item.anchorJustification ?? null,
      futureRegularPriceMinor: item.futureRegularPriceMinor ?? null,
      preCampaignPriceMinor: product?.salePriceMinor ?? null,
    };
  });

  return {
    id,
    campaignType: input.campaignType,
    status: "draft",
    startsOn: input.startsOn,
    endsOn: input.endsOn ?? null,
    displayMode: input.displayMode,
    headlinePercent: input.headlinePercent ?? null,
    rasprodajaGround: input.rasprodajaGround ?? null,
    specialConditions: input.specialConditions ?? null,
    reducedUtilityReason: input.reducedUtilityReason ?? null,
    marketingLabel: input.marketingLabel ?? null,
    seasonAttested: input.seasonAttested,
    separationAttested: input.separationAttested,
    activatedAt: null,
    endedAt: null,
    overdue: false,
    items,
    warnings: [],
  };
}

// The cutover the real engine reads from `crate::reklamacije::CUTOVER_DATE`.
// The regime is frozen here at create and never recomputed.
const REKLAMACIJA_CUTOVER = "2026-08-01T00:00:00Z";

/**
 * Mirrors `legal.rs::reklamacija_breach` — summary and citation only.
 *
 * `penalty` is deliberately `null` whatever the legal form, for the same reason
 * the AML double withholds it: every statutory fine figure lives in
 * `src-tauri/src/legal.rs` and nowhere else, and a second copy here could
 * silently drift out of tier. The citation is regime-versioned because the two
 * ZZP laws put the reklamacija duty and its prekršajne odredbe in different
 * articles.
 */
function reklamacijaNotice(regime: ReklamacijaRegime): LegalNotice {
  return {
    summary:
      "Nepostupanje po reklamaciji potrošača u propisanim rokovima je prekršaj.",
    penalty: null,
    citation:
      regime === "old"
        ? "Zakon o zaštiti potrošača (Sl. glasnik RS, br. 88/2021), čl. 55; " +
          "prekršajne odredbe čl. 188."
        : "Zakon o zaštiti potrošača (Sl. glasnik RS, br. 35/2026), čl. 63; " +
          "prekršajne odredbe čl. 210 st. 1 tač. 24.",
    isLegalDuty: true,
  };
}

/** `reklamacije.rs::NO_FEE_NOTICE` verbatim. NEW regime only. */
const REKLAMACIJA_NO_FEE_NOTICE =
  "Zabranjeno je naplatiti utvrđivanje nesaobraznosti (čl. 63 st. 3). " +
  "Otklanjanje nesaobraznosti — popravka ili zamena — je bez naknade po " +
  "posebnoj odredbi (čl. 56 st. 1), i u starom i u novom režimu.";

/** Mirrors `reklamacije.rs::ensure_no_fee_attested`. A double that accepts what
 *  the backend rejects hides the gate from every UI test that uses it. */
function assertNoFeeAttested(view: ReklamacijaView, attested: boolean): void {
  if (view.noFeeNotice === null || view.noFeeAttested) {
    return;
  }
  if (!attested) {
    throw {
      code: "validation_error",
      message:
        "Za novu reklamaciju potvrdite da utvrđivanje nesaobraznosti nije naplaćeno (čl. 63 st. 3).",
    };
  }
  view.noFeeAttested = true;
  view.noFeeAttestedAt = now;
}

function findReklamacija(
  reklamacije: ReklamacijaView[],
  id: number,
): ReklamacijaView {
  const view = reklamacije.find((candidate) => candidate.id === id);
  if (!view) {
    throw { code: "not_found", message: "Reklamacija nije pronađena." };
  }

  return view;
}

function mockReklamacijaView(
  id: number,
  registerNumber: number,
  input: ReklamacijaInput,
): ReklamacijaView {
  const regime: ReklamacijaRegime =
    new Date(input.filedAt) < new Date(REKLAMACIJA_CUTOVER) ? "old" : "new";
  const view: ReklamacijaView = {
    id,
    registerNumber,
    regime,
    status: "open",
    filedAt: input.filedAt,
    podnosilacImePrezime: input.podnosilacImePrezime,
    kontakt: input.kontakt,
    podaciORobi: input.podaciORobi,
    opisNesaobraznosti: input.opisNesaobraznosti,
    zahtev: input.zahtev,
    robaKind: input.robaKind,
    datumIzdavanjaPotvrde: now,
    createdBy: 1,
    createdAt: now,
    updatedAt: now,
    events: [],
    deadlines: {
      answerDue: now,
      resolutionDue: null,
      clock: "running",
      consumerWindowDue: null,
      answerOverdue: false,
      resolutionOverdue: false,
      oneExtensionUsed: false,
    },
    purgeEligible: false,
    notice: reklamacijaNotice(regime),
    noFeeAttested: false,
    noFeeAttestedAt: null,
    noFeeNotice: regime === "new" ? REKLAMACIJA_NO_FEE_NOTICE : null,
  };
  recomputeMockDeadlines(view);
  return view;
}

// A deliberately simplified stand-in for `compute_deadlines`: the 8-day answer
// clock never pauses; the resolution clock pauses on `consumer_received_answer`
// and restarts to a fresh span on `consumer_responded`; a granted extension
// takes the consented date; `resolved` stops the clock. UI-fidelity only.
function recomputeMockDeadlines(view: ReklamacijaView): void {
  const span = view.robaKind === "opsta" ? 15 : 30;
  const answerDue = addDays(view.filedAt, 8);
  const received = view.events.find(
    (event) => event.eventType === "consumer_received_answer",
  );
  const responded = view.events.find(
    (event) => event.eventType === "consumer_responded",
  );
  const extension = view.events.find(
    (event) => event.eventType === "extension_granted",
  );
  const resolved = view.events.some((event) => event.eventType === "resolved");
  const answered = view.events.some(
    (event) => event.eventType === "answer_given",
  );

  let clock = "running";
  let resolutionDue: string | null = addDays(view.filedAt, span);
  let consumerWindowDue: string | null = null;

  if (received) {
    consumerWindowDue = addDays(received.eventDate, 3);
  }
  if (received && !responded) {
    clock = dateGt(now, consumerWindowDue as string) ? "impasse" : "paused";
    resolutionDue = null;
  } else if (received && responded) {
    // Match the Rust engine: OLD restarts to a fresh span from the response;
    // NEW resumes, preserving the base + elapsed suspension.
    resolutionDue =
      view.regime === "old"
        ? addDays(responded.eventDate, span)
        : addDays(
            addDays(view.filedAt, span),
            daysBetween(received.eventDate, responded.eventDate),
          );
  }
  if (extension?.detailJson && resolutionDue != null) {
    // Tighter-date tiebreak, as the engine does.
    const extDeadline = JSON.parse(extension.detailJson).newDeadline as string;
    resolutionDue = dateGt(resolutionDue, extDeadline) ? extDeadline : resolutionDue;
  }
  if (resolved) {
    clock = "resolved";
    resolutionDue = null;
    consumerWindowDue = null;
  }

  view.deadlines = {
    answerDue,
    resolutionDue,
    clock,
    consumerWindowDue,
    answerOverdue: !resolved && !answered && dateGt(now, answerDue),
    resolutionOverdue: resolutionDue != null && dateGt(now, resolutionDue),
    oneExtensionUsed: extension != null,
  };
}

function addDays(iso: string, days: number): string {
  const date = new Date(iso);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

function dateGt(a: string, b: string): boolean {
  return new Date(a).getTime() > new Date(b).getTime();
}

function daysBetween(from: string, to: string): number {
  const ms = new Date(to).getTime() - new Date(from).getTime();
  return Math.round(ms / 86_400_000);
}

function mockBarcodeLookup(barcode: string): ProductLookupSuggestion | null {
  if (barcode !== "4008400320328") {
    return null;
  }

  return {
    barcode,
    source: {
      provider: "open_food_facts",
      label: "Open Food Facts",
      barcode,
      fetchedAt: now,
      acceptedFields: ["name", "brand", "packageSize"],
    },
    fields: {
      name: "Kinder Bueno",
      brand: "Kinder",
      packageSize: "43 g",
      imageUrl:
        "https://images.openfoodfacts.org/images/products/400/840/032/0328/front_en.3.400.jpg",
      categoryName: "Chocolate bars",
    },
  };
}

function createBackupJob(
  backupType: BackupJob["backupType"],
  path: string,
): BackupJob {
  return {
    id: Date.now(),
    backupType,
    path,
    status: "completed",
    errorMessage: null,
    fileSizeBytes: 131072,
    createdAt: now,
    completedAt: now,
  };
}

function parseMockCsv(csvText: string) {
  const [headerLine = "", ...dataLines] = csvText
    .trim()
    .split(/\r?\n/)
    .filter(Boolean);
  const delimiter = headerLine.includes(";") ? ";" : ",";
  const headers = headerLine.split(delimiter).map((value) => value.trim());
  const rows = dataLines.map((line) =>
    line.split(delimiter).map((value) => value.trim()),
  );

  return { delimiter, headers, rows };
}

function rowToValues(headers: string[], row: string[]) {
  return Object.fromEntries(
    headers.map((header, index) => [header, row[index] ?? ""]),
  );
}

function filterProducts(
  products: ProductSummary[],
  query: ProductListQuery = {},
) {
  const normalized = query.search?.trim().toLowerCase() ?? "";

  return products.filter((product) => {
    const matchesSearch =
      !normalized ||
      [product.name, product.sku, product.barcode ?? ""].some((value) =>
        value.toLowerCase().includes(normalized),
      );
    const matchesCategory =
      query.categoryId === undefined || product.categoryId === query.categoryId;
    const matchesActive =
      query.active === undefined || product.active === query.active;
    const matchesLowStock =
      !query.lowStock ||
      product.currentStockMilli <= product.minimumStockMilli;
    const matchesMissingBarcode = !query.missingBarcode || !product.barcode;
    const matchesTaxRate =
      query.taxRateId === undefined || product.taxRateId === query.taxRateId;

    return (
      matchesSearch &&
      matchesCategory &&
      matchesActive &&
      matchesLowStock &&
      matchesMissingBarcode &&
      matchesTaxRate
    );
  });
}

function rejectDuplicateProduct(
  products: ProductSummary[],
  sku: string,
  barcode: string | null,
  currentProductId?: number,
) {
  if (
    products.some(
      (product) => product.id !== currentProductId && product.sku === sku,
    )
  ) {
    throw { code: "duplicate_sku", message: "SKU/šifra već postoji." };
  }

  if (
    barcode &&
    products.some(
      (product) =>
        product.id !== currentProductId && product.barcode === barcode,
    )
  ) {
    throw { code: "duplicate_barcode", message: "Barcode već postoji." };
  }
}

function findTaxRate(taxRates: TaxRateSummary[], taxRateId: number) {
  const taxRate = taxRates.find((item) => item.id === taxRateId && item.active);

  if (!taxRate) {
    throw { code: "validation_error", message: "Izaberite važeću PDV stopu." };
  }

  return taxRate;
}

function toStockItem(product: ProductSummary): StockListItem {
  return {
    productId: product.id,
    productName: product.name,
    sku: product.sku,
    barcode: product.barcode,
    categoryId: product.categoryId,
    categoryName: product.categoryName,
    unitOfMeasure: product.unitOfMeasure,
    currentQuantityMilli: product.currentStockMilli,
    minimumStockMilli: product.minimumStockMilli,
    lowStock: product.currentStockMilli <= product.minimumStockMilli,
    salePriceMinor: product.salePriceMinor,
    lastMovementAt: ledgerLatest(product.id),
  };
}

function ledgerLatest(_productId: number): string | null {
  return null;
}

/** Verbatim `cash_deposit.rs::report_footer` — one wording, so the demo cannot
 *  soften a label the real report hardened. */
const CASH_DEPOSIT_FOOTER =
  "Zbir po danu prometa je konvencija ove aplikacije, a ne zakonska kategorija: " +
  "rok teče od prijema gotovine, a ni Zakon 68/2015 ni Pravilnik 77/2011 ne " +
  "poznaju dnevni izveštaj. Iz osnovice je izuzeta samo ona gotovina podignuta " +
  "sa tekućeg računa radnje za koju je zabeleženo da je isplaćena u skladu sa " +
  "Pravilnikom 77/2011 čl. 2 st. 2 (uz originalnu dokumentaciju podnetu banci " +
  "na uvid i overu) ili čl. 2 st. 3 (dnevni limit od 150.000 dinara bez " +
  "dokumentacije). Podizanja bez te potvrde ostaju u osnovici i imaju rok za " +
  "polog. Izuzeće je olakšica na nivou podzakonskog akta — sam zakon („po bilo " +
  "kom osnovu“) i kazna iz čl. 7 ne sadrže nijedan izuzetak. Izveštaj je " +
  "informativan: nadzor vrši Poreska uprava, a rok ne blokira prodaju, " +
  "zatvaranje smene ni fiskalizaciju.";

/**
 * The demo double for `cash_deposit_report`.
 *
 * The seven-working-day arithmetic is **not** reimplemented here: a second copy
 * of a statutory count would drift from the one in
 * `src-tauri/src/cash_deposit.rs` that decides the real deadline. The demo
 * buckets carry fixed deadlines, and the only thing that moves is how much of
 * them a polog recorded in this session has discharged — oldest bucket first,
 * the way `build_buckets` draws them down.
 *
 * `penalty` is `null` for the same reason it is null in the AML double: every
 * fine figure lives in `legal.rs` and nowhere else.
 */
function buildMockCashDepositReport(
  asOf: string,
  depositedMinor: number,
  calendar: CashDepositCalendar,
  _profile: ShopProfile,
): CashDepositReport {
  const demoInflows = [
    { tradingDate: "2026-06-08", subjectMinor: 180_000, dueOn: "2026-06-16" },
    { tradingDate: "2026-06-17", subjectMinor: 120_000, dueOn: "2026-06-25" },
  ];

  let remaining = Math.max(depositedMinor, 0);
  const buckets: DepositBucket[] = demoInflows.map((inflow) => {
    const applied = Math.min(remaining, inflow.subjectMinor);
    remaining -= applied;
    const outstandingMinor = inflow.subjectMinor - applied;

    return {
      tradingDate: inflow.tradingDate,
      subjectMinor: inflow.subjectMinor,
      depositedMinor: applied,
      outstandingMinor,
      dueOn: inflow.dueOn,
      // The polog is still lawful on the deadline day itself.
      isOverdue: outstandingMinor > 0 && inflow.dueOn < asOf,
    };
  });

  return {
    asOf,
    buckets,
    outstandingMinor: buckets.reduce(
      (sum, bucket) => sum + bucket.outstandingMinor,
      0,
    ),
    overdueMinor: buckets
      .filter((bucket) => bucket.isOverdue)
      .reduce((sum, bucket) => sum + bucket.outstandingMinor, 0),
    excludedFloatMinor: 0,
    saturdayIsWorking: calendar.saturdayIsWorking,
    calendarHorizonYear: calendar.horizonYear,
    beyondSeededCalendar: false,
    notice: {
      summary:
        "Dinare primljene u gotovom po bilo kom osnovu treba uplatiti na " +
        "tekući račun u roku od sedam radnih dana.",
      penalty: null,
      citation:
        "Zakon o obavljanju plaćanja pravnih lica, preduzetnika i fizičkih " +
        "lica koja ne obavljaju delatnost (Sl. glasnik RS, br. 68/2015), " +
        "čl. 3 st. 1. Nadzor: Poreska uprava (čl. 6).",
      isLegalDuty: true,
    },
    footer: CASH_DEPOSIT_FOOTER,
  };
}

/**
 * The prodajno mesto the seeded archive is keyed on. `commands::cenovnik::
 * prodajno_mesto` derives it from the shop's address and then freezes it, so
 * this matches `companySettings.address` above.
 */
const MOCK_OUTLET = "Bulevar 1, Beograd";

/** `src-tauri/src/cenovnik.rs::COLUMNS`, in order. */
const CENOVNIK_COLUMNS = [
  "sifra",
  "barkod",
  "naziv",
  "jedinica_mere",
  "prodajna_cena",
  "jedinicna_cena",
  "jedinica_za_jedinicnu_cenu",
  "datum_azuriranja",
];

interface MockCenovnikRow {
  sifra: string;
  barkod: string | null;
  naziv: string;
  jedinicaMere: string;
  prodajnaCenaMinor: number;
  /** `null` when the article states no jedinična cena — a visible gap in the
   *  published file, never a guessed figure. */
  jedinicnaCenaMinor: number | null;
  jedinicaZaJedinicnuCenu: string | null;
}

/**
 * The published file in its real shape — BOM, `;` separator, CRLF, the barcode
 * quoted as text and prices rendered with two decimals from integer para at the
 * boundary only. A double that published a tidier file than
 * `src-tauri/src/cenovnik.rs::render_csv` would let a reader-side defect through.
 */
function renderMockCenovnik(rows: MockCenovnikRow[], generatedAt: string): string {
  const day = generatedAt.slice(0, 10).split("-").reverse().join("-");
  const line = (fields: string[]) => `${fields.join(";")}\r\n`;

  return (
    "﻿" +
    line(CENOVNIK_COLUMNS) +
    rows
      .map((row) =>
        line([
          row.sifra,
          row.barkod ? `"${row.barkod}"` : "",
          row.naziv,
          row.jedinicaMere,
          rsdFromPara(row.prodajnaCenaMinor),
          row.jedinicnaCenaMinor === null
            ? ""
            : rsdFromPara(row.jedinicnaCenaMinor),
          row.jedinicaZaJedinicnuCenu ?? "",
          day,
        ]),
      )
      .join("")
  );
}

/** Two decimals from integer para, at the boundary only — never a float. */
function rsdFromPara(minor: number): string {
  const sign = minor < 0 ? "-" : "";
  const absolute = Math.abs(minor);

  return `${sign}${Math.floor(absolute / 100)}.${`${absolute % 100}`.padStart(2, "0")}`;
}

/**
 * The jedinična cena in para per one unit of the measure, mirroring
 * `CenovnikRow::jedinicna_cena_minor`: no measure means no unit price at all,
 * and no package content means one selling unit IS one of the measure, so the
 * two prices coincide. Rounded to the nearest para — truncation would publish
 * 6,66 where the true figure is 6,67.
 */
function unitPriceMinor(product: ProductSummary): number | null {
  if (!product.jedinicnaCenaJedinica) {
    return null;
  }
  const sadrzajMilli = product.jedinicnaCenaSadrzajMilli;
  if (sadrzajMilli == null) {
    return product.salePriceMinor;
  }
  if (sadrzajMilli <= 0) {
    return null;
  }

  return Math.round((product.salePriceMinor * 1000) / sadrzajMilli);
}

/**
 * A stand-in for the archive's `content_hash`. **Not SHA-256** — the real digest
 * is computed in `src-tauri/src/cenovnik.rs::content_hash` and nothing on this
 * side reproduces it. All a double owes is what the field is used for: a stable
 * 64-hex handle that changes whenever the body does, so a surface comparing two
 * publications cannot pass by accident.
 */
function mockContentHash(body: string): string {
  let hash = 0x811c9dc5;
  for (const character of body) {
    hash = Math.imul(hash ^ character.codePointAt(0)!, 0x01000193) >>> 0;
  }

  return Array.from({ length: 8 }, (_, index) =>
    ((hash + index * 0x9e3779b1) >>> 0).toString(16).padStart(8, "0"),
  ).join("");
}

/**
 * One archived cenovnik for the seeded catalog. `mlekoPriceMinor` is what makes
 * the older publication differ from the newer one — čl. 6 st. 5 exists so the
 * two can be compared, and an archive whose files were identical would
 * demonstrate nothing.
 */
function mockSnapshot(
  id: number,
  generatedAt: string,
  mlekoPriceMinor: number,
): { snapshot: CenovnikSnapshot; body: string } {
  const body = renderMockCenovnik(
    [
      {
        sifra: "MLEKO-1L",
        barkod: "8600000000010",
        naziv: "Mleko 1 l",
        jedinicaMere: "kom",
        prodajnaCenaMinor: mlekoPriceMinor,
        jedinicnaCenaMinor: mlekoPriceMinor,
        jedinicaZaJedinicnuCenu: "l",
      } satisfies MockCenovnikRow,
      {
        sifra: "KAFA-200",
        barkod: "8600000000027",
        naziv: "Kafa 200 g",
        jedinicaMere: "kom",
        prodajnaCenaMinor: 50000,
        jedinicnaCenaMinor: 250000,
        jedinicaZaJedinicnuCenu: "kg",
      },
    ],
    generatedAt,
  );

  return {
    snapshot: {
      id,
      prodajnoMesto: MOCK_OUTLET,
      generatedAt,
      rowCount: 2,
      contentHash: mockContentHash(body),
      // Nothing has accepted the file: the seeded shop has configured no target,
      // which is the state a fresh install is actually in.
      publishedAt: null,
      publishedTarget: null,
      // Derived on read, exactly as the backend derives it — never stored.
      current: false,
    },
    body,
  };
}

/**
 * The prodajna cena the file publishes for each šifra — the inverse of
 * `renderMockCenovnik`, and the only source the till guard may compare against.
 * Columns are located by NAME out of the file's own header, as
 * `cenovnik.rs::published_prices` does it, so a reordered header cannot silently
 * shift the price column.
 */
function publishedPrices(body: string): Map<string, number> {
  const lines = body.replace(/^﻿/, "").split(/\r?\n/);
  const header = (lines.shift() ?? "").split(";");
  const sifraAt = header.indexOf("sifra");
  const cenaAt = header.indexOf("prodajna_cena");
  const prices = new Map<string, number>();

  if (sifraAt < 0 || cenaAt < 0) {
    return prices;
  }

  for (const line of lines) {
    const fields = line.split(";");
    const sifra = fields[sifraAt];
    const cena = fields[cenaAt];
    if (!sifra || !/^-?\d+\.\d{2}$/.test(cena ?? "")) {
      continue;
    }
    prices.set(sifra, Math.round(Number(cena) * 100));
  }

  return prices;
}

/**
 * Mirrors `src-tauri/src/aml.rs::assess_cash_payment` exactly — inclusive
 * `>=` on the cap ("10.000 evra **ili više**") and the 80% soft line. The mock
 * must not warn where the real backend would stay silent, or the reverse.
 *
 * `penalty` is deliberately `null` here whatever the legal form: every
 * statutory fine figure lives in `src-tauri/src/legal.rs` and nowhere else, so
 * a second copy in this double could silently drift out of tier — which is
 * exactly the defect the 31.07.2026 penalty-tier sweep corrected. A `null`
 * penalty is the one answer that can never be the wrong tier, and the till
 * already renders it as "set your legal form in Podešavanja → Profil".
 */
function assessCashPayment(
  cashMinor: number,
  rate: EurRate | null,
  _profile: ShopProfile,
): AmlAssessment {
  const notice: LegalNotice = {
    summary:
      "Zabranjeno je primiti gotovinu u iznosu od 10.000 evra ili više u " +
      "dinarskoj protivvrednosti. Iznos se mora uplatiti na tekući račun.",
    penalty: null,
    citation:
      "Zakon o sprečavanju pranja novca i finansiranja terorizma, čl. 46 st. 1. " +
      "Nadzor: tržišna inspekcija (čl. 110 st. 6).",
    isLegalDuty: true,
  };

  const fallbackThresholdMinor = AML_CAP_EUR * AML_FALLBACK_RATE_MINOR;

  if (!rate) {
    return {
      cashMinor,
      thresholdMinor: 0,
      fallbackThresholdMinor,
      breached: false,
      nearThreshold: false,
      rateUnavailable: true,
      rate: null,
      notice,
    };
  }

  const thresholdMinor = AML_CAP_EUR * rate.rateMinor;
  const softMinor = Math.floor((thresholdMinor * AML_SOFT_RATIO_PERCENT) / 100);

  return {
    cashMinor,
    thresholdMinor,
    fallbackThresholdMinor,
    breached: cashMinor >= thresholdMinor,
    nearThreshold: cashMinor >= softMinor && cashMinor < thresholdMinor,
    rateUnavailable: false,
    rate,
    notice,
  };
}

function createSalePreview(
  products: ProductSummary[],
  request: SaleDraftRequest,
): SalePreview {
  const items = request.items.map((item) => {
    const product = products.find((candidate) => candidate.id === item.productId);

    if (!product) {
      throw { code: "not_found", message: "Artikal nije pronađen." };
    }

    const grossMinor = Math.round(
      (product.salePriceMinor * item.quantityMilli) / 1000,
    );
    const discountMinor = discountAmount(grossMinor, item.discount);
    const totalMinor = grossMinor - discountMinor;

    return {
      productId: product.id,
      productName: product.name,
      productSku: product.sku,
      productBarcode: product.barcode,
      quantityMilli: item.quantityMilli,
      quantityLabel: `${item.quantityMilli / 1000} ${product.unitOfMeasure}`,
      unitPriceMinor: product.salePriceMinor,
      discountMinor,
      taxRateBasisPoints: product.taxRateBasisPoints,
      taxMinor: includedTax(totalMinor, product.taxRateBasisPoints),
      totalMinor,
    };
  });
  const subtotalMinor = items.reduce(
    (sum, item) =>
      sum + Math.round((item.unitPriceMinor * item.quantityMilli) / 1000),
    0,
  );
  const itemDiscountMinor = items.reduce(
    (sum, item) => sum + item.discountMinor,
    0,
  );
  const receiptDiscountMinor = discountAmount(
    items.reduce((sum, item) => sum + item.totalMinor, 0),
    request.receiptDiscount,
  );
  const totalMinor =
    items.reduce((sum, item) => sum + item.totalMinor, 0) - receiptDiscountMinor;

  return {
    items,
    subtotalMinor,
    discountMinor: itemDiscountMinor + receiptDiscountMinor,
    taxMinor: includedTax(totalMinor, 2000),
    totalMinor,
  };
}

function discountAmount(
  totalMinor: number,
  discount: SaleDraftRequest["receiptDiscount"],
) {
  if (!discount) {
    return 0;
  }

  if (discount.type === "amount") {
    return discount.amountMinor;
  }

  return Math.round((totalMinor * discount.basisPoints) / 10000);
}

function includedTax(totalMinor: number, basisPoints: number) {
  if (basisPoints === 0) {
    return 0;
  }

  return Math.round((totalMinor * basisPoints) / (10000 + basisPoints));
}

function createReceiptDetail(): ReceiptDetail {
  return {
    id: 1,
    receiptNumber: "R-2026-0001",
    createdAt: now,
    cashierName: "Mina Kasir",
    shiftId: 1,
    status: "completed",
    fiscalStatus: "not_fiscalized",
    documentType: "sale",
    totalMinor: 100000,
    linkedDocumentCount: 0,
    paymentMethods: ["cash"],
    subtotalMinor: 100000,
    discountMinor: 0,
    taxMinor: 16667,
    items: [
      {
        id: 1,
        productId: 2,
        productName: "Kafa 200 g",
        productSku: "KAFA-200",
        productBarcode: "8600000000027",
        quantityMilli: 2000,
        returnedQuantityMilli: 0,
        unitPriceMinor: 50000,
        discountMinor: 0,
        taxRateBasisPoints: 2000,
        taxMinor: 16667,
        totalMinor: 100000,
      },
    ],
    payments: [
      {
        id: 1,
        paymentMethod: "cash",
        amountMinor: 100000,
        createdAt: now,
      },
    ],
    linkedDocuments: [],
    canVoid: true,
    canReturn: true,
  };
}

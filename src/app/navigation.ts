import type { LucideIcon } from "lucide-react";
import {
  ArchiveIcon,
  BarChart3Icon,
  BookIcon,
  CalendarClockIcon,
  ClipboardListIcon,
  ClockIcon,
  FileSpreadsheetIcon,
  MessageSquareWarningIcon,
  PackageIcon,
  ReceiptTextIcon,
  SettingsIcon,
  ShieldCheckIcon,
  ShoppingCartIcon,
  TagIcon,
  UploadIcon,
} from "lucide-react";

export interface NavigationItem {
  id: string;
  label: string;
  icon: LucideIcon;
  adminOnly?: boolean;
}

export const navigationItems = [
  {
    id: "register",
    label: "Kasa",
    icon: ShoppingCartIcon,
  },
  {
    id: "products",
    label: "Artikli",
    icon: PackageIcon,
  },
  {
    id: "inventory",
    label: "Lager",
    icon: ArchiveIcon,
  },
  {
    id: "receipts",
    label: "Računi",
    icon: ReceiptTextIcon,
  },
  {
    id: "reklamacije",
    label: "Reklamacije",
    icon: MessageSquareWarningIcon,
    adminOnly: true,
  },
  {
    id: "kep",
    label: "KEP",
    icon: BookIcon,
    adminOnly: true,
  },
  // Admin-only, and every `popis_*` command behind it is admin-gated backend
  // side too. The popis releases the knjigovodstveno stanje to the commission
  // (PoP čl. 8 st. 5), records who signed for the counted state, and books the
  // result — none of that is a kasir's to do, and the komisija rows name people.
  {
    id: "popis",
    label: "Popis",
    icon: ClipboardListIcon,
    adminOnly: true,
  },
  {
    id: "worktime",
    label: "Radno vreme",
    icon: ClockIcon,
    adminOnly: true,
  },
  // Deliberately NOT adminOnly: ZoR čl. 83 st. 1 and ZZPL čl. 26 are the
  // employee's own rights, so the surface that discharges them must be
  // reachable by the employee. `worktime_my_hours` is session-gated and
  // returns own rows only, so there is nothing here an admin gate would
  // protect.
  {
    id: "moji-sati",
    label: "Moji sati",
    icon: CalendarClockIcon,
  },
  {
    id: "campaigns",
    label: "Kampanje",
    icon: TagIcon,
    adminOnly: true,
  },
  {
    id: "reports",
    label: "Izveštaji",
    icon: BarChart3Icon,
    adminOnly: true,
  },
  // Admin-only, and every command behind it is admin-gated backend-side as
  // well. ZZPL čl. 48 st. 4 puts the evidencija pristupa in the rukovalac's
  // hands, the breach file very often describes a colleague in a shop this
  // size, and the čl. 46 nalog is the rukovalac's to issue — none of the three
  // is a kasir's to read, let alone to sign.
  {
    id: "privatnost",
    label: "Privatnost",
    icon: ShieldCheckIcon,
    adminOnly: true,
  },
  {
    id: "import",
    label: "Import",
    icon: UploadIcon,
  },
  {
    id: "settings",
    label: "Podešavanja",
    icon: SettingsIcon,
    adminOnly: true,
  },
] as const satisfies readonly NavigationItem[];

export type NavigationItemId = (typeof navigationItems)[number]["id"];

export interface FoundationCard {
  title: string;
  description: string;
  icon: LucideIcon;
}

export const foundationCards = [
  {
    title: "Lokalna kasa",
    description: "Prodaja, artikli i računi ostaju vezani za ovu kasu.",
    icon: FileSpreadsheetIcon,
  },
  {
    title: "Jedna radnja",
    description: "Rad je pripremljen za jedan objekat i jednu kasu.",
    icon: PackageIcon,
  },
  {
    title: "Dnevni rad",
    description: "Osnovni statusi za prodaju, lager i izveštaje su spremni.",
    icon: SettingsIcon,
  },
] as const satisfies readonly FoundationCard[];

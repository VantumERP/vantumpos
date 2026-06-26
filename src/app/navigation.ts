import type { LucideIcon } from "lucide-react";
import {
  ArchiveIcon,
  BarChart3Icon,
  FileSpreadsheetIcon,
  PackageIcon,
  ReceiptTextIcon,
  SettingsIcon,
  ShoppingCartIcon,
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
    label: "Racuni",
    icon: ReceiptTextIcon,
  },
  {
    id: "reports",
    label: "Izvestaji",
    icon: BarChart3Icon,
  },
  {
    id: "import",
    label: "Import",
    icon: UploadIcon,
  },
  {
    id: "settings",
    label: "Podesavanja",
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
    description: "Prodaja, artikli i racuni ostaju vezani za ovu kasu.",
    icon: FileSpreadsheetIcon,
  },
  {
    title: "Jedna radnja",
    description: "Rad je pripremljen za jedan objekat i jednu kasu.",
    icon: PackageIcon,
  },
  {
    title: "Dnevni rad",
    description: "Osnovni statusi za prodaju, lager i izvestaje su spremni.",
    icon: SettingsIcon,
  },
] as const satisfies readonly FoundationCard[];

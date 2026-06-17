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
    description: "Prodajni tok ce koristiti lokalni SQLite backend.",
    icon: FileSpreadsheetIcon,
  },
  {
    title: "Jedna radnja",
    description: "MVP je za jedan objekat, jednu kasu i jednu bazu.",
    icon: PackageIcon,
  },
  {
    title: "Adapter arhitektura",
    description: "UI komunicira preko domain service ugovora.",
    icon: SettingsIcon,
  },
] as const satisfies readonly FoundationCard[];

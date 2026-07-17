import type {
  CampaignDisplayMode,
  CampaignType,
  RasprodajaGround,
} from "@/services/types";

/**
 * Copy the campaigns screen and the wizard must render identically. Two copies
 * of a legally-loaded string are two strings that can drift: the ground labels
 * below are the čl. 37 st. 6 closed list, and a shop's paperwork has to name
 * the same ground the campaign was saved under.
 */

export const typeLabels: Record<CampaignType, string> = {
  rasprodaja: "Rasprodaja",
  sezonsko_snizenje: "Sezonsko sniženje",
  akcijska_prodaja: "Akcijska prodaja",
  promotivna_prodaja: "Promotivna prodaja",
};

export const displayModeLabels: Record<CampaignDisplayMode, string> = {
  two_prices: "Snižena i prethodna cena",
  percentage: "Procenat sniženja",
};

// Verbatim from čl. 37 st. 6 — a closed list of three grounds.
export const groundLabels: Record<RasprodajaGround, string> = {
  prestanak_poslovanja: "Prestanak poslovanja trgovca",
  prestanak_u_objektu: "Prestanak poslovanja u određenim objektima",
  prestanak_prodaje_robe: "Prestanak prodaje određene robe",
};

/** čl. 36 st. 2 t. 3: the only lawful stand-in for a rasprodaja end date. */
export const STOCK_LABEL = "dok traju zalihe";

export const ANCHOR_TRUNCATED_NOTE =
  "Evidencija ne pokriva ceo prozor — prethodna cena je skraćena.";

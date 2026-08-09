import { useState } from "react";
import type { ReactNode } from "react";

import { AuditLogPanel } from "./AuditLogPanel";
import { BreachLogPanel } from "./BreachLogPanel";
import { Cl47RegisterPanel } from "./Cl47RegisterPanel";
import { SupportApprovalPanel } from "./SupportApprovalPanel";
import type { PosServices } from "@/services/ports";
import type { UserAccount } from "@/services/types";

/**
 * Privatnost — the four ZZPL surfaces, in the order their legal weight runs.
 *
 * **Daljinska podrška first, deliberately.** Čl. 46 is the penalised article in
 * this module (čl. 95 st. 1 t. 23) and the nalog is the only thing here that is
 * a legal duty in the strict sense. The evidencija pristupa beside it is a
 * prudential control — čl. 50 appears nowhere in čl. 95 — and putting the log
 * first would invite the reader to take the module's headline surface for its
 * headline obligation.
 *
 * The four tabs are the four stores v18 carries, and nothing about a surface
 * that is legally optional is presented as a duty. Each panel loads its own
 * data: the module never fetches on all four's behalf, so opening Privatnost
 * does not read the whole ZZPL corner of the database.
 */
type PrivacyTab = "podrska" | "pristup" | "povrede" | "radnje";

const TABS: { id: PrivacyTab; label: string }[] = [
  { id: "podrska", label: "Daljinska podrška" },
  { id: "pristup", label: "Evidencija pristupa" },
  { id: "povrede", label: "Povrede podataka" },
  { id: "radnje", label: "Radnje obrade" },
];

export function PrivacyModule({
  services,
  currentUser,
}: {
  services: PosServices;
  /**
   * Passed through to the čl. 46 panel for the one control that needs a role:
   * SW-14 req. 28's unmask. Optional and absent-means-closed, like every other
   * privacy gate in this app.
   */
  currentUser?: UserAccount;
}) {
  const [activeTab, setActiveTab] = useState<PrivacyTab>("podrska");

  const panels: Record<PrivacyTab, ReactNode> = {
    podrska: (
      <SupportApprovalPanel services={services} currentUser={currentUser} />
    ),
    pristup: <AuditLogPanel services={services} />,
    povrede: <BreachLogPanel services={services} />,
    radnje: <Cl47RegisterPanel services={services} />,
  };

  return (
    <div className="flex flex-col gap-4">
      <div
        role="tablist"
        aria-label="Privatnost"
        className="inline-flex w-fit items-center justify-center gap-1 rounded-lg bg-muted p-1 text-muted-foreground"
      >
        {TABS.map((tab) => (
          <PrivacyTabButton
            key={tab.id}
            active={activeTab === tab.id}
            onSelect={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </PrivacyTabButton>
        ))}
      </div>

      {panels[activeTab]}
    </div>
  );
}

function PrivacyTabButton({
  active,
  children,
  onSelect,
}: {
  active: boolean;
  children: ReactNode;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      className={[
        "inline-flex h-7 items-center justify-center rounded-md px-3 text-xs font-medium transition-colors",
        active
          ? "bg-secondary text-secondary-foreground"
          : "text-muted-foreground hover:bg-background hover:text-foreground",
      ].join(" ")}
      onClick={onSelect}
    >
      {children}
    </button>
  );
}

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { PrivacyModule } from "./PrivacyModule";
import { UserDialog, UsersScreen } from "@/app/AppShell";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices, UsersService } from "@/services/ports";
import type {
  EmployeeProfile,
  SupportSession,
  UserAccount,
} from "@/services/types";

const admin: UserAccount = {
  id: 1,
  username: "admin",
  displayName: "Administrator",
  role: "admin",
  active: true,
  createdAt: "2026-01-05T08:00:00Z",
  updatedAt: "2026-01-05T08:00:00Z",
  lastLoginAt: null,
};

const kasirka: UserAccount = {
  ...admin,
  id: 7,
  username: "jelena",
  displayName: "Jelena Đurić",
  role: "cashier",
};

const prazanProfil: EmployeeProfile = {
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

/** Every affordance that would end an employee's record rather than an account. */
const BRISANJE = /obriši|izbriši|brisanje|ukloni|uklanjanje|delete/i;

/** A čl. 46 nalog that is live, so the req. 28 unmask block is on screen. */
const zivNalog: SupportSession = {
  id: 1,
  grantedBy: 1,
  grantedByName: "Administrator",
  grantedAt: "2026-08-01T09:00:00Z",
  scope: "Pregled greške na štampi fiskalnog isečka",
  expiresAt: "2026-08-01T10:00:00Z",
  startedAt: null,
  endedAt: null,
  revokedAt: null,
  odsustvoOtkrivenoAt: null,
};

function servicesWithNalog(): PosServices {
  const posServices = createMockServices();
  vi.spyOn(posServices.privacy, "activeSupportSession").mockResolvedValue(
    zivNalog,
  );
  return posServices;
}

describe("navigation", () => {
  it("carries Privatnost as an admin-only surface", () => {
    const item = navigationItems.find((candidate) => candidate.id === "privatnost");

    expect(item).toMatchObject({ label: "Privatnost", adminOnly: true });
  });
});

describe("PrivacyModule", () => {
  it("opens on the čl. 46 nalog and reaches all four surfaces", async () => {
    const posServices = createMockServices();
    const user = userEvent.setup();

    render(<PrivacyModule services={posServices} />);

    // Čl. 46 is the one penalised article in this module, so the nalog is what
    // the module opens on — never the prudential log.
    expect(
      await screen.findByRole("tab", { name: /daljinska podrška/i, selected: true }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: /evidencija pristupa/i }));
    expect(
      await screen.findByText(/nije propisan prekršaj/i),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: /povrede podataka/i }));
    expect(
      await screen.findByLabelText(/činjenice o povredi/i),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: /radnje obrade/i }));
    expect(await screen.findByText(/čl\. 47 st\. 7/i)).toBeInTheDocument();
  });

  /**
   * The one hop that fails silently: req. 28's unmask is `absent-means-closed`,
   * so a dropped `currentUser` does not throw — the button simply never renders
   * and the shop can never lift the mask, with the whole suite green. Every
   * `SupportApprovalPanel` test renders the panel directly with its own
   * `currentUser`, so none of them sees this prop being handed over at all.
   *
   * Both arms, because only the pair is a wiring test: the vlasnik arm fails if
   * `PrivacyModule` stops passing the user down, and the no-user arm fails if
   * the panel ever starts defaulting the gate open.
   */
  it("hands the vlasnik's role to the čl. 46 panel, and hands nothing when it has nothing", async () => {
    const { unmount } = render(
      <PrivacyModule services={servicesWithNalog()} currentUser={admin} />,
    );

    expect(
      await screen.findByRole("button", { name: /otkrij razlog odsustva/i }),
    ).toBeEnabled();
    unmount();

    render(<PrivacyModule services={servicesWithNalog()} />);

    // The same live nalog, so the block itself is on screen either way — what
    // differs is the control.
    await screen.findByText(/kategorija odsustva/i);
    expect(
      screen.queryByRole("button", { name: /otkrij razlog odsustva/i }),
    ).not.toBeInTheDocument();
  });

  /**
   * Req. 24 / §5 item 14. Deactivation must be structurally incapable of
   * deleting the personnel record, and the UI is half of „structurally“: an
   * affordance that does not exist cannot be clicked by mistake.
   */
  it("has no Delete employee control anywhere", async () => {
    const posServices = createMockServices();
    vi.spyOn(posServices.users, "listUsers").mockResolvedValue([admin, kasirka]);
    vi.spyOn(posServices.users, "getEmployeeProfile").mockResolvedValue(
      prazanProfil,
    );
    const user = userEvent.setup();

    const { unmount } = render(
      <UsersScreen services={posServices} currentUser={admin} />,
    );
    await screen.findByText("Jelena Đurić");

    expect(screen.queryAllByRole("button", { name: BRISANJE })).toHaveLength(0);
    // The one lifecycle action there is.
    expect(screen.getAllByRole("button", { name: /deaktiviraj/i }).length).toBeGreaterThan(0);

    // …and none inside the employee's own record either.
    await user.click(screen.getAllByRole("button", { name: /^uredi$/i })[0]);
    await screen.findByRole("dialog");
    expect(screen.queryAllByRole("button", { name: BRISANJE })).toHaveLength(0);
    unmount();

    render(
      <UserDialog
        open
        user={kasirka}
        profile={prazanProfil}
        onOpenChange={() => {}}
        onSave={async () => {}}
      />,
    );
    expect(screen.queryAllByRole("button", { name: BRISANJE })).toHaveLength(0);
  });

  /**
   * The same prohibition one layer down: a service method named „delete“ is an
   * affordance waiting for a button, so there is not one to wire up.
   */
  it("exposes no employee-deleting method on the users service", () => {
    const posServices = createMockServices();
    const users = posServices.users as UsersService & Record<string, unknown>;

    for (const method of ["deleteUser", "removeUser", "purgeUser", "deleteEmployee"]) {
      expect(users[method]).toBeUndefined();
    }
    expect(typeof posServices.users.deactivateUser).toBe("function");
  });

  it("states that deactivation never reduces the personnel record", async () => {
    const posServices = createMockServices();
    vi.spyOn(posServices.users, "listUsers").mockResolvedValue([kasirka]);

    render(<UsersScreen services={posServices} currentUser={admin} />);

    await waitFor(() => {
      expect(screen.getByText("Jelena Đurić")).toBeInTheDocument();
    });
    expect(
      screen.getByText(/deaktivacija ne briše evidenciju o zaposlenom/i),
    ).toBeInTheDocument();
  });
});

describe("privacy services", () => {
  it("wires every ZZPL command name through the local adapter", async () => {
    const posServices: PosServices = createMockServices();

    expect(typeof posServices.privacy.grantSupportAccess).toBe("function");
    expect(typeof posServices.privacy.searchAudit).toBe("function");
    expect(typeof posServices.privacy.listBreaches).toBe("function");
    expect(typeof posServices.privacy.listProcessingActivities).toBe("function");
  });
});

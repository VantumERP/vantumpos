import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CampaignsModule } from "./CampaignsModule";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { CampaignSummary, CampaignView } from "@/services/types";

const sezonskoDraft: CampaignSummary = {
  id: 1,
  campaignType: "sezonsko_snizenje",
  status: "draft",
  startsOn: "2026-07-05T00:00:00Z",
  endsOn: "2026-08-01T00:00:00Z",
  marketingLabel: "Letnje sniženje",
  itemCount: 2,
  overdue: false,
};

const rasprodajaActive: CampaignSummary = {
  id: 2,
  campaignType: "rasprodaja",
  status: "active",
  startsOn: "2026-06-01T00:00:00Z",
  endsOn: null,
  marketingLabel: null,
  itemCount: 1,
  overdue: false,
};

const akcijskaOverdue: CampaignSummary = {
  id: 3,
  campaignType: "akcijska_prodaja",
  status: "active",
  startsOn: "2026-05-01T00:00:00Z",
  endsOn: "2026-05-20T00:00:00Z",
  marketingLabel: "Nedelja kafe",
  itemCount: 1,
  overdue: true,
};

const promotivnaEnded: CampaignSummary = {
  id: 4,
  campaignType: "promotivna_prodaja",
  status: "ended",
  startsOn: "2026-03-01T00:00:00Z",
  endsOn: "2026-04-01T00:00:00Z",
  marketingLabel: null,
  itemCount: 1,
  overdue: false,
};

// Memo worked example (b): the anchor is 11.900 frozen at activation, and the
// campaign price has already been stepped down to 9.900.
const sezonskoView: CampaignView = {
  id: 1,
  campaignType: "sezonsko_snizenje",
  status: "draft",
  startsOn: "2026-07-05T00:00:00Z",
  endsOn: "2026-08-01T00:00:00Z",
  displayMode: "two_prices",
  headlinePercent: null,
  rasprodajaGround: null,
  specialConditions: "Ne važi uz druge popuste.",
  reducedUtilityReason: null,
  marketingLabel: "Letnje sniženje",
  seasonAttested: true,
  separationAttested: false,
  activatedAt: null,
  endedAt: null,
  overdue: false,
  items: [
    {
      productId: 10,
      productName: "Kafa 200 g",
      sku: "KAF-200",
      campaignPriceMinor: 990000,
      prethodnaCenaMinor: 1190000,
      anchorStatus: "computed",
      anchorWindowDays: 30,
      anchorTruncated: true,
      anchorReason: null,
      anchorJustification: null,
      futureRegularPriceMinor: null,
      preCampaignPriceMinor: 1290000,
    },
    {
      productId: 11,
      productName: "Jogurt 1 kg",
      sku: "JOG-1000",
      campaignPriceMinor: 12000,
      prethodnaCenaMinor: 15000,
      anchorStatus: "manual",
      anchorWindowDays: null,
      anchorTruncated: false,
      anchorReason: "Perishable",
      anchorJustification: "Lako kvarljiva roba — cena sa police 30.06.2026.",
      futureRegularPriceMinor: null,
      preCampaignPriceMinor: 16000,
    },
  ],
  warnings: [
    {
      code: "w4",
      message: "Evidencija cena ne pokriva ceo prozor — prethodna cena je skraćena.",
      productId: 10,
    },
    {
      code: "w1",
      message: "Prethodna cena je važila kraće od 3 dana u prozoru.",
      productId: null,
    },
  ],
};

const activeView: CampaignView = {
  ...sezonskoView,
  status: "active",
  activatedAt: "2026-07-05T08:00:00Z",
};

const promotivnaView: CampaignView = {
  id: 4,
  campaignType: "promotivna_prodaja",
  status: "active",
  startsOn: "2026-03-01T00:00:00Z",
  endsOn: "2026-04-01T00:00:00Z",
  displayMode: "two_prices",
  headlinePercent: null,
  rasprodajaGround: null,
  specialConditions: null,
  reducedUtilityReason: null,
  marketingLabel: null,
  seasonAttested: false,
  separationAttested: false,
  activatedAt: "2026-03-01T08:00:00Z",
  endedAt: null,
  overdue: false,
  items: [
    {
      productId: 20,
      productName: "Novi čaj",
      sku: "CAJ-NEW",
      campaignPriceMinor: 50000,
      prethodnaCenaMinor: null,
      anchorStatus: "none",
      anchorWindowDays: null,
      anchorTruncated: false,
      anchorReason: null,
      anchorJustification: null,
      futureRegularPriceMinor: 80000,
      preCampaignPriceMinor: null,
    },
  ],
  warnings: [],
};

function servicesWith(
  summaries: CampaignSummary[],
  view: CampaignView,
): PosServices {
  const services = createMockServices();
  vi.spyOn(services.campaigns, "listCampaigns").mockResolvedValue(summaries);
  vi.spyOn(services.campaigns, "getCampaign").mockResolvedValue(view);
  return services;
}

describe("navigation", () => {
  it("exposes campaigns as an admin-only item", () => {
    const item = navigationItems.find(
      (candidate) => candidate.id === "campaigns",
    );

    expect(item).toMatchObject({ label: "Kampanje", adminOnly: true });
  });
});

describe("CampaignsModule list", () => {
  it("renders type labels and status badges", async () => {
    const services = servicesWith(
      [sezonskoDraft, rasprodajaActive, akcijskaOverdue, promotivnaEnded],
      sezonskoView,
    );

    render(<CampaignsModule services={services} />);

    expect(await screen.findByText("Sezonsko sniženje")).toBeInTheDocument();
    expect(screen.getByText("Rasprodaja")).toBeInTheDocument();
    expect(screen.getByText("Akcijska prodaja")).toBeInTheDocument();
    expect(screen.getByText("Promotivna prodaja")).toBeInTheDocument();

    expect(screen.getByText("Nacrt")).toBeInTheDocument();
    expect(screen.getAllByText("Aktivna")).toHaveLength(2);
    expect(screen.getByText("Završena")).toBeInTheDocument();
  });

  it("renders the literal dok traju zalihe when a rasprodaja has no end date", async () => {
    const services = servicesWith([rasprodajaActive], sezonskoView);

    render(<CampaignsModule services={services} />);

    expect(await screen.findByText(/dok traju zalihe/)).toBeInTheDocument();
  });

  it("renders a destructive overdue badge that tells the user to return prices", async () => {
    const services = servicesWith([sezonskoDraft, akcijskaOverdue], sezonskoView);

    render(<CampaignsModule services={services} />);

    const badge = await screen.findByText("Isteklo — vratite cene");
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveAttribute("data-overdue", "true");

    // Only the overdue campaign carries it.
    expect(screen.getAllByText("Isteklo — vratite cene")).toHaveLength(1);
  });
});

describe("CampaignsModule detail", () => {
  it("shows the frozen anchor, its status, window and truncation marker", async () => {
    const user = userEvent.setup();
    const services = servicesWith([sezonskoDraft], sezonskoView);

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );

    const detail = await screen.findByRole("region", {
      name: "Detalji kampanje",
    });

    const anchored = within(detail).getByRole("row", { name: /Kafa 200 g/ });
    expect(within(anchored).getByText("11.900,00 RSD")).toBeInTheDocument();
    expect(within(anchored).getByText("Izračunata")).toBeInTheDocument();
    expect(within(anchored).getByText("30 dana")).toBeInTheDocument();
    expect(within(anchored).getByText(/skraćena/i)).toBeInTheDocument();

    const manual = within(detail).getByRole("row", { name: /Jogurt 1 kg/ });
    expect(within(manual).getByText("Ručno uneta")).toBeInTheDocument();

    expect(
      within(detail).getByText("Lako kvarljiva roba — cena sa police 30.06.2026."),
    ).toBeInTheDocument();
    expect(within(detail).getByText("Ne važi uz druge popuste.")).toBeInTheDocument();
  });

  it("renders warnings as advisory rows and never affirms legality", async () => {
    const user = userEvent.setup();
    const services = servicesWith([sezonskoDraft], sezonskoView);

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );

    const advisories = await screen.findByRole("list", { name: "Upozorenja" });
    expect(within(advisories).getAllByRole("listitem")).toHaveLength(2);
    expect(
      within(advisories).getByText(
        "Evidencija cena ne pokriva ceo prozor — prethodna cena je skraćena.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Upozorenja ne blokiraju rad sa kampanjom."),
    ).toBeInTheDocument();

    // Warnings are advisory, never blockers: the lifecycle action stays enabled.
    expect(screen.getByRole("button", { name: "Aktiviraj" })).toBeEnabled();

    // Nothing may affirm legality — čl. 38 st. 4 bites even when st. 3 is right.
    expect(screen.queryByText(/u redu/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/zakonit/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/usklađen/i)).not.toBeInTheDocument();
  });
});

describe("CampaignsModule lifecycle", () => {
  it("activates a draft through the service", async () => {
    const user = userEvent.setup();
    const services = servicesWith([sezonskoDraft], sezonskoView);
    const activate = vi
      .spyOn(services.campaigns, "activateCampaign")
      .mockResolvedValue(activeView);

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );
    await user.click(await screen.findByRole("button", { name: "Aktiviraj" }));

    await waitFor(() => expect(activate).toHaveBeenCalledWith(1));
    expect(await screen.findByRole("button", { name: "Završi kampanju…" })).toBeInTheDocument();
  });

  it("offers Otkaži only on drafts", async () => {
    const user = userEvent.setup();
    const draftServices = servicesWith([sezonskoDraft], sezonskoView);

    const draft = render(<CampaignsModule services={draftServices} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );
    expect(await screen.findByRole("button", { name: "Otkaži" })).toBeInTheDocument();
    draft.unmount();

    const activeServices = servicesWith(
      [{ ...sezonskoDraft, status: "active" }],
      activeView,
    );
    render(<CampaignsModule services={activeServices} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );

    expect(await screen.findByRole("button", { name: "Završi kampanju…" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Otkaži" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Aktiviraj" })).not.toBeInTheDocument();
  });

  it("cancels a draft through the service", async () => {
    const user = userEvent.setup();
    const services = servicesWith([sezonskoDraft], sezonskoView);
    const cancel = vi
      .spyOn(services.campaigns, "cancelCampaign")
      .mockResolvedValue({ ...sezonskoView, status: "cancelled" });

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );
    await user.click(await screen.findByRole("button", { name: "Otkaži" }));

    await waitFor(() => expect(cancel).toHaveBeenCalledWith(1));
  });

  it("steps an active campaign's item price down", async () => {
    const user = userEvent.setup();
    const services = servicesWith(
      [{ ...sezonskoDraft, status: "active" }],
      activeView,
    );
    const adjust = vi
      .spyOn(services.campaigns, "adjustItemPrice")
      .mockResolvedValue(activeView);

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Nova cena za Kafa 200 g" }),
    );

    const input = await screen.findByLabelText("Nova cena za Kafa 200 g");
    await user.clear(input);
    await user.type(input, "8.900,00");
    await user.click(screen.getByRole("button", { name: "Sačuvaj novu cenu" }));

    await waitFor(() => expect(adjust).toHaveBeenCalledWith(1, 10, 890000));
  });

  it("prefills the end dialog from the pre-campaign prices and sends overrides", async () => {
    const user = userEvent.setup();
    const services = servicesWith(
      [{ ...sezonskoDraft, status: "active" }],
      activeView,
    );
    const end = vi
      .spyOn(services.campaigns, "endCampaign")
      .mockResolvedValue({ ...activeView, status: "ended" });

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Završi kampanju…" }),
    );

    // Pre-campaign prices, not the campaign prices.
    expect(await screen.findByLabelText("Povratna cena za Kafa 200 g")).toHaveValue(
      "12.900,00",
    );
    expect(screen.getByLabelText("Povratna cena za Jogurt 1 kg")).toHaveValue(
      "160,00",
    );

    const edited = screen.getByLabelText("Povratna cena za Kafa 200 g");
    await user.clear(edited);
    await user.type(edited, "13.400,00");
    await user.click(screen.getByRole("button", { name: "Potvrdi završetak" }));

    await waitFor(() =>
      expect(end).toHaveBeenCalledWith(1, [
        { productId: 10, returnPriceMinor: 1340000 },
        { productId: 11, returnPriceMinor: 16000 },
      ]),
    );
  });

  it("prefills a promotivna end dialog from the future regular price", async () => {
    const user = userEvent.setup();
    const services = servicesWith(
      [{ ...promotivnaEnded, status: "active" }],
      promotivnaView,
    );

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #4" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Završi kampanju…" }),
    );

    expect(await screen.findByLabelText("Povratna cena za Novi čaj")).toHaveValue(
      "800,00",
    );
  });

  it("surfaces a command error instead of pretending the action succeeded", async () => {
    const user = userEvent.setup();
    const services = servicesWith([sezonskoDraft], sezonskoView);
    vi.spyOn(services.campaigns, "activateCampaign").mockRejectedValue({
      code: "business",
      message: "Sezonsko sniženje je već iskorišćeno dva puta u 2026.",
    });

    render(<CampaignsModule services={services} />);
    await user.click(
      await screen.findByRole("button", { name: "Detalji za kampanju #1" }),
    );
    await user.click(await screen.findByRole("button", { name: "Aktiviraj" }));

    expect(
      await screen.findByText("Sezonsko sniženje je već iskorišćeno dva puta u 2026."),
    ).toBeInTheDocument();
  });
});

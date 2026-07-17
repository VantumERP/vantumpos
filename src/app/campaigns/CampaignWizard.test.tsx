import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CampaignWizard } from "./CampaignWizard";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type {
  CampaignValidationReport,
  CampaignView,
  ProductSummary,
} from "@/services/types";

const kafa: ProductSummary = {
  id: 10,
  name: "Kafa 200 g",
  sku: "KAF-200",
  barcode: null,
  categoryId: null,
  categoryName: null,
  unitOfMeasure: "kom",
  salePriceMinor: 1290000,
  purchasePriceMinor: 900000,
  taxRateId: 1,
  taxRateBasisPoints: 2000,
  minimumStockMilli: 0,
  currentStockMilli: 10000,
  allowNegativeStock: false,
  active: true,
  perishable: false,
  perishableJustification: null,
  externalSource: null,
};

const emptyReport: CampaignValidationReport = {
  hard: [],
  warnings: [],
  anchors: [],
};

/** Memo worked example (b): the čl. 37 st. 3 anchor for Kafa is 11.900. */
const computedReport: CampaignValidationReport = {
  hard: [],
  warnings: [],
  anchors: [
    {
      productId: 10,
      anchorStatus: "computed",
      prethodnaCenaMinor: 1190000,
      anchorWindowDays: 30,
      anchorTruncated: false,
      anchorReason: null,
      anchorJustification: null,
    },
  ],
};

/** What the backend returns for a perishable / incomputable item: the anchor
 *  slot is `manual` and h13a fires until the human fills it in. */
const manualReport: CampaignValidationReport = {
  hard: [
    {
      code: "h13a",
      message: "Za ovaj artikal prethodna cena mora biti uneta ručno uz obrazloženje.",
      productId: 10,
    },
  ],
  warnings: [],
  anchors: [
    {
      productId: 10,
      anchorStatus: "manual",
      prethodnaCenaMinor: null,
      anchorWindowDays: null,
      anchorTruncated: false,
      anchorReason: "perishable",
      anchorJustification: null,
    },
  ],
};

function servicesWith(
  report: CampaignValidationReport,
  products: ProductSummary[] = [kafa],
): PosServices {
  const services = createMockServices();

  vi.spyOn(services.campaigns, "validateCampaign").mockResolvedValue(report);
  vi.spyOn(services.catalog, "searchProducts").mockResolvedValue({
    items: products,
    categories: [],
    taxRates: [],
    total: products.length,
  });

  return services;
}

function renderWizard(
  services: PosServices,
  campaign: CampaignView | null = null,
) {
  const onSaved = vi.fn();
  const onClose = vi.fn();

  render(
    <CampaignWizard
      services={services}
      campaign={campaign}
      onClose={onClose}
      onSaved={onSaved}
    />,
  );

  return { onSaved, onClose };
}

function setDate(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

async function addKafa(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "Dodaj Kafa 200 g" }));
}

describe("CampaignWizard type selection", () => {
  it("states the plain-language legal note under every campaign type", async () => {
    renderWizard(servicesWith(emptyReport));

    expect(
      await screen.findByText(
        "Najviše dva puta godišnje, počinje 25.12–10.01. ili 01–15.07, do 60 dana. Brojimo po datumu početka u kalendarskoj godini.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Do 31 dan. Do 3 dana može samo procenat umesto dve cene."),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Samo roba koja se prvi put uvodi u ponudu; do 60 dana; unosi se buduća redovna cena.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Samo uz zakonski osnov; roba se fizički izdvaja; bez prijema novih količina.",
      ),
    ).toBeInTheDocument();
  });

  it("asks for the rasprodaja ground and the separation attestation only for rasprodaja", async () => {
    const user = userEvent.setup();
    renderWizard(servicesWith(emptyReport));

    expect(
      screen.queryByLabelText("Zakonski osnov rasprodaje"),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "Rasprodaja" }));

    expect(await screen.findByLabelText("Zakonski osnov rasprodaje")).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", {
        name: "Potvrđujem da je roba na rasprodaji fizički izdvojena (čl. 37 st. 7).",
      }),
    ).toBeInTheDocument();

    // čl. 36 st. 2 t. 3: an open end date is lawful only here, and the field
    // has to say what leaving it empty means.
    expect(screen.getByLabelText("Datum isteka")).toHaveAttribute(
      "placeholder",
      "dok traju zalihe",
    );
    expect(screen.getByText(/dok traju zalihe/)).toBeInTheDocument();
  });
});

describe("CampaignWizard display mode", () => {
  it("disables the percentage-only option for a four-day akcijska prodaja", async () => {
    const user = userEvent.setup();
    renderWizard(servicesWith(emptyReport));

    await user.click(screen.getByRole("radio", { name: "Akcijska prodaja" }));
    setDate("Datum početka", "2026-07-01");
    setDate("Datum isteka", "2026-07-04");

    // 01.07–04.07 inclusive is 4 days — čl. 37 st. 11 allows a bare percentage
    // only up to 3.
    await waitFor(() =>
      expect(screen.getByRole("option", { name: "Procenat sniženja" })).toBeDisabled(),
    );

    setDate("Datum isteka", "2026-07-03");

    await waitFor(() =>
      expect(screen.getByRole("option", { name: "Procenat sniženja" })).toBeEnabled(),
    );
  });

  it("keeps the percentage option disabled for a three-day sezonsko sniženje", async () => {
    const user = userEvent.setup();
    renderWizard(servicesWith(emptyReport));

    await user.click(screen.getByRole("radio", { name: "Sezonsko sniženje" }));
    setDate("Datum početka", "2026-07-01");
    setDate("Datum isteka", "2026-07-03");

    await waitFor(() =>
      expect(screen.getByRole("option", { name: "Procenat sniženja" })).toBeDisabled(),
    );
  });
});

describe("CampaignWizard validation summary", () => {
  it("blocks saving on hard violations and renders their messages", async () => {
    renderWizard(
      servicesWith({
        ...emptyReport,
        hard: [
          {
            code: "h2",
            message:
              "Sezonsko sniženje mora početi u periodu 25.12–10.01. ili 01.07–15.07. (čl. 37 st. 8).",
            productId: null,
          },
          { code: "h14d", message: "Kampanja mora imati bar jedan artikal.", productId: null },
        ],
      }),
    );

    const blockers = await screen.findByRole("list", { name: "Prepreke" });
    expect(within(blockers).getAllByRole("listitem")).toHaveLength(2);
    expect(
      within(blockers).getByText(
        "Sezonsko sniženje mora početi u periodu 25.12–10.01. ili 01.07–15.07. (čl. 37 st. 8).",
      ),
    ).toBeInTheDocument();

    expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeDisabled();
  });

  it("lists warnings without blocking the save and says they do not block it", async () => {
    const user = userEvent.setup();
    renderWizard(
      servicesWith({
        ...computedReport,
        warnings: [
          {
            code: "w1",
            message: "Prethodna cena je važila kraće od 3 dana u prozoru.",
            productId: 10,
          },
        ],
      }),
    );

    await addKafa(user);
    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "9.900,00");

    const advisories = await screen.findByRole("list", { name: "Upozorenja" });
    expect(
      within(advisories).getByText("Prethodna cena je važila kraće od 3 dana u prozoru."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Upozorenja ne blokiraju čuvanje."),
    ).toBeInTheDocument();

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeEnabled(),
    );

    // Nothing may affirm legality — čl. 38 st. 4 bites even when st. 3 is right.
    expect(screen.queryByText(/u redu/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/zakonit/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/usklađen/i)).not.toBeInTheDocument();
  });

  it("blocks the save until an added item has a readable price", async () => {
    const user = userEvent.setup();
    renderWizard(servicesWith(computedReport));

    await addKafa(user);

    expect(
      await screen.findByText("Unesite ispravnu cenu u kampanji za Kafa 200 g."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeDisabled();

    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "9.900,00");

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeEnabled(),
    );
  });
});

describe("CampaignWizard item anchors", () => {
  it("previews a computed anchor with its window instead of asking for one", async () => {
    const user = userEvent.setup();
    renderWizard(servicesWith(computedReport));

    await addKafa(user);
    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "9.900,00");

    expect(
      await screen.findByText("Prethodna: 11.900,00 RSD (prozor 30 d.)"),
    ).toBeInTheDocument();
    expect(
      screen.queryByLabelText("Prethodna cena za Kafa 200 g"),
    ).not.toBeInTheDocument();
  });

  it("marks a truncated window rather than passing the anchor off as complete", async () => {
    const user = userEvent.setup();
    renderWizard(
      servicesWith({
        ...computedReport,
        anchors: [{ ...computedReport.anchors[0], anchorTruncated: true }],
      }),
    );

    await addKafa(user);
    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "9.900,00");

    expect(await screen.findByText(/skraćena/i)).toBeInTheDocument();
  });

  it("asks for a manual anchor and justification exactly when the report demands them", async () => {
    const user = userEvent.setup();
    renderWizard(servicesWith(manualReport));

    await addKafa(user);
    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "120,00");

    expect(
      await screen.findByLabelText("Prethodna cena za Kafa 200 g"),
    ).toBeInTheDocument();
    expect(
      screen.getByLabelText("Obrazloženje prethodne cene za Kafa 200 g"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Za ovaj artikal prethodna cena mora biti uneta ručno uz obrazloženje."),
    ).toBeInTheDocument();
  });

  it("asks promotivna for the future regular price and never for an anchor", async () => {
    const user = userEvent.setup();
    const services = servicesWith({
      ...emptyReport,
      anchors: [
        {
          productId: 10,
          anchorStatus: "none",
          prethodnaCenaMinor: null,
          anchorWindowDays: null,
          anchorTruncated: false,
          anchorReason: null,
          anchorJustification: null,
        },
      ],
    });
    renderWizard(services);

    await user.click(screen.getByRole("radio", { name: "Promotivna prodaja" }));
    await addKafa(user);

    expect(
      await screen.findByLabelText("Buduća redovna cena za Kafa 200 g"),
    ).toBeInTheDocument();
    expect(
      screen.queryByLabelText("Prethodna cena za Kafa 200 g"),
    ).not.toBeInTheDocument();
  });
});

describe("CampaignWizard saving", () => {
  it("creates a draft from a correctly shaped input", async () => {
    const user = userEvent.setup();
    const services = servicesWith(computedReport);
    const create = vi.spyOn(services.campaigns, "createCampaign");
    const { onSaved } = renderWizard(services);

    await user.click(screen.getByRole("radio", { name: "Sezonsko sniženje" }));
    setDate("Datum početka", "2026-07-05");
    setDate("Datum isteka", "2026-08-01");
    await user.type(screen.getByLabelText("Marketinška oznaka"), "Letnje sniženje");
    await user.click(
      screen.getByRole("checkbox", {
        name: "Potvrđujem da je sezona protekla (čl. 37 st. 8).",
      }),
    );
    await addKafa(user);
    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "9.900,00");

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "Sačuvaj nacrt" }));

    await waitFor(() =>
      expect(create).toHaveBeenCalledWith({
        campaignType: "sezonsko_snizenje",
        startsOn: "2026-07-05T00:00:00Z",
        endsOn: "2026-08-01T00:00:00Z",
        displayMode: "two_prices",
        headlinePercent: null,
        rasprodajaGround: null,
        specialConditions: null,
        reducedUtilityReason: null,
        marketingLabel: "Letnje sniženje",
        seasonAttested: true,
        separationAttested: false,
        items: [
          {
            productId: 10,
            campaignPriceMinor: 990000,
            manualPrethodnaMinor: null,
            anchorJustification: null,
            futureRegularPriceMinor: null,
          },
        ],
      }),
    );
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
  });

  it("sends the manual anchor and its justification when the report demanded them", async () => {
    const user = userEvent.setup();
    const services = servicesWith(manualReport);
    const create = vi.spyOn(services.campaigns, "createCampaign");
    renderWizard(services);

    await user.click(screen.getByRole("radio", { name: "Akcijska prodaja" }));
    setDate("Datum početka", "2026-07-01");
    setDate("Datum isteka", "2026-07-10");
    await addKafa(user);
    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "120,00");
    await user.type(
      await screen.findByLabelText("Prethodna cena za Kafa 200 g"),
      "150,00",
    );
    await user.type(
      screen.getByLabelText("Obrazloženje prethodne cene za Kafa 200 g"),
      "Cena sa police 30.06.2026.",
    );

    // The report still carries h13a (it is mocked), so Save stays blocked —
    // the wizard must not decide for itself that the anchor is now fine.
    expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeDisabled();
    expect(create).not.toHaveBeenCalled();

    await waitFor(() =>
      expect(services.campaigns.validateCampaign).toHaveBeenLastCalledWith(
        expect.objectContaining({
          items: [
            {
              productId: 10,
              campaignPriceMinor: 12000,
              manualPrethodnaMinor: 15000,
              anchorJustification: "Cena sa police 30.06.2026.",
              futureRegularPriceMinor: null,
            },
          ],
        }),
      ),
    );
  });

  it("prefills an existing draft and updates it in place", async () => {
    const user = userEvent.setup();
    const services = servicesWith(computedReport);
    const update = vi.spyOn(services.campaigns, "updateCampaign");
    const draft: CampaignView = {
      id: 7,
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
          anchorTruncated: false,
          anchorReason: null,
          anchorJustification: null,
          futureRegularPriceMinor: null,
          preCampaignPriceMinor: 1290000,
        },
      ],
      warnings: [],
    };

    renderWizard(services, draft);

    expect(screen.getByLabelText("Datum početka")).toHaveValue("2026-07-05");
    expect(screen.getByLabelText("Marketinška oznaka")).toHaveValue("Letnje sniženje");
    expect(screen.getByLabelText("Cena u kampanji za Kafa 200 g")).toHaveValue("9.900,00");
    expect(screen.getByLabelText("Posebni uslovi")).toHaveValue("Ne važi uz druge popuste.");

    const price = screen.getByLabelText("Cena u kampanji za Kafa 200 g");
    await user.clear(price);
    await user.type(price, "8.900,00");

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "Sačuvaj nacrt" }));

    await waitFor(() =>
      expect(update).toHaveBeenCalledWith(
        7,
        expect.objectContaining({
          items: [
            {
              productId: 10,
              campaignPriceMinor: 890000,
              manualPrethodnaMinor: null,
              anchorJustification: null,
              futureRegularPriceMinor: null,
            },
          ],
        }),
      ),
    );
  });

  it("surfaces a save failure instead of pretending the draft was stored", async () => {
    const user = userEvent.setup();
    const services = servicesWith(computedReport);
    vi.spyOn(services.campaigns, "createCampaign").mockRejectedValue({
      code: "business",
      message: "Sezonsko sniženje je već iskorišćeno dva puta u 2026.",
    });
    const { onSaved } = renderWizard(services);

    await addKafa(user);
    await user.type(screen.getByLabelText("Cena u kampanji za Kafa 200 g"), "9.900,00");

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Sačuvaj nacrt" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "Sačuvaj nacrt" }));

    expect(
      await screen.findByText("Sezonsko sniženje je već iskorišćeno dva puta u 2026."),
    ).toBeInTheDocument();
    expect(onSaved).not.toHaveBeenCalled();
  });
});

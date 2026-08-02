import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { RetentionPanel } from "./RetentionPanel";
import { Toaster } from "@/components/ui/sonner";
import { createMockServices } from "@/services/mock-adapter";

function renderPanel(services = createMockServices()) {
  render(
    <>
      <RetentionPanel retention={services.retention} />
      <Toaster />
    </>,
  );
  return services;
}

/**
 * The row a class is shown on. Found by the name a person reads rather than by
 * `record_class`: the operator never sees the stored key, and a screen offering
 * to move „worktime_draft“ forward is a screen nobody in the shop can use.
 */
async function rowFor(naziv: string) {
  const label = await screen.findByText(naziv);
  // eslint-disable-next-line testing-library/no-node-access
  return label.closest("tr") as HTMLTableRowElement;
}

describe("RetentionPanel", () => {
  it("shows every class, the rok in force and where it comes from", async () => {
    renderPanel();

    const pristup = await rowFor("Evidencija pristupa podacima o ličnosti");
    expect(within(pristup).getByText("2028-06-18")).toBeInTheDocument();

    // The trajno classes are listed too — „rok se ne podešava“ is the answer to
    // a question the operator would otherwise ask by trying.
    const zaposleni = await rowFor("Evidencija o zaposlenim licima");
    expect(
      within(zaposleni).getByText("Trajno — bez datuma isteka"),
    ).toBeInTheDocument();
    expect(within(zaposleni).getByText("Rok se ne podešava.")).toBeInTheDocument();
  });

  it("moves a bounded rok forward and shows the value that is now in force", async () => {
    const user = userEvent.setup();
    const services = renderPanel();
    const extendPolicy = vi.spyOn(services.retention, "extendPolicy");

    const pristup = await rowFor("Evidencija pristupa podacima o ličnosti");
    const field = within(pristup).getByLabelText(
      "Novi rok — Evidencija pristupa podacima o ličnosti",
    );
    await user.clear(field);
    await user.type(field, "2030-01-01");
    await user.click(within(pristup).getByRole("button", { name: "Pomeri rok" }));

    expect(extendPolicy).toHaveBeenCalledWith("access_log", "2030-01-01");
    expect(await screen.findByText("Rok čuvanja je pomeren.")).toBeInTheDocument();

    const moved = await rowFor("Evidencija pristupa podacima o ličnosti");
    expect(within(moved).getByText("2030-01-01")).toBeInTheDocument();
  });

  it("shows the backend refusal verbatim when a rok would be shortened", async () => {
    const user = userEvent.setup();
    renderPanel();

    const pristup = await rowFor("Evidencija pristupa podacima o ličnosti");
    const field = within(pristup).getByLabelText(
      "Novi rok — Evidencija pristupa podacima o ličnosti",
    );
    await user.clear(field);
    await user.type(field, "2027-01-01");
    await user.click(within(pristup).getByRole("button", { name: "Pomeri rok" }));

    expect(
      await screen.findByText(/se ne skraćuje na „2027-01-01“/),
    ).toBeInTheDocument();

    // The refused value must not be shown as though it had been stored.
    const unchanged = await rowFor("Evidencija pristupa podacima o ličnosti");
    expect(within(unchanged).getByText("2028-06-18")).toBeInTheDocument();
  });

  it("offers no control at all for a class that is kept trajno", async () => {
    renderPanel();

    const zaposleni = await rowFor("Evidencija o zaposlenim licima");
    expect(
      within(zaposleni).queryByRole("button", { name: "Pomeri rok" }),
    ).not.toBeInTheDocument();
    expect(
      within(zaposleni).queryByLabelText(
        "Novi rok — Evidencija o zaposlenim licima",
      ),
    ).not.toBeInTheDocument();

    const registar = await rowFor("Evidencija o radnjama obrade");
    expect(
      within(registar).queryByRole("button", { name: "Pomeri rok" }),
    ).not.toBeInTheDocument();
  });

  it("states that the rok only ever moves forward, and never off", async () => {
    renderPanel();

    expect(await screen.findByText(/samo unapred/i)).toBeInTheDocument();
    expect(await screen.findByText(/trajno je odsustvo roka/i)).toBeInTheDocument();
  });
});

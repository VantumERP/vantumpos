import { PlusIcon } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { CatalogService } from "@/services/ports";
import type { CommandErrorShape, ProductFormData, ProductSummary } from "@/services/types";

interface ProductCatalogScreenProps {
  catalog: CatalogService;
}

interface ProductFormErrors {
  name?: string;
  sku?: string;
  salePrice?: string;
}

const emptyForm = {
  name: "",
  sku: "",
  barcode: "",
  salePrice: "",
};

export function ProductCatalogScreen({ catalog }: ProductCatalogScreenProps) {
  const [products, setProducts] = useState<ProductSummary[]>([]);
  const [search, setSearch] = useState("");
  const [formOpen, setFormOpen] = useState(false);
  const [form, setForm] = useState(emptyForm);
  const [errors, setErrors] = useState<ProductFormErrors>({});

  useEffect(() => {
    let cancelled = false;

    catalog.listProducts({}).then((result) => {
      if (!cancelled) {
        setProducts(result.items);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [catalog]);

  async function saveProduct() {
    const nextErrors: ProductFormErrors = {};

    if (!form.name.trim()) {
      nextErrors.name = "Naziv je obavezan.";
    }

    if (!form.sku.trim()) {
      nextErrors.sku = "SKU/sifra je obavezna.";
    }

    let salePriceMinor = 0;
    if (form.salePrice.trim()) {
      try {
        salePriceMinor = parseRsdInput(form.salePrice);
      } catch (error) {
        nextErrors.salePrice =
          error instanceof Error ? error.message : "Cena nije ispravna.";
      }
    }

    if (Object.keys(nextErrors).length > 0) {
      setErrors(nextErrors);
      return;
    }

    const request: ProductFormData = {
      name: form.name.trim(),
      sku: form.sku.trim(),
      barcode: form.barcode.trim() || null,
      categoryId: 1,
      unitOfMeasure: "kom",
      salePriceMinor,
      purchasePriceMinor: 0,
      taxRateId: 1,
      minimumStockMilli: 0,
      allowNegativeStock: false,
      active: true,
    };

    try {
      const created = await catalog.createProduct(request);
      setProducts((current) => [created, ...current]);
      setForm(emptyForm);
      setErrors({});
      setFormOpen(false);
    } catch (error) {
      const commandError = error as CommandErrorShape;
      if (commandError.code === "duplicate_sku") {
        setErrors({ sku: commandError.message });
        return;
      }

      setErrors({ name: commandError.message ?? "Artikal nije sacuvan." });
    }
  }

  async function setActive(product: ProductSummary, active: boolean) {
    const updated = await catalog.setProductActive(product.id, active);
    setProducts((current) =>
      current.map((item) => (item.id === updated.id ? updated : item)),
    );
  }

  const visibleProducts = products.filter((product) => {
    const term = search.toLocaleLowerCase("sr-Latn");
    return (
      !term ||
      product.name.toLocaleLowerCase("sr-Latn").includes(term) ||
      product.sku.toLocaleLowerCase("sr-Latn").includes(term)
    );
  });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div className="min-w-0">
          <h2 className="text-base font-semibold">Artikli</h2>
        </div>
        <div className="flex flex-col gap-2 md:flex-row">
          <InputGroup className="md:w-72">
            <InputGroupInput
              aria-label="Pretraga artikala"
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
            <InputGroupAddon align="inline-end">Pretraga</InputGroupAddon>
          </InputGroup>
          <Button type="button" onClick={() => setFormOpen(true)}>
            <PlusIcon data-icon="inline-start" />
            Novi artikal
          </Button>
        </div>
      </div>

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Naziv</TableHead>
            <TableHead>SKU/sifra</TableHead>
            <TableHead>Barcode</TableHead>
            <TableHead>Cena</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Akcije</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {visibleProducts.map((product) => (
            <TableRow key={product.id}>
              <TableCell>{product.name}</TableCell>
              <TableCell>{product.sku}</TableCell>
              <TableCell>{product.barcode ?? "-"}</TableCell>
              <TableCell>{formatRsd(product.salePriceMinor)}</TableCell>
              <TableCell>
                <Badge variant={product.active ? "secondary" : "outline"}>
                  {product.active ? "Aktivan" : "Neaktivan"}
                </Badge>
              </TableCell>
              <TableCell>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setActive(product, !product.active)}
                >
                  {product.active ? "Deaktiviraj" : "Aktiviraj"} {product.name}
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      <Dialog open={formOpen} onOpenChange={setFormOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Novi artikal</DialogTitle>
            <DialogDescription>Unesite osnovne podatke za prodaju.</DialogDescription>
          </DialogHeader>
          <FieldGroup>
            <Field data-invalid={Boolean(errors.name)}>
              <FieldLabel htmlFor="product-name">Naziv</FieldLabel>
              <Input
                id="product-name"
                value={form.name}
                aria-invalid={Boolean(errors.name)}
                onChange={(event) =>
                  setForm((current) => ({ ...current, name: event.target.value }))
                }
              />
              <FieldError>{errors.name}</FieldError>
            </Field>
            <Field data-invalid={Boolean(errors.sku)}>
              <FieldLabel htmlFor="product-sku">SKU/sifra</FieldLabel>
              <Input
                id="product-sku"
                value={form.sku}
                aria-invalid={Boolean(errors.sku)}
                onChange={(event) =>
                  setForm((current) => ({ ...current, sku: event.target.value }))
                }
              />
              <FieldError>{errors.sku}</FieldError>
            </Field>
            <Field>
              <FieldLabel htmlFor="product-barcode">Barcode</FieldLabel>
              <Input
                id="product-barcode"
                value={form.barcode}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    barcode: event.target.value,
                  }))
                }
              />
            </Field>
            <Field data-invalid={Boolean(errors.salePrice)}>
              <FieldLabel htmlFor="product-sale-price">
                Prodajna cena sa PDV
              </FieldLabel>
              <Input
                id="product-sale-price"
                value={form.salePrice}
                aria-invalid={Boolean(errors.salePrice)}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    salePrice: event.target.value,
                  }))
                }
              />
              <FieldError>{errors.salePrice}</FieldError>
            </Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" onClick={saveProduct}>
              Sacuvaj artikal
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

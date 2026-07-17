import {
  EditIcon,
  FolderOpenIcon,
  Globe2Icon,
  PackagePlusIcon,
  PlusIcon,
  SearchIcon,
} from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  KeyboardEvent as ReactKeyboardEvent,
  MutableRefObject,
} from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
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
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { formatRsd, parseRsdInput } from "@/lib/money";
import type { PosServices } from "@/services/ports";
import type {
  CategorySummary,
  CommandError,
  PrethodnaCenaDto,
  ProductExternalSource,
  ProductListQuery,
  ProductLookupSuggestion,
  ProductSummary,
  SaveCategoryRequest,
  SaveProductRequest,
  TaxRateSummary,
} from "@/services/types";

type ActiveFilter = "all" | "active" | "inactive";
type ProductEntryMode = "quick" | "bulk";
type BulkProductEditableField =
  | "barcode"
  | "name"
  | "sku"
  | "salePrice"
  | "taxRateId"
  | "categoryId";

interface CatalogModuleProps {
  services: PosServices;
  onOpenInventory: (productId: number) => void;
}

interface ProductFormState {
  name: string;
  sku: string;
  barcode: string;
  categoryId: string;
  unitOfMeasure: string;
  salePrice: string;
  purchasePrice: string;
  taxRateId: string;
  minimumStock: string;
  allowNegativeStock: boolean;
  active: boolean;
  externalSource: ProductExternalSource | null;
}

type ProductFieldErrors = Partial<Record<keyof ProductFormState, string>> & {
  form?: string;
};

type BulkProductFieldErrors = Partial<
  Record<"barcode" | "name" | "sku" | "salePrice" | "taxRateId", string>
> & {
  form?: string;
};

interface BulkProductRowState {
  id: string;
  barcode: string;
  name: string;
  sku: string;
  salePrice: string;
  taxRateId: string;
  categoryId: string;
  externalSource: ProductExternalSource | null;
  fieldErrors: BulkProductFieldErrors;
  lookupError: string | null;
  lookupLoading: boolean;
}

interface CategoryFormState {
  id?: number;
  name: string;
  active: boolean;
}

const EMPTY_PRODUCT_FORM: ProductFormState = {
  name: "",
  sku: "",
  barcode: "",
  categoryId: "none",
  unitOfMeasure: "kom",
  salePrice: "",
  purchasePrice: "",
  taxRateId: "",
  minimumStock: "0",
  allowNegativeStock: false,
  active: true,
  externalSource: null,
};

let bulkRowSequence = 0;

export function CatalogModule({ services, onOpenInventory }: CatalogModuleProps) {
  const [search, setSearch] = useState("");
  const [categoryFilter, setCategoryFilter] = useState("all");
  const [activeFilter, setActiveFilter] = useState<ActiveFilter>("all");
  const [taxRateFilter, setTaxRateFilter] = useState("all");
  const [lowStock, setLowStock] = useState(false);
  const [missingBarcode, setMissingBarcode] = useState(false);
  const [products, setProducts] = useState<ProductSummary[]>([]);
  const [categories, setCategories] = useState<CategorySummary[]>([]);
  const [taxRates, setTaxRates] = useState<TaxRateSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [productSheetOpen, setProductSheetOpen] = useState(false);
  const [editingProduct, setEditingProduct] = useState<ProductSummary | null>(
    null,
  );
  const [productForm, setProductForm] = useState<ProductFormState>(
    EMPTY_PRODUCT_FORM,
  );
  const [productErrors, setProductErrors] = useState<ProductFieldErrors>({});
  const [savingProduct, setSavingProduct] = useState(false);
  const [prethodnaCena, setPrethodnaCena] = useState<PrethodnaCenaDto | null>(
    null,
  );
  const [lookupSuggestion, setLookupSuggestion] =
    useState<ProductLookupSuggestion | null>(null);
  const [lookupLoading, setLookupLoading] = useState(false);
  const [lookupError, setLookupError] = useState<string | null>(null);
  const [bulkRows, setBulkRows] = useState<BulkProductRowState[]>([]);
  const [savingBulkProducts, setSavingBulkProducts] = useState(false);
  const [categorySheetOpen, setCategorySheetOpen] = useState(false);
  const [categoryForm, setCategoryForm] = useState<CategoryFormState>({
    name: "",
    active: true,
  });
  const [categoryError, setCategoryError] = useState<string | null>(null);
  const [savingCategory, setSavingCategory] = useState(false);

  const productQuery = useMemo<ProductListQuery>(
    () => ({
      search,
      categoryId:
        categoryFilter === "all" ? undefined : Number(categoryFilter),
      active:
        activeFilter === "all" ? undefined : activeFilter === "active",
      lowStock: lowStock || undefined,
      missingBarcode: missingBarcode || undefined,
      taxRateId: taxRateFilter === "all" ? undefined : Number(taxRateFilter),
    }),
    [
      activeFilter,
      categoryFilter,
      lowStock,
      missingBarcode,
      search,
      taxRateFilter,
    ],
  );

  const loadProducts = useCallback(async () => {
    setLoading(true);
    setLoadError(null);

    try {
      const result = await services.catalog.listProducts(productQuery);
      setProducts(result.items);
      setCategories(result.categories ?? []);
      setTaxRates(result.taxRates ?? []);
    } catch (error) {
      setLoadError(commandMessage(error, "Artikli nisu učitani."));
    } finally {
      setLoading(false);
    }
  }, [productQuery, services]);

  useEffect(() => {
    void loadProducts();
  }, [loadProducts]);

  // ZoT čl. 37 st. 3-4: lowering the offered price of an existing article is
  // what triggers the prethodna cena duty, so the advisory is fetched exactly
  // then. It is read-only guidance and never gates saving.
  useEffect(() => {
    if (!productSheetOpen || !editingProduct) {
      setPrethodnaCena(null);
      return;
    }

    let enteredPriceMinor: number;
    try {
      enteredPriceMinor = parseRsdInput(productForm.salePrice);
    } catch {
      setPrethodnaCena(null);
      return;
    }

    if (enteredPriceMinor >= editingProduct.salePriceMinor) {
      setPrethodnaCena(null);
      return;
    }

    let cancelled = false;

    void services.catalog
      .getPrethodnaCena(editingProduct.id, new Date().toISOString())
      .then((result) => {
        if (!cancelled) {
          setPrethodnaCena(result);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setPrethodnaCena(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [editingProduct, productForm.salePrice, productSheetOpen, services]);

  function openCreateProduct() {
    const taxRateId = defaultProductTaxRateId(taxRates);

    setEditingProduct(null);
    setProductErrors({});
    setLookupSuggestion(null);
    setLookupError(null);
    setProductForm({
      ...EMPTY_PRODUCT_FORM,
      taxRateId,
    });
    setBulkRows([createBulkProductRow(taxRateId)]);
    setProductSheetOpen(true);
  }

  function openEditProduct(product: ProductSummary) {
    setEditingProduct(product);
    setProductErrors({});
    setLookupSuggestion(null);
    setLookupError(null);
    setBulkRows([createBulkProductRow(defaultProductTaxRateId(taxRates))]);
    setProductForm({
      name: product.name,
      sku: product.sku,
      barcode: product.barcode ?? "",
      categoryId: product.categoryId?.toString() ?? "none",
      unitOfMeasure: product.unitOfMeasure,
      salePrice: minorUnitsInput(product.salePriceMinor),
      purchasePrice: minorUnitsInput(product.purchasePriceMinor),
      taxRateId: product.taxRateId.toString(),
      minimumStock: quantityInput(product.minimumStockMilli),
      allowNegativeStock: product.allowNegativeStock,
      active: product.active,
      externalSource: product.externalSource ?? null,
    });
    setProductSheetOpen(true);
  }

  async function handleBarcodeLookup() {
    const barcode = productForm.barcode.trim();

    setLookupSuggestion(null);
    setLookupError(null);

    if (!barcode) {
      setLookupError("Unesite barcode pre pretrage.");
      return;
    }

    setLookupLoading(true);
    try {
      const suggestion = await services.catalog.lookupProductByBarcode(barcode);

      if (!suggestion) {
        setLookupError("Nema javnih podataka za ovaj barcode.");
        return;
      }

      setLookupSuggestion(suggestion);
    } catch (error) {
      setLookupError(commandMessage(error, "Pretraga barcode-a nije uspela."));
    } finally {
      setLookupLoading(false);
    }
  }

  function applyLookupSuggestion() {
    if (!lookupSuggestion) {
      return;
    }

    const acceptedFields: string[] = [];
    const nextForm = { ...productForm };
    const suggestedName = lookupProductDisplayName(lookupSuggestion.fields);

    if (suggestedName) {
      nextForm.name = suggestedName;
      acceptedFields.push("name");
    }

    if (lookupSuggestion.fields.brand) {
      acceptedFields.push("brand");
    }

    if (lookupSuggestion.fields.packageSize) {
      acceptedFields.push("packageSize");
    }

    if (lookupSuggestion.fields.categoryName) {
      const category = matchingCategory(categories, lookupSuggestion.fields.categoryName);

      if (category && nextForm.categoryId === "none") {
        nextForm.categoryId = category.id.toString();
        acceptedFields.push("categoryName");
      }
    }

    nextForm.externalSource = {
      ...lookupSuggestion.source,
      acceptedFields,
    };

    setProductForm(nextForm);
    setLookupError(null);
  }

  async function handleProductSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const validation = validateProductForm(productForm);

    if (Object.keys(validation).length > 0) {
      setProductErrors(validation);
      return;
    }

    setSavingProduct(true);
    setProductErrors({});

    try {
      const request = productRequest(productForm);
      const saved = editingProduct
        ? await services.catalog.updateProduct(editingProduct.id, request)
        : await services.catalog.createProduct(request);

      setProducts((current) =>
        editingProduct
          ? current.map((product) => (product.id === saved.id ? saved : product))
          : [saved, ...current],
      );
      setProductSheetOpen(false);
    } catch (error) {
      setProductErrors(commandFieldErrors(error));
    } finally {
      setSavingProduct(false);
    }
  }

  function handleBulkRowChange(
    rowId: string,
    field: BulkProductEditableField,
    value: string,
  ) {
    setBulkRows((current) =>
      current.map((row) => {
        if (row.id !== rowId) {
          return row;
        }

        const nextRow = { ...row, [field]: value };
        const nextErrors = { ...row.fieldErrors, [field]: undefined };

        if (field === "barcode") {
          nextRow.lookupError = null;
          nextRow.externalSource =
            row.externalSource?.barcode === value.trim()
              ? row.externalSource
              : null;
        }

        return {
          ...nextRow,
          fieldErrors: {
            ...nextErrors,
            form: undefined,
          },
        };
      }),
    );
  }

  function handleBulkAddRow() {
    setBulkRows((current) => [
      ...current,
      createBulkProductRow(defaultProductTaxRateId(taxRates)),
    ]);
  }

  function handleBulkRemoveRow(rowId: string) {
    setBulkRows((current) => {
      if (current.length === 1) {
        return [
          createBulkProductRow(current[0]?.taxRateId || defaultProductTaxRateId(taxRates)),
        ];
      }

      return current.filter((row) => row.id !== rowId);
    });
  }

  async function handleBulkBarcodeLookup(rowId: string) {
    const row = bulkRows.find((item) => item.id === rowId);
    const barcode = row?.barcode.trim() ?? "";

    if (!row) {
      return;
    }

    if (!barcode) {
      setBulkRows((current) =>
        current.map((item) =>
          item.id === rowId
            ? {
                ...item,
                fieldErrors: {
                  ...item.fieldErrors,
                  barcode: "Unesite barcode pre pretrage.",
                },
                lookupError: null,
              }
            : item,
        ),
      );
      return;
    }

    setBulkRows((current) => {
      const nextRows = current.map((item) =>
        item.id === rowId
          ? {
              ...item,
              lookupLoading: true,
              lookupError: null,
              fieldErrors: { ...item.fieldErrors, barcode: undefined },
            }
          : item,
      );

      if (!shouldAppendBulkRowAfterScan(current, rowId)) {
        return nextRows;
      }

      return [
        ...nextRows,
        createBulkProductRow(defaultProductTaxRateId(taxRates)),
      ];
    });

    try {
      const suggestion = await services.catalog.lookupProductByBarcode(barcode);

      setBulkRows((current) =>
        current.map((item) => {
          if (item.id !== rowId) {
            return item;
          }

          if (!suggestion) {
            return {
              ...item,
              lookupLoading: false,
              lookupError: "Nema javnih podataka za ovaj barcode.",
            };
          }

          const suggestedName = lookupProductDisplayName(suggestion.fields);
          const acceptsSuggestedName = Boolean(suggestedName && !item.name.trim());
          const acceptedFields = acceptedLookupFields(
            suggestion.fields,
            acceptsSuggestedName,
          );
          const nextName = acceptsSuggestedName ? suggestedName : item.name;

          return {
            ...item,
            name: nextName,
            externalSource: {
              ...suggestion.source,
              acceptedFields,
            },
            fieldErrors: {
              ...item.fieldErrors,
              name: undefined,
              form: undefined,
            },
            lookupError: null,
            lookupLoading: false,
          };
        }),
      );
    } catch (error) {
      setBulkRows((current) =>
        current.map((item) =>
          item.id === rowId
            ? {
                ...item,
                lookupLoading: false,
                lookupError: commandMessage(
                  error,
                  "Pretraga barcode-a nije uspela.",
                ),
              }
            : item,
        ),
      );
    }
  }

  async function handleBulkSubmit() {
    const rowsWithInput = bulkRows.filter(hasBulkRowInput);

    if (rowsWithInput.length === 0) {
      setBulkRows((current) =>
        current.map((row, index) =>
          index === 0
            ? {
                ...row,
                fieldErrors: {
                  ...row.fieldErrors,
                  form: "Unesite bar jedan artikal.",
                },
              }
            : row,
        ),
      );
      return;
    }

    const validationRows = bulkRows.map((row) => {
      if (!hasBulkRowInput(row)) {
        return { ...row, fieldErrors: {}, lookupError: null };
      }

      return {
        ...row,
        fieldErrors: validateBulkProductRow(row),
        lookupError: null,
      };
    });

    if (
      validationRows.some(
        (row) => hasBulkRowInput(row) && hasBulkRowErrors(row.fieldErrors),
      )
    ) {
      setBulkRows(validationRows);
      return;
    }

    setSavingBulkProducts(true);
    setBulkRows(validationRows);

    const createdProducts: ProductSummary[] = [];
    let nextRows = validationRows;

    for (const row of validationRows.filter(hasBulkRowInput)) {
      try {
        const saved = await services.catalog.createProduct(bulkProductRequest(row));
        createdProducts.push(saved);
        nextRows = nextRows.map((item) =>
          item.id === row.id ? { ...item, fieldErrors: {}, lookupError: null } : item,
        );
      } catch (error) {
        nextRows = nextRows.map((item) =>
          item.id === row.id
            ? {
                ...item,
                fieldErrors: bulkCommandFieldErrors(error),
              }
            : item,
        );
      }
    }

    if (createdProducts.length > 0) {
      setProducts((current) => [...createdProducts, ...current]);
    }

    const failedRows = nextRows.filter(
      (row) => hasBulkRowInput(row) && hasBulkRowErrors(row.fieldErrors),
    );

    if (failedRows.length > 0) {
      setBulkRows(failedRows);
    } else {
      setBulkRows([createBulkProductRow(defaultProductTaxRateId(taxRates))]);
      setProductSheetOpen(false);
    }

    setSavingBulkProducts(false);
  }

  async function handleSetProductActive(product: ProductSummary, active: boolean) {
    const updated = await services.catalog.setProductActive(product.id, active);
    setProducts((current) =>
      current.map((item) => (item.id === updated.id ? updated : item)),
    );
  }

  function openCreateCategory() {
    setCategoryError(null);
    setCategoryForm({ name: "", active: true });
    setCategorySheetOpen(true);
  }

  function openEditCategory(category: CategorySummary) {
    setCategoryError(null);
    setCategoryForm(category);
    setCategorySheetOpen(true);
  }

  async function handleCategorySubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!categoryForm.name.trim()) {
      setCategoryError("Naziv kategorije je obavezan.");
      return;
    }

    setSavingCategory(true);
    setCategoryError(null);

    try {
      const request: SaveCategoryRequest = {
        id: categoryForm.id,
        name: categoryForm.name.trim(),
        active: categoryForm.active,
      };
      const saved = await services.catalog.saveCategory(request);
      setCategories((current) =>
        categoryForm.id
          ? current.map((category) =>
              category.id === saved.id ? saved : category,
            )
          : [...current, saved],
      );
      setCategorySheetOpen(false);
    } catch (error) {
      setCategoryError(commandMessage(error, "Kategorija nije sačuvana."));
    } finally {
      setSavingCategory(false);
    }
  }

  return (
    <div className="flex flex-1 flex-col gap-4">
      <Tabs defaultValue="products">
        <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <TabsList>
            <TabsTrigger value="products">Artikli</TabsTrigger>
            <TabsTrigger value="categories">Kategorije</TabsTrigger>
          </TabsList>
          <Button type="button" onClick={openCreateProduct}>
            <PlusIcon data-icon="inline-start" />
            Novi artikal
          </Button>
        </div>
        <TabsContent value="products" className="flex flex-col gap-4">
          <ProductFilters
            activeFilter={activeFilter}
            categories={categories}
            categoryFilter={categoryFilter}
            lowStock={lowStock}
            missingBarcode={missingBarcode}
            search={search}
            taxRateFilter={taxRateFilter}
            taxRates={taxRates}
            onActiveFilterChange={setActiveFilter}
            onCategoryFilterChange={setCategoryFilter}
            onLowStockChange={setLowStock}
            onMissingBarcodeChange={setMissingBarcode}
            onSearchChange={setSearch}
            onTaxRateFilterChange={setTaxRateFilter}
          />
          {loadError ? (
            <div className="rounded-md border p-4 text-sm text-destructive">
              {loadError}
            </div>
          ) : loading ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Spinner data-icon="inline-start" />
              Učitavanje artikala
            </div>
          ) : products.length === 0 ? (
            <Empty>
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <PackagePlusIcon />
                </EmptyMedia>
                <EmptyTitle>Nema artikala za izabrane filtere</EmptyTitle>
                <EmptyDescription>
                  Promenite filtere ili otvorite novi artikal.
                </EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Button type="button" onClick={openCreateProduct}>
                  <PlusIcon data-icon="inline-start" />
                  Novi artikal
                </Button>
              </EmptyContent>
            </Empty>
          ) : (
            <ProductTable
              products={products}
              onEdit={openEditProduct}
              onOpenInventory={onOpenInventory}
              onSetActive={handleSetProductActive}
            />
          )}
        </TabsContent>
        <TabsContent value="categories" className="flex flex-col gap-4">
          <div className="flex justify-end">
            <Button type="button" variant="outline" onClick={openCreateCategory}>
              <PlusIcon data-icon="inline-start" />
              Nova kategorija
            </Button>
          </div>
          <CategoryTable categories={categories} onEdit={openEditCategory} />
        </TabsContent>
      </Tabs>
      <ProductSheet
        allowBulk={!editingProduct}
        bulkRows={bulkRows}
        categories={categories}
        errors={productErrors}
        form={productForm}
        lookupError={lookupError}
        lookupLoading={lookupLoading}
        lookupSuggestion={lookupSuggestion}
        open={productSheetOpen}
        prethodnaCena={prethodnaCena}
        saving={savingProduct}
        taxRates={taxRates}
        title={editingProduct ? "Izmena artikla" : "Novi artikal"}
        onApplyLookupSuggestion={applyLookupSuggestion}
        onBarcodeLookup={handleBarcodeLookup}
        onBulkAddRow={handleBulkAddRow}
        onBulkLookup={handleBulkBarcodeLookup}
        onBulkRemoveRow={handleBulkRemoveRow}
        onBulkRowChange={handleBulkRowChange}
        onBulkSubmit={handleBulkSubmit}
        onFormChange={setProductForm}
        onOpenChange={setProductSheetOpen}
        onSubmit={handleProductSubmit}
        savingBulk={savingBulkProducts}
      />
      <CategorySheet
        error={categoryError}
        form={categoryForm}
        open={categorySheetOpen}
        saving={savingCategory}
        onFormChange={setCategoryForm}
        onOpenChange={setCategorySheetOpen}
        onSubmit={handleCategorySubmit}
      />
    </div>
  );
}

interface ProductFiltersProps {
  activeFilter: ActiveFilter;
  categories: CategorySummary[];
  categoryFilter: string;
  lowStock: boolean;
  missingBarcode: boolean;
  search: string;
  taxRateFilter: string;
  taxRates: TaxRateSummary[];
  onActiveFilterChange: (value: ActiveFilter) => void;
  onCategoryFilterChange: (value: string) => void;
  onLowStockChange: (value: boolean) => void;
  onMissingBarcodeChange: (value: boolean) => void;
  onSearchChange: (value: string) => void;
  onTaxRateFilterChange: (value: string) => void;
}

function ProductFilters({
  activeFilter,
  categories,
  categoryFilter,
  lowStock,
  missingBarcode,
  search,
  taxRateFilter,
  taxRates,
  onActiveFilterChange,
  onCategoryFilterChange,
  onLowStockChange,
  onMissingBarcodeChange,
  onSearchChange,
  onTaxRateFilterChange,
}: ProductFiltersProps) {
  const categoryItems = [
    { label: "Sve kategorije", value: "all" },
    ...categories.map((category) => ({
      label: category.name,
      value: category.id.toString(),
    })),
  ];
  const activeItems = [
    { label: "Svi statusi", value: "all" },
    { label: "Aktivni", value: "active" },
    { label: "Neaktivni", value: "inactive" },
  ];
  const taxRateItems = [
    { label: "Sve PDV stope", value: "all" },
    ...taxRates.map((taxRate) => ({
      label: taxRateLabel(taxRate),
      value: taxRate.id.toString(),
    })),
  ];

  return (
    <div className="flex flex-col gap-3">
      <Field>
        <FieldLabel htmlFor="catalog-search" className="sr-only">
          Pretraga artikala
        </FieldLabel>
        <InputGroup>
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
          <InputGroupInput
            id="catalog-search"
            aria-label="Pretraga artikala"
            type="search"
            placeholder="Naziv, SKU/šifra ili barcode"
            value={search}
            onChange={(event) => onSearchChange(event.target.value)}
          />
        </InputGroup>
      </Field>
      <div className="flex flex-wrap items-center gap-2">
        <CatalogSelect
          id="catalog-category-filter"
          items={categoryItems}
          value={categoryFilter}
          onValueChange={onCategoryFilterChange}
        />
        <CatalogSelect
          id="catalog-active-filter"
          items={activeItems}
          value={activeFilter}
          onValueChange={(value) => onActiveFilterChange(value as ActiveFilter)}
        />
        <CatalogSelect
          id="catalog-tax-filter"
          items={taxRateItems}
          value={taxRateFilter}
          onValueChange={onTaxRateFilterChange}
        />
        <Field orientation="horizontal" className="w-auto">
          <Checkbox
            id="catalog-low-stock"
            checked={lowStock}
            onCheckedChange={(checked) => onLowStockChange(Boolean(checked))}
          />
          <FieldLabel htmlFor="catalog-low-stock" className="font-normal">
            Nizak lager
          </FieldLabel>
        </Field>
        <Field orientation="horizontal" className="w-auto">
          <Checkbox
            id="catalog-missing-barcode"
            checked={missingBarcode}
            onCheckedChange={(checked) =>
              onMissingBarcodeChange(Boolean(checked))
            }
          />
          <FieldLabel htmlFor="catalog-missing-barcode" className="font-normal">
            Bez barcode-a
          </FieldLabel>
        </Field>
      </div>
    </div>
  );
}

function ProductTable({
  products,
  onEdit,
  onOpenInventory,
  onSetActive,
}: {
  products: ProductSummary[];
  onEdit: (product: ProductSummary) => void;
  onOpenInventory: (productId: number) => void;
  onSetActive: (product: ProductSummary, active: boolean) => void;
}) {
  return (
    <div className="overflow-hidden rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Naziv</TableHead>
            <TableHead>SKU/šifra</TableHead>
            <TableHead>Barcode</TableHead>
            <TableHead>Kategorija</TableHead>
            <TableHead>Cena sa PDV</TableHead>
            <TableHead>PDV</TableHead>
            <TableHead>Stanje</TableHead>
            <TableHead>Minimum</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Akcije</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {products.map((product) => (
            <TableRow key={product.id}>
              <TableCell className="font-medium">
                <div className="flex flex-col gap-1">
                  <span>{product.name}</span>
                  {product.externalSource ? (
                    <ExternalSourceBadge source={product.externalSource} />
                  ) : null}
                </div>
              </TableCell>
              <TableCell>{product.sku}</TableCell>
              <TableCell>{product.barcode || "-"}</TableCell>
              <TableCell>{product.categoryName || "-"}</TableCell>
              <TableCell>{formatRsd(product.salePriceMinor)}</TableCell>
              <TableCell>{productTaxLabel(product)}</TableCell>
              <TableCell>
                <StockBadge product={product} />
              </TableCell>
              <TableCell>{formatQuantity(product.minimumStockMilli)}</TableCell>
              <TableCell>
                <Badge variant={product.active ? "secondary" : "outline"}>
                  {product.active ? "Aktivan" : "Neaktivan"}
                </Badge>
              </TableCell>
              <TableCell>
                <div className="flex flex-wrap gap-1">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    aria-label={`Izmeni ${product.name}`}
                    onClick={() => onEdit(product)}
                  >
                    <EditIcon data-icon="inline-start" />
                    Izmeni
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    aria-label={
                      product.active
                        ? `Deaktiviraj ${product.name}`
                        : `Aktiviraj ${product.name}`
                    }
                    onClick={() => void onSetActive(product, !product.active)}
                  >
                    {product.active ? "Deaktiviraj" : "Aktiviraj"}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    aria-label={`Lager za ${product.name}`}
                    onClick={() => onOpenInventory(product.id)}
                  >
                    <FolderOpenIcon data-icon="inline-start" />
                    Lager
                  </Button>
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

function CategoryTable({
  categories,
  onEdit,
}: {
  categories: CategorySummary[];
  onEdit: (category: CategorySummary) => void;
}) {
  if (categories.length === 0) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <FolderOpenIcon />
          </EmptyMedia>
          <EmptyTitle>Nema kategorija</EmptyTitle>
          <EmptyDescription>Dodajte prvu kategoriju za artikle.</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <div className="overflow-hidden rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Naziv</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Akcije</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {categories.map((category) => (
            <TableRow key={category.id}>
              <TableCell className="font-medium">{category.name}</TableCell>
              <TableCell>
                <Badge variant={category.active ? "secondary" : "outline"}>
                  {category.active ? "Aktivna" : "Neaktivna"}
                </Badge>
              </TableCell>
              <TableCell>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => onEdit(category)}
                >
                  <EditIcon data-icon="inline-start" />
                  Izmeni
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

function ProductSheet({
  allowBulk,
  bulkRows,
  categories,
  errors,
  form,
  lookupError,
  lookupLoading,
  lookupSuggestion,
  open,
  prethodnaCena,
  saving,
  taxRates,
  title,
  onApplyLookupSuggestion,
  onBarcodeLookup,
  onBulkAddRow,
  onBulkLookup,
  onBulkRemoveRow,
  onBulkRowChange,
  onBulkSubmit,
  onFormChange,
  onOpenChange,
  onSubmit,
  savingBulk,
}: {
  allowBulk: boolean;
  bulkRows: BulkProductRowState[];
  categories: CategorySummary[];
  errors: ProductFieldErrors;
  form: ProductFormState;
  lookupError: string | null;
  lookupLoading: boolean;
  lookupSuggestion: ProductLookupSuggestion | null;
  open: boolean;
  prethodnaCena: PrethodnaCenaDto | null;
  saving: boolean;
  taxRates: TaxRateSummary[];
  title: string;
  onApplyLookupSuggestion: () => void;
  onBarcodeLookup: () => void;
  onBulkAddRow: () => void;
  onBulkLookup: (rowId: string) => void;
  onBulkRemoveRow: (rowId: string) => void;
  onBulkRowChange: (
    rowId: string,
    field: BulkProductEditableField,
    value: string,
  ) => void;
  onBulkSubmit: () => void;
  onFormChange: (form: ProductFormState) => void;
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  savingBulk: boolean;
}) {
  const [entryMode, setEntryMode] = useState<ProductEntryMode>("quick");
  const scannerInputRef = useRef<HTMLInputElement>(null);
  const bulkBarcodeRefs = useRef<Record<string, HTMLInputElement | null>>({});
  const categoryItems = [
    { label: "Bez kategorije", value: "none" },
    ...categories
      .filter((category) => category.active)
      .map((category) => ({
        label: category.name,
        value: category.id.toString(),
      })),
  ];
  const taxRateItems = taxRates
    .filter((taxRate) => taxRate.active)
    .map((taxRate) => ({
      label: taxRateLabel(taxRate),
      value: taxRate.id.toString(),
    }));

  useEffect(() => {
    if (!open) {
      return;
    }

    setEntryMode("quick");
  }, [open]);

  useEffect(() => {
    if (!open || entryMode !== "quick") {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      scannerInputRef.current?.focus();
    }, 0);

    return () => window.clearTimeout(timeoutId);
  }, [entryMode, open]);

  useEffect(() => {
    if (!open || entryMode !== "bulk") {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      const lastRow = bulkRows[bulkRows.length - 1];
      if (lastRow) {
        bulkBarcodeRefs.current[lastRow.id]?.focus();
      }
    }, 0);

    return () => window.clearTimeout(timeoutId);
  }, [bulkRows.length, entryMode, open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    function handleShortcut(event: KeyboardEvent) {
      if (!event.altKey || event.ctrlKey || event.metaKey) {
        return;
      }

      const key = event.key.toLowerCase();

      if (key === "b" && allowBulk) {
        event.preventDefault();
        setEntryMode("bulk");
      }

      if (key === "q") {
        event.preventDefault();
        setEntryMode("quick");
      }
    }

    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [allowBulk, open]);

  function handleQuickFormKeyDown(
    event: ReactKeyboardEvent<HTMLFormElement>,
  ) {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
      event.preventDefault();
      event.currentTarget.requestSubmit();
    }
  }

  function handleScannerKeyDown(event: ReactKeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter" && form.barcode.trim()) {
      event.preventDefault();
      onBarcodeLookup();
    }
  }

  function handleEntryModeChange(value: string | null) {
    if (value === "quick" || (value === "bulk" && allowBulk)) {
      setEntryMode(value);
    }
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="w-full overflow-y-auto sm:max-w-5xl">
        <SheetHeader>
          <SheetTitle>{title}</SheetTitle>
          <SheetDescription>
            Podaci se koriste za kasu, lager i kasnije račune.
          </SheetDescription>
        </SheetHeader>
        <Tabs
          value={entryMode}
          onValueChange={handleEntryModeChange}
          className="px-6"
        >
          <TabsList>
            <TabsTrigger value="quick">Brz unos</TabsTrigger>
            {allowBulk ? <TabsTrigger value="bulk">Bulk unos</TabsTrigger> : null}
          </TabsList>
          <TabsContent value="quick">
            <form
              className="flex flex-1 flex-col gap-4"
              onKeyDown={handleQuickFormKeyDown}
              onSubmit={onSubmit}
            >
              <FieldGroup>
                {errors.form ? (
                  <div className="rounded-md border p-3 text-sm text-destructive">
                    {errors.form}
                  </div>
                ) : null}
                <Field data-invalid={errors.barcode ? true : undefined}>
                  <FieldLabel htmlFor="product-barcode">Barcode / SKU</FieldLabel>
                  <InputGroup>
                    <InputGroupInput
                      ref={scannerInputRef}
                      id="product-barcode"
                      aria-invalid={errors.barcode ? true : undefined}
                      autoComplete="off"
                      inputMode="numeric"
                      value={form.barcode}
                      onChange={(event) => {
                        const value = event.target.value;
                        onFormChange({
                          ...form,
                          barcode: value,
                          externalSource:
                            form.externalSource?.barcode === value.trim()
                              ? form.externalSource
                              : null,
                        });
                      }}
                      onKeyDown={handleScannerKeyDown}
                    />
                    <InputGroupAddon align="inline-end">
                      <InputGroupButton
                        aria-label="Pronađi podatke po barcode-u"
                        disabled={lookupLoading}
                        size="sm"
                        type="button"
                        onClick={onBarcodeLookup}
                      >
                        {lookupLoading ? (
                          <Spinner data-icon="inline-start" aria-hidden="true" />
                        ) : (
                          <SearchIcon data-icon="inline-start" />
                        )}
                        Pronađi
                      </InputGroupButton>
                    </InputGroupAddon>
                  </InputGroup>
                  <FieldError>{errors.barcode}</FieldError>
                </Field>
                <BarcodeLookupPanel
                  error={lookupError}
                  suggestion={lookupSuggestion}
                  source={form.externalSource}
                  onApply={onApplyLookupSuggestion}
                />
                <TextField
                  error={errors.name}
                  id="product-name"
                  label="Naziv"
                  value={form.name}
                  onChange={(value) => onFormChange({ ...form, name: value })}
                />
                <div className="grid gap-3 md:grid-cols-2">
                  <TextField
                    error={errors.sku}
                    id="product-sku"
                    label="SKU/šifra"
                    value={form.sku}
                    onChange={(value) => onFormChange({ ...form, sku: value })}
                  />
                  <TextField
                    error={errors.salePrice}
                    id="product-sale-price"
                    label="Prodajna cena sa PDV"
                    value={form.salePrice}
                    onChange={(value) =>
                      onFormChange({ ...form, salePrice: value })
                    }
                  />
                  <Field data-invalid={errors.taxRateId ? true : undefined}>
                    <FieldLabel htmlFor="product-tax-rate">PDV stopa</FieldLabel>
                    <CatalogSelect
                      id="product-tax-rate"
                      items={taxRateItems}
                      value={form.taxRateId}
                      onValueChange={(value) =>
                        onFormChange({ ...form, taxRateId: value })
                      }
                    />
                    <FieldError>{errors.taxRateId}</FieldError>
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="product-category">Kategorija</FieldLabel>
                    <CatalogSelect
                      id="product-category"
                      items={categoryItems}
                      value={form.categoryId}
                      onValueChange={(value) =>
                        onFormChange({ ...form, categoryId: value })
                      }
                    />
                  </Field>
                </div>
                <PrethodnaCenaAdvisory advisory={prethodnaCena} />
                <div className="rounded-md border p-3">
                  <div className="mb-3 text-sm font-medium">
                    Dodatna podešavanja
                  </div>
                  <FieldGroup>
                    <div className="grid gap-3 md:grid-cols-2">
                      <TextField
                        id="product-unit"
                        label="Jedinica mere"
                        value={form.unitOfMeasure}
                        onChange={(value) =>
                          onFormChange({ ...form, unitOfMeasure: value })
                        }
                      />
                      <TextField
                        error={errors.purchasePrice}
                        id="product-purchase-price"
                        label="Nabavna cena bez PDV"
                        value={form.purchasePrice}
                        onChange={(value) =>
                          onFormChange({ ...form, purchasePrice: value })
                        }
                      />
                      <TextField
                        error={errors.minimumStock}
                        id="product-minimum-stock"
                        label="Minimalni lager"
                        value={form.minimumStock}
                        onChange={(value) =>
                          onFormChange({ ...form, minimumStock: value })
                        }
                      />
                    </div>
                    <Field orientation="horizontal">
                      <Switch
                        id="product-negative-stock"
                        checked={form.allowNegativeStock}
                        onCheckedChange={(checked) =>
                          onFormChange({
                            ...form,
                            allowNegativeStock: Boolean(checked),
                          })
                        }
                      />
                      <FieldLabel htmlFor="product-negative-stock">
                        Dozvoli negativan lager
                      </FieldLabel>
                    </Field>
                    <Field orientation="horizontal">
                      <Switch
                        id="product-active"
                        checked={form.active}
                        onCheckedChange={(checked) =>
                          onFormChange({ ...form, active: Boolean(checked) })
                        }
                      />
                      <FieldLabel htmlFor="product-active">
                        Aktivan artikal
                      </FieldLabel>
                    </Field>
                  </FieldGroup>
                </div>
              </FieldGroup>
              <SheetFooter className="px-0">
                <Button type="submit" disabled={saving}>
                  {saving ? <Spinner data-icon="inline-start" /> : null}
                  Sačuvaj artikal
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => onOpenChange(false)}
                >
                  Odustani
                </Button>
              </SheetFooter>
            </form>
          </TabsContent>
          {allowBulk ? (
            <TabsContent value="bulk">
              <BulkProductEntry
                barcodeRefs={bulkBarcodeRefs}
                categoryItems={categoryItems}
                rows={bulkRows}
                saving={savingBulk}
                taxRateItems={taxRateItems}
                onAddRow={onBulkAddRow}
                onLookup={onBulkLookup}
                onRemoveRow={onBulkRemoveRow}
                onRowChange={onBulkRowChange}
                onSubmit={onBulkSubmit}
              />
            </TabsContent>
          ) : null}
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}

function BulkProductEntry({
  barcodeRefs,
  categoryItems,
  rows,
  saving,
  taxRateItems,
  onAddRow,
  onLookup,
  onRemoveRow,
  onRowChange,
  onSubmit,
}: {
  barcodeRefs: MutableRefObject<Record<string, HTMLInputElement | null>>;
  categoryItems: Array<{ label: string; value: string }>;
  rows: BulkProductRowState[];
  saving: boolean;
  taxRateItems: Array<{ label: string; value: string }>;
  onAddRow: () => void;
  onLookup: (rowId: string) => void;
  onRemoveRow: (rowId: string) => void;
  onRowChange: (
    rowId: string,
    field: BulkProductEditableField,
    value: string,
  ) => void;
  onSubmit: () => void;
}) {
  function handleKeyDown(event: ReactKeyboardEvent<HTMLFormElement>) {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
      event.preventDefault();
      onSubmit();
    }

    if (event.altKey && event.key.toLowerCase() === "n") {
      event.preventDefault();
      onAddRow();
    }
  }

  return (
    <form className="flex flex-col gap-3" onKeyDown={handleKeyDown} onSubmit={(event) => {
      event.preventDefault();
      onSubmit();
    }}>
      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12">Red</TableHead>
              <TableHead>Barcode / SKU</TableHead>
              <TableHead>Naziv</TableHead>
              <TableHead>SKU/šifra</TableHead>
              <TableHead>Cena</TableHead>
              <TableHead>PDV</TableHead>
              <TableHead>Kategorija</TableHead>
              <TableHead>Source</TableHead>
              <TableHead>Akcije</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row, index) => {
              const rowNumber = index + 1;

              return (
                <TableRow key={row.id}>
                  <TableCell className="font-medium">{rowNumber}</TableCell>
                  <TableCell className="min-w-44 align-top">
                    <InputGroup>
                      <InputGroupInput
                        ref={(element) => {
                          barcodeRefs.current[row.id] = element;
                        }}
                        aria-label={`Barcode / SKU red ${rowNumber}`}
                        aria-invalid={row.fieldErrors.barcode ? true : undefined}
                        autoComplete="off"
                        inputMode="numeric"
                        value={row.barcode}
                        onChange={(event) =>
                          onRowChange(row.id, "barcode", event.target.value)
                        }
                        onKeyDown={(event) => {
                          if (event.key === "Enter" && row.barcode.trim()) {
                            event.preventDefault();
                            onLookup(row.id);
                          }
                        }}
                      />
                      <InputGroupAddon align="inline-end">
                        <InputGroupButton
                          aria-label={`Pronađi javne podatke za red ${rowNumber}`}
                          disabled={row.lookupLoading}
                          size="icon-sm"
                          type="button"
                          onClick={() => onLookup(row.id)}
                        >
                          {row.lookupLoading ? (
                            <Spinner aria-hidden="true" />
                          ) : (
                            <SearchIcon />
                          )}
                        </InputGroupButton>
                      </InputGroupAddon>
                    </InputGroup>
                    <FieldError>{row.fieldErrors.barcode}</FieldError>
                  </TableCell>
                  <TableCell className="min-w-56 align-top">
                    <Input
                      aria-label={`Naziv red ${rowNumber}`}
                      aria-invalid={row.fieldErrors.name ? true : undefined}
                      value={row.name}
                      onChange={(event) =>
                        onRowChange(row.id, "name", event.target.value)
                      }
                    />
                    <FieldError>{row.fieldErrors.name}</FieldError>
                  </TableCell>
                  <TableCell className="min-w-40 align-top">
                    <Input
                      aria-label={`SKU/šifra red ${rowNumber}`}
                      aria-invalid={row.fieldErrors.sku ? true : undefined}
                      value={row.sku}
                      onChange={(event) =>
                        onRowChange(row.id, "sku", event.target.value)
                      }
                    />
                    <FieldError>{row.fieldErrors.sku}</FieldError>
                  </TableCell>
                  <TableCell className="min-w-32 align-top">
                    <Input
                      aria-label={`Cena red ${rowNumber}`}
                      aria-invalid={row.fieldErrors.salePrice ? true : undefined}
                      inputMode="decimal"
                      value={row.salePrice}
                      onChange={(event) =>
                        onRowChange(row.id, "salePrice", event.target.value)
                      }
                    />
                    <FieldError>{row.fieldErrors.salePrice}</FieldError>
                  </TableCell>
                  <TableCell className="min-w-36 align-top">
                    <CatalogSelect
                      ariaLabel={`PDV red ${rowNumber}`}
                      className="w-36"
                      id={`bulk-tax-rate-${row.id}`}
                      items={taxRateItems}
                      value={row.taxRateId}
                      onValueChange={(value) =>
                        onRowChange(row.id, "taxRateId", value)
                      }
                    />
                    <FieldError>{row.fieldErrors.taxRateId}</FieldError>
                  </TableCell>
                  <TableCell className="min-w-40 align-top">
                    <CatalogSelect
                      ariaLabel={`Kategorija red ${rowNumber}`}
                      className="w-40"
                      id={`bulk-category-${row.id}`}
                      items={categoryItems}
                      value={row.categoryId}
                      onValueChange={(value) =>
                        onRowChange(row.id, "categoryId", value)
                      }
                    />
                  </TableCell>
                  <TableCell className="min-w-36 align-top">
                    <div className="flex flex-col gap-1">
                      {row.externalSource ? (
                        <ExternalSourceBadge source={row.externalSource} />
                      ) : null}
                      {row.lookupError ? (
                        <FieldError>{row.lookupError}</FieldError>
                      ) : null}
                      {row.fieldErrors.form ? (
                        <FieldError>{row.fieldErrors.form}</FieldError>
                      ) : null}
                    </div>
                  </TableCell>
                  <TableCell className="align-top">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      aria-label={`Ukloni red ${rowNumber}`}
                      onClick={() => onRemoveRow(row.id)}
                    >
                      Ukloni
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </div>
      <SheetFooter className="px-0">
        <Button type="button" variant="outline" onClick={onAddRow}>
          <PlusIcon data-icon="inline-start" />
          Dodaj red
        </Button>
        <Button type="submit" disabled={saving}>
          {saving ? <Spinner data-icon="inline-start" /> : null}
          Sačuvaj sve artikle
        </Button>
      </SheetFooter>
    </form>
  );
}

const INCOMPUTABLE_PRETHODNA_CENA_MESSAGES: Record<
  NonNullable<PrethodnaCenaDto["reason"]>,
  string
> = {
  too_new_in_assortment:
    "Roba je u asortimanu kraće od 15 dana — zakon ne propisuje jasan referentni period. Unesite prethodnu cenu ručno i obrazložite.",
  not_offered_in_window: "Artikal nije bio u ponudi tokom referentnog perioda.",
  no_history: "Nema evidencije cena za ovaj artikal.",
};

/// Read-only guidance for the operator when an offered price is lowered.
///
/// Deliberately never affirms that a sniženje is lawful: ZoT čl. 38 st. 4 can
/// still bite even when the čl. 37 st. 3 arithmetic is correct. It also never
/// blocks saving.
function PrethodnaCenaAdvisory({
  advisory,
}: {
  advisory: PrethodnaCenaDto | null;
}) {
  if (!advisory) {
    return null;
  }

  if (advisory.status === "computed" && advisory.priceMinor !== null) {
    return (
      <div className="rounded-md border p-3">
        <div className="text-sm font-medium">
          Prethodna cena: {formatRsd(advisory.priceMinor)}
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          Prethodna cena izračunata prema čl. 37 st. 3. Mora biti istaknuta uz
          sniženu cenu na prodajnom mestu.
        </p>
        {advisory.truncated ? (
          <p className="mt-1 text-xs text-muted-foreground">
            Evidencija cena ne pokriva ceo period od 30 dana — proverite
            podatke.
          </p>
        ) : null}
      </div>
    );
  }

  const message = advisory.reason
    ? INCOMPUTABLE_PRETHODNA_CENA_MESSAGES[advisory.reason]
    : null;

  if (!message) {
    return null;
  }

  return (
    <div className="rounded-md border p-3">
      <p className="text-xs text-muted-foreground">{message}</p>
    </div>
  );
}

function BarcodeLookupPanel({
  error,
  suggestion,
  source,
  onApply,
}: {
  error: string | null;
  suggestion: ProductLookupSuggestion | null;
  source: ProductExternalSource | null;
  onApply: () => void;
}) {
  return (
    <div className="flex flex-col gap-2">
      {source ? (
        <div className="flex flex-wrap items-center gap-2">
          <ExternalSourceBadge source={source} />
        </div>
      ) : null}
      {error ? <FieldError>{error}</FieldError> : null}
      {suggestion ? (
        <Alert>
          <Globe2Icon aria-hidden="true" />
          <AlertTitle>Predlog sa {suggestion.source.label}</AlertTitle>
          <AlertDescription>
            <div className="flex flex-col gap-2">
              <div className="font-medium text-foreground">
                {suggestion.fields.name ?? suggestion.barcode}
              </div>
              <div className="flex flex-wrap gap-2">
                {suggestion.fields.brand ? (
                  <Badge variant="outline">{suggestion.fields.brand}</Badge>
                ) : null}
                {suggestion.fields.packageSize ? (
                  <Badge variant="outline">{suggestion.fields.packageSize}</Badge>
                ) : null}
                {suggestion.fields.categoryName ? (
                  <Badge variant="outline">{suggestion.fields.categoryName}</Badge>
                ) : null}
              </div>
              <div>
                <Button type="button" size="sm" onClick={onApply}>
                  <Globe2Icon data-icon="inline-start" />
                  Primeni javne podatke
                </Button>
              </div>
            </div>
          </AlertDescription>
        </Alert>
      ) : null}
    </div>
  );
}

function CategorySheet({
  error,
  form,
  open,
  saving,
  onFormChange,
  onOpenChange,
  onSubmit,
}: {
  error: string | null;
  form: CategoryFormState;
  open: boolean;
  saving: boolean;
  onFormChange: (form: CategoryFormState) => void;
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent>
        <SheetHeader>
          <SheetTitle>{form.id ? "Izmena kategorije" : "Nova kategorija"}</SheetTitle>
          <SheetDescription>Kategorije pomažu filtriranje artikala.</SheetDescription>
        </SheetHeader>
        <form className="flex flex-1 flex-col gap-4 px-6" onSubmit={onSubmit}>
          <FieldGroup>
            <Field data-invalid={error ? true : undefined}>
              <FieldLabel htmlFor="category-name">Naziv kategorije</FieldLabel>
              <Input
                id="category-name"
                aria-invalid={error ? true : undefined}
                value={form.name}
                onChange={(event) =>
                  onFormChange({ ...form, name: event.target.value })
                }
              />
              <FieldError>{error}</FieldError>
            </Field>
            <Field orientation="horizontal">
              <Switch
                id="category-active"
                checked={form.active}
                onCheckedChange={(checked) =>
                  onFormChange({ ...form, active: Boolean(checked) })
                }
              />
              <FieldLabel htmlFor="category-active">Aktivna kategorija</FieldLabel>
            </Field>
          </FieldGroup>
          <SheetFooter className="px-0">
            <Button type="submit" disabled={saving}>
              {saving ? <Spinner data-icon="inline-start" /> : null}
              Sačuvaj kategoriju
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Odustani
            </Button>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

function TextField({
  error,
  id,
  label,
  value,
  onChange,
}: {
  error?: string;
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <Field data-invalid={error ? true : undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input
        id={id}
        aria-invalid={error ? true : undefined}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      <FieldError>{error}</FieldError>
    </Field>
  );
}

function CatalogSelect({
  ariaLabel,
  className = "w-44",
  id,
  items,
  value,
  onValueChange,
}: {
  ariaLabel?: string;
  className?: string;
  id: string;
  items: Array<{ label: string; value: string }>;
  value: string;
  onValueChange: (value: string) => void;
}) {
  return (
    <Select
      items={items}
      value={value}
      onValueChange={(nextValue) => {
        if (nextValue) {
          onValueChange(nextValue);
        }
      }}
    >
      <SelectTrigger id={id} aria-label={ariaLabel} className={className}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          {items.map((item) => (
            <SelectItem key={item.value} value={item.value}>
              {item.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}

function StockBadge({ product }: { product: ProductSummary }) {
  const low = product.currentStockMilli <= product.minimumStockMilli;

  return low ? (
    <Badge variant="outline">{formatQuantity(product.currentStockMilli)}</Badge>
  ) : (
    <span>{formatQuantity(product.currentStockMilli)}</span>
  );
}

function ExternalSourceBadge({ source }: { source: ProductExternalSource }) {
  return (
    <Badge variant="outline">
      <Globe2Icon data-icon="inline-start" />
      {source.label}
    </Badge>
  );
}

function validateProductForm(form: ProductFormState): ProductFieldErrors {
  const errors: ProductFieldErrors = {};

  if (!form.name.trim()) {
    errors.name = "Naziv je obavezan.";
  }

  if (!form.sku.trim()) {
    errors.sku = "SKU/šifra je obavezna.";
  }

  if (!form.taxRateId) {
    errors.taxRateId = "Izaberite PDV stopu.";
  }

  try {
    if (parseRsdInput(form.salePrice) < 0) {
      errors.salePrice = "Prodajna cena ne može biti negativna.";
    }
  } catch (error) {
    errors.salePrice = commandMessage(error, "Prodajna cena nije ispravna.");
  }

  if (form.purchasePrice.trim()) {
    try {
      if (parseRsdInput(form.purchasePrice) < 0) {
        errors.purchasePrice = "Nabavna cena ne može biti negativna.";
      }
    } catch (error) {
      errors.purchasePrice = commandMessage(
        error,
        "Nabavna cena nije ispravna.",
      );
    }
  }

  try {
    if (parseQuantityInput(form.minimumStock) < 0) {
      errors.minimumStock = "Minimalni lager ne može biti negativan.";
    }
  } catch (error) {
    errors.minimumStock = commandMessage(error, "Minimalni lager nije ispravan.");
  }

  return errors;
}

function productRequest(form: ProductFormState): SaveProductRequest {
  return {
    name: form.name.trim(),
    sku: form.sku.trim(),
    barcode: form.barcode.trim() || null,
    categoryId: form.categoryId === "none" ? null : Number(form.categoryId),
    unitOfMeasure: form.unitOfMeasure.trim() || "kom",
    salePriceMinor: parseRsdInput(form.salePrice),
    purchasePriceMinor: form.purchasePrice.trim()
      ? parseRsdInput(form.purchasePrice)
      : 0,
    taxRateId: Number(form.taxRateId),
    minimumStockMilli: parseQuantityInput(form.minimumStock),
    allowNegativeStock: form.allowNegativeStock,
    active: form.active,
    externalSource: form.externalSource,
  };
}

function lookupProductDisplayName(
  fields: ProductLookupSuggestion["fields"],
) {
  const name = fields.name?.trim() ?? "";
  const brand = fields.brand?.trim() ?? "";
  const packageSize = fields.packageSize?.trim() ?? "";
  const nameWithBrand = productNameWithBrand(name, brand);

  return [nameWithBrand, packageSize].filter(Boolean).join(" ").trim();
}

function productNameWithBrand(name: string, brand: string) {
  if (!name) {
    return brand;
  }

  if (!brand) {
    return name;
  }

  return normalizedLookupText(name).startsWith(normalizedLookupText(brand))
    ? name
    : `${brand} ${name}`;
}

function acceptedLookupFields(
  fields: ProductLookupSuggestion["fields"],
  acceptsSuggestedName: boolean,
) {
  const acceptedFields: string[] = [];

  if (!acceptsSuggestedName) {
    return acceptedFields;
  }

  if (fields.name) {
    acceptedFields.push("name");
  }

  if (fields.brand) {
    acceptedFields.push("brand");
  }

  if (fields.packageSize) {
    acceptedFields.push("packageSize");
  }

  return acceptedFields;
}

function matchingCategory(
  categories: CategorySummary[],
  categoryName: string,
) {
  const normalizedCategory = normalizedLookupText(categoryName);

  return (
    categories.find(
      (category) => normalizedLookupText(category.name) === normalizedCategory,
    ) ?? null
  );
}

function normalizedLookupText(value: string) {
  return value.trim().replace(/\s+/g, " ").toLocaleLowerCase("sr-RS");
}

function defaultProductTaxRateId(taxRates: TaxRateSummary[]) {
  return taxRates.find((rate) => rate.active)?.id.toString() ?? "";
}

function createBulkProductRow(taxRateId: string): BulkProductRowState {
  bulkRowSequence += 1;

  return {
    id: `bulk-product-${bulkRowSequence}`,
    barcode: "",
    name: "",
    sku: "",
    salePrice: "",
    taxRateId,
    categoryId: "none",
    externalSource: null,
    fieldErrors: {},
    lookupError: null,
    lookupLoading: false,
  };
}

function shouldAppendBulkRowAfterScan(rows: BulkProductRowState[], rowId: string) {
  const rowIndex = rows.findIndex((row) => row.id === rowId);

  if (rowIndex === -1 || rowIndex !== rows.length - 1) {
    return false;
  }

  const row = rows[rowIndex];

  return Boolean(row.barcode.trim()) && !row.lookupLoading;
}

function hasBulkRowInput(row: BulkProductRowState) {
  return Boolean(
    row.barcode.trim() ||
      row.name.trim() ||
      row.sku.trim() ||
      row.salePrice.trim(),
  );
}

function validateBulkProductRow(row: BulkProductRowState) {
  const errors: BulkProductFieldErrors = {};

  if (!row.name.trim()) {
    errors.name = "Naziv je obavezan.";
  }

  if (!row.sku.trim()) {
    errors.sku = "SKU/šifra je obavezna.";
  }

  if (!row.taxRateId) {
    errors.taxRateId = "Izaberite PDV stopu.";
  }

  if (!row.salePrice.trim()) {
    errors.salePrice = "Prodajna cena je obavezna.";
  } else {
    try {
      if (parseRsdInput(row.salePrice) < 0) {
        errors.salePrice = "Prodajna cena ne može biti negativna.";
      }
    } catch (error) {
      errors.salePrice = commandMessage(error, "Prodajna cena nije ispravna.");
    }
  }

  return errors;
}

function hasBulkRowErrors(errors: BulkProductFieldErrors) {
  return Object.values(errors).some(Boolean);
}

function bulkProductRequest(row: BulkProductRowState): SaveProductRequest {
  return {
    name: row.name.trim(),
    sku: row.sku.trim(),
    barcode: row.barcode.trim() || null,
    categoryId: row.categoryId === "none" ? null : Number(row.categoryId),
    unitOfMeasure: "kom",
    salePriceMinor: parseRsdInput(row.salePrice),
    purchasePriceMinor: 0,
    taxRateId: Number(row.taxRateId),
    minimumStockMilli: 0,
    allowNegativeStock: false,
    active: true,
    externalSource: row.externalSource,
  };
}

function commandFieldErrors(error: unknown): ProductFieldErrors {
  if (isCommandError(error)) {
    if (error.code === "duplicate_sku") {
      return { sku: error.message };
    }

    if (error.code === "duplicate_barcode") {
      return { barcode: error.message };
    }

    return { form: error.message };
  }

  return { form: "Artikal nije sačuvan." };
}

function bulkCommandFieldErrors(error: unknown): BulkProductFieldErrors {
  if (isCommandError(error)) {
    if (error.code === "duplicate_sku") {
      return { sku: error.message };
    }

    if (error.code === "duplicate_barcode") {
      return { barcode: error.message };
    }

    return { form: error.message };
  }

  return { form: "Artikal nije sačuvan." };
}

function commandMessage(error: unknown, fallback: string) {
  if (isCommandError(error) || error instanceof Error) {
    return error.message;
  }

  return fallback;
}

function isCommandError(error: unknown): error is CommandError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error
  );
}

function minorUnitsInput(value: number) {
  return (value / 100).toLocaleString("sr-RS", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

function parseQuantityInput(input: string) {
  const parsed = Number(input.trim().replace(",", "."));

  if (!Number.isFinite(parsed)) {
    throw new Error("Količina nije ispravna.");
  }

  return Math.round(parsed * 1000);
}

function quantityInput(value: number) {
  return (value / 1000).toLocaleString("sr-RS", {
    maximumFractionDigits: 3,
  });
}

function formatQuantity(value: number) {
  const normalized = quantityInput(value);
  return `${normalized} kom`;
}

function taxRateLabel(taxRate: TaxRateSummary) {
  return `${taxRate.name} (${formatVat(taxRate.rateBasisPoints)})`;
}

function productTaxLabel(product: ProductSummary) {
  return product.taxRateName
    ? `${product.taxRateName} (${formatVat(product.taxRateBasisPoints)})`
    : formatVat(product.taxRateBasisPoints);
}

function formatVat(rateBasisPoints: number) {
  return `${(rateBasisPoints / 100).toLocaleString("sr-RS", {
    maximumFractionDigits: 2,
  })}%`;
}

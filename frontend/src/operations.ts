import type { PageSelectionValue } from "./pageSelection";

export type Operation = "split" | "pdf-image" | "merge" | "image-pdf";
export type ImageTarget = "png" | "jpeg";

export type OperationDefinition = {
  id: Operation;
  label: string;
  description: string;
};

export type FileKind = "pdf" | "image" | "unsupported";

export type FileSet = {
  files: File[];
  primaryFile: File | null;
  allPdfs: boolean;
  allImages: boolean;
  hasSinglePdf: boolean;
  hasMergeSet: boolean;
  hasImageSet: boolean;
};

const operationDefinitions: OperationDefinition[] = [
  {
    id: "pdf-image",
    label: "Export images",
    description: "Render PDF pages as PNG or JPEG files.",
  },
  { id: "split", label: "Extract pages", description: "Save selected pages as a new PDF." },
  { id: "merge", label: "Merge PDFs", description: "Combine PDFs in the order shown." },
  { id: "image-pdf", label: "Create PDF", description: "Turn images into PDF pages." },
];

export function classifyFile(file: File): FileKind {
  if (isPdfFile(file)) return "pdf";
  if (isImageFile(file)) return "image";
  return "unsupported";
}

function isPdfFile(file: File): boolean {
  return file.type === "application/pdf" || file.name.toLowerCase().endsWith(".pdf");
}

function isImageFile(file: File): boolean {
  return ["image/png", "image/jpeg"].includes(file.type) || /\.(png|jpe?g)$/i.test(file.name);
}

export function describeFileSet(files: File[]): FileSet {
  const primaryFile = files[0] ?? null;
  const allPdfs = files.length > 0 && files.every(isPdfFile);
  const allImages = files.length > 0 && files.every(isImageFile);

  return {
    files,
    primaryFile,
    allPdfs,
    allImages,
    hasSinglePdf: files.length === 1 && primaryFile ? isPdfFile(primaryFile) : false,
    hasMergeSet: files.length > 1 && allPdfs,
    hasImageSet: files.length > 1 && allImages,
  };
}

export function visibleOperationsFor(files: File[]): OperationDefinition[] {
  const fileSet = describeFileSet(files);

  if (fileSet.hasMergeSet) {
    return operationDefinitions.filter((item) => item.id === "pdf-image" || item.id === "merge");
  }

  if (fileSet.hasSinglePdf) {
    return operationDefinitions.filter((item) => item.id === "split" || item.id === "pdf-image");
  }

  if (fileSet.allImages) {
    return operationDefinitions.filter((item) => item.id === "image-pdf");
  }

  return [];
}

export function preferredOperation(files: File[]): Operation {
  return describeFileSet(files).allImages ? "image-pdf" : "pdf-image";
}

export function normalizeFileSelection(
  nextFiles: File[],
  previousOperation: Operation,
): { files: File[]; operation: Operation } | { error: string } {
  const unsupported = nextFiles.some((file) => classifyFile(file) === "unsupported");
  if (unsupported) {
    return { error: "Upload a PDF, PNG, or JPEG file." };
  }

  const fileSet = describeFileSet(nextFiles);
  if (nextFiles.length > 1 && !fileSet.allPdfs && !fileSet.allImages) {
    return { error: "Choose PDFs or images, not both in the same operation." };
  }

  const nextOperations = visibleOperationsFor(nextFiles);
  const operation = nextOperations.some((item) => item.id === previousOperation)
    ? previousOperation
    : preferredOperation(nextFiles);

  return { files: nextFiles, operation };
}

export function submitLabelFor(operation: Operation, exportPagesMode: "all" | "range"): string {
  if (operation === "split") return "Download extracted PDF";
  if (operation === "pdf-image")
    return exportPagesMode === "all" ? "Download all images" : "Download images";
  if (operation === "image-pdf") return "Create PDF";
  return "Download result";
}

export function fileSetLabel(fileSet: FileSet): string {
  return fileSet.allImages ? "Image queue" : "PDF queue";
}

export function fileSetDescription(operation: Operation): string {
  if (operation === "pdf-image") return "Images export in the order shown.";
  return "Processed in the order shown.";
}

export function buildJobForm(input: {
  operation: Operation;
  files: File[];
  targetImage: ImageTarget;
  extractPages: PageSelectionValue;
  exportPages: PageSelectionValue;
}): FormData {
  const fileSet = describeFileSet(input.files);

  if (input.operation === "merge") {
    if (!fileSet.hasMergeSet) throw new Error("Choose two or more PDFs to merge.");
    const form = new FormData();
    form.append("action", "merge");
    for (const file of input.files) form.append("files", file);
    return form;
  }

  if (input.operation === "split") {
    if (!fileSet.hasSinglePdf) throw new Error("Choose one PDF to extract pages.");
    const form = new FormData();
    form.append("action", "split");
    form.append("file", input.files[0]);
    form.append("pages", pageSelectionExpression(input.extractPages));
    return form;
  }

  const form = new FormData();
  form.append("action", "convert");

  if (input.operation === "pdf-image") {
    if (!fileSet.allPdfs) throw new Error("Choose one or more PDFs to export pages as images.");
    for (const file of input.files) form.append("files", file);
    form.append("target", input.targetImage);
    form.append("pages", pageSelectionExpression(input.exportPages));
  } else {
    if (!fileSet.allImages)
      throw new Error("Choose one or more PNG or JPEG images to create a PDF.");
    for (const file of input.files) form.append("files", file);
    form.append("target", "pdf");
    form.append("layout", "single");
  }

  return form;
}

function pageSelectionExpression(value: PageSelectionValue): string {
  return value.mode === "all" ? "all" : value.range;
}

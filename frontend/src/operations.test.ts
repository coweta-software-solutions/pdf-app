import { describe, expect, it } from "vitest";
import {
  buildJobForm,
  classifyFile,
  fileSetDescription,
  normalizeFileSelection,
  visibleOperationsFor,
} from "./operations";

const allPages = { mode: "all" as const, range: "1" };
const pageRange = { mode: "range" as const, range: "1-2" };

function file(name: string, type = "") {
  return new File(["content"], name, { type });
}

function formValues(form: FormData, key: string) {
  return form.getAll(key).map((value) => (value instanceof File ? value.name : value));
}

describe("operations", () => {
  it("recognizes PDFs by MIME type and extension", () => {
    expect(classifyFile(file("document.bin", "application/pdf"))).toBe("pdf");
    expect(classifyFile(file("document.pdf"))).toBe("pdf");
  });

  it("recognizes PNG and JPEG images by MIME type and extension", () => {
    expect(classifyFile(file("image.bin", "image/png"))).toBe("image");
    expect(classifyFile(file("image.bin", "image/jpeg"))).toBe("image");
    expect(classifyFile(file("image.png"))).toBe("image");
    expect(classifyFile(file("image.jpg"))).toBe("image");
    expect(classifyFile(file("image.jpeg"))).toBe("image");
  });

  it("returns unsupported for unsupported files", () => {
    expect(classifyFile(file("notes.txt", "text/plain"))).toBe("unsupported");
  });

  it("exposes split and pdf-image for one PDF", () => {
    expect(visibleOperationsFor([file("one.pdf")]).map((item) => item.id)).toEqual([
      "pdf-image",
      "split",
    ]);
  });

  it("exposes pdf-image and merge for multiple PDFs", () => {
    expect(visibleOperationsFor([file("one.pdf"), file("two.pdf")]).map((item) => item.id)).toEqual(
      ["pdf-image", "merge"],
    );
  });

  it("exposes image-pdf for images", () => {
    expect(
      visibleOperationsFor([file("one.png"), file("two.jpeg")]).map((item) => item.id),
    ).toEqual(["image-pdf"]);
  });

  it("rejects mixed PDF and image selections", () => {
    expect(normalizeFileSelection([file("one.pdf"), file("two.png")], "pdf-image")).toEqual({
      error: "Choose PDFs or images, not both in the same operation.",
    });
  });

  it("rejects unsupported selections", () => {
    expect(normalizeFileSelection([file("notes.txt", "text/plain")], "pdf-image")).toEqual({
      error: "Upload a PDF, PNG, or JPEG file.",
    });
  });

  it("builds merge forms with action merge and files", () => {
    const form = buildJobForm({
      operation: "merge",
      files: [file("one.pdf"), file("two.pdf")],
      targetImage: "png",
      extractPages: allPages,
      exportPages: allPages,
    });

    expect(form.get("action")).toBe("merge");
    expect(formValues(form, "files")).toEqual(["one.pdf", "two.pdf"]);
  });

  it("builds split forms with action split, file, and pages", () => {
    const form = buildJobForm({
      operation: "split",
      files: [file("one.pdf")],
      targetImage: "png",
      extractPages: pageRange,
      exportPages: allPages,
    });

    expect(form.get("action")).toBe("split");
    expect((form.get("file") as File).name).toBe("one.pdf");
    expect(form.get("pages")).toBe("1-2");
  });

  it("builds PDF-to-image forms with action convert, target, pages, and files", () => {
    const form = buildJobForm({
      operation: "pdf-image",
      files: [file("one.pdf"), file("two.pdf")],
      targetImage: "jpeg",
      extractPages: allPages,
      exportPages: pageRange,
    });

    expect(form.get("action")).toBe("convert");
    expect(form.get("target")).toBe("jpeg");
    expect(form.get("pages")).toBe("1-2");
    expect(formValues(form, "files")).toEqual(["one.pdf", "two.pdf"]);
  });

  it("describes PDF-to-image exports as ordered image output", () => {
    expect(fileSetDescription("pdf-image")).toBe("Images export in the order shown.");
  });

  it("builds image-to-PDF forms with action convert, target pdf, layout single, and files", () => {
    const form = buildJobForm({
      operation: "image-pdf",
      files: [file("one.png"), file("two.jpeg")],
      targetImage: "png",
      extractPages: allPages,
      exportPages: allPages,
    });

    expect(form.get("action")).toBe("convert");
    expect(form.get("target")).toBe("pdf");
    expect(form.get("layout")).toBe("single");
    expect(formValues(form, "files")).toEqual(["one.png", "two.jpeg"]);
  });
});

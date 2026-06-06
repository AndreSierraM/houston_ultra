import { strictEqual } from "node:assert";
import { describe, it } from "node:test";
import { shouldBlockCloudFileOpen } from "../src/lib/open-href-detect.ts";

describe("open-href cloud gate", () => {
  it("blocks local paths for cloud agents", () => {
    strictEqual(shouldBlockCloudFileOpen("report.pdf", true), true);
    strictEqual(shouldBlockCloudFileOpen("subfolder/output.docx", true), true);
  });

  it("allows external URLs for cloud agents", () => {
    strictEqual(shouldBlockCloudFileOpen("https://example.com", true), false);
    strictEqual(shouldBlockCloudFileOpen("mailto:user@example.com", true), false);
  });

  it("does not block local paths for local agents", () => {
    strictEqual(shouldBlockCloudFileOpen("report.pdf", false), false);
  });

  it("ignores blank hrefs", () => {
    strictEqual(shouldBlockCloudFileOpen("  ", true), false);
  });
});

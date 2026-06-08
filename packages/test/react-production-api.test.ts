import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

import * as ReactFramework from "../hawk2ui-react/src/index.ts";

const reactPackageJson = JSON.parse(
  readFileSync(new URL("../hawk2ui-react/package.json", import.meta.url), "utf8"),
) as { description: string };

test("React production package exposes the reconciler renderer without legacy compiler exports", () => {
  expect("createRoot" in ReactFramework).toBe(true);
  expect("compileHawkReact" in ReactFramework).toBe(false);
  expect("createHawkReactRoot" in ReactFramework).toBe(false);
});

test("React production package metadata describes the scene operation renderer", () => {
  expect(reactPackageJson.description).toContain("React 19");
  expect(reactPackageJson.description).toContain("scene operations");
  expect(reactPackageJson.description).not.toContain("native records");
});

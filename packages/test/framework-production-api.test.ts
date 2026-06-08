import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

import * as ReactFramework from "../hawk2ui-react/src/index.ts";
import * as SolidFramework from "../hawk2ui-solid/src/index.ts";
import * as VueFramework from "../hawk2ui-vue/src/index.ts";

const reactPackageJson = JSON.parse(
  readFileSync(new URL("../hawk2ui-react/package.json", import.meta.url), "utf8"),
) as { description: string };
const vuePackageJson = JSON.parse(readFileSync(new URL("../hawk2ui-vue/package.json", import.meta.url), "utf8")) as {
  description: string;
};

test("production framework entrypoints do not expose runtime-object testkit renderers", () => {
  expect("createHawkReactRoot" in ReactFramework).toBe(false);
  expect("compileHawkReact" in ReactFramework).toBe(false);
  expect("renderHawkSolid" in SolidFramework).toBe(false);
  expect("createHawkVueRenderer" in VueFramework).toBe(false);
});

test("Vue production package exposes native runtime app entrypoints", () => {
  expect("createApp" in VueFramework).toBe(true);
  expect("createHawkVueApp" in VueFramework).toBe(true);
  expect("createHawkVueRenderer" in VueFramework).toBe(false);
});

test("React production package metadata describes the scene operation renderer", () => {
  expect(reactPackageJson.description).toContain("React 19");
  expect(reactPackageJson.description).toContain("scene operations");
  expect(reactPackageJson.description).not.toContain("native records");
});

test("Vue production package metadata describes the scene operation renderer", () => {
  expect(vuePackageJson.description).toContain("Vue 3.5+");
  expect(vuePackageJson.description).toContain("scene operations");
  expect(vuePackageJson.description).not.toContain("native records");
});

import { expect, test } from "bun:test";

import * as ReactFramework from "../hawk2ui-react/src/index.ts";
import * as SolidFramework from "../hawk2ui-solid/src/index.ts";
import * as VueFramework from "../hawk2ui-vue/src/index.ts";

test("production framework entrypoints do not expose runtime-object testkit renderers", () => {
  expect("createHawkReactRoot" in ReactFramework).toBe(false);
  expect("renderHawkSolid" in SolidFramework).toBe(false);
  expect("createHawkVueRenderer" in VueFramework).toBe(false);
});

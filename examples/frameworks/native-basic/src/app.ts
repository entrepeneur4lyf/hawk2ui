import { createHawkApp } from "@hawk2ui/native";

export default createHawkApp({
  name: "native-basic",
  root: {
    id: "root",
    kind: "view",
    styleRefs: ["surface.card"],
    assetRefs: [{ name: "hawk.logo", path: "assets/logo.svg" }],
    children: [
      { id: "title", key: "title", kind: "text", props: { text: "Native Hawk2UI" } },
      { id: "cta", key: "cta", kind: "button", props: { label: "Launch" } }
    ]
  }
});

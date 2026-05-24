export type HawkElementKind = "view" | "text" | "button";

export interface HawkElementSpec {
  readonly id: string;
  readonly kind: HawkElementKind;
  readonly key?: string;
  readonly props?: Record<string, string | number | boolean>;
  readonly styleRefs?: readonly string[];
  readonly assetRefs?: readonly { name: string; path: string }[];
  readonly refs?: readonly string[];
  readonly children?: readonly HawkElementSpec[];
}

export interface HawkAppSpec {
  readonly name: string;
  readonly root: HawkElementSpec;
}

export function createHawkApp(spec: HawkAppSpec): HawkAppSpec {
  if (!spec.name.trim()) {
    throw new Error("Hawk2UI native app requires a stable name.");
  }
  return spec;
}

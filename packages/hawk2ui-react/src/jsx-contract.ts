import type { JSX } from "./jsx-runtime";
import type { HawkNativeProps } from "./nativeTypes";

type HawkIntrinsicName = keyof JSX.IntrinsicElements;

const requiredHawkIntrinsicNames: readonly HawkIntrinsicName[] = [
  "hawk-view",
  "hawk-text",
  "hawk-button",
  "hawk-input",
  "hawk-image",
  "hawk-vector",
  "hawk-surface",
  "hawk-custom-surface",
  "hawk-scroll-view",
  "hawk-list",
  "view",
  "text",
  "button",
  "input",
  "image",
  "vector",
  "custom-surface",
  "scroll-view",
  "list",
  "div",
  "span",
  "p",
];

const buttonProps: JSX.IntrinsicElements["button"] = {
  id: "cta",
  disabled: true,
  onPointerPress: "handlePress",
};

const inputProps: JSX.IntrinsicElements["input"] = {
  id: "name",
  value: "Ada",
  checked: false,
  onInput: "handleInput",
  onChange: "handleChange",
};

const viewAliasProps: JSX.IntrinsicElements["div"] = {
  id: "panel",
  role: "group",
  className: "surface.card",
};

const textAliasProps: JSX.IntrinsicElements["span"] = {
  id: "label",
  children: "Ready",
};

const hawkProps: readonly HawkNativeProps[] = [
  buttonProps,
  inputProps,
  viewAliasProps,
  textAliasProps,
];

void requiredHawkIntrinsicNames;
void hawkProps;

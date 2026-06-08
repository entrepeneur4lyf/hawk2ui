import type * as React from "react";

import type { HawkNativeProps } from "./nativeTypes.ts";

export { Fragment, jsx, jsxs } from "react/jsx-runtime";

interface HawkIntrinsicElements {
  "hawk-view": HawkNativeProps;
  "hawk-text": HawkNativeProps;
  "hawk-button": HawkNativeProps;
  "hawk-input": HawkNativeProps;
  "hawk-image": HawkNativeProps;
  "hawk-vector": HawkNativeProps;
  "hawk-surface": HawkNativeProps;
  "hawk-custom-surface": HawkNativeProps;
  "hawk-scroll-view": HawkNativeProps;
  "hawk-list": HawkNativeProps;
  view: HawkNativeProps;
  text: HawkNativeProps;
  button: HawkNativeProps;
  input: HawkNativeProps;
  image: HawkNativeProps;
  vector: HawkNativeProps;
  "custom-surface": HawkNativeProps;
  "scroll-view": HawkNativeProps;
  list: HawkNativeProps;
  div: HawkNativeProps;
  section: HawkNativeProps;
  main: HawkNativeProps;
  article: HawkNativeProps;
  header: HawkNativeProps;
  footer: HawkNativeProps;
  nav: HawkNativeProps;
  aside: HawkNativeProps;
  form: HawkNativeProps;
  label: HawkNativeProps;
  ul: HawkNativeProps;
  ol: HawkNativeProps;
  li: HawkNativeProps;
  span: HawkNativeProps;
  p: HawkNativeProps;
  strong: HawkNativeProps;
  em: HawkNativeProps;
  small: HawkNativeProps;
  code: HawkNativeProps;
  h1: HawkNativeProps;
  h2: HawkNativeProps;
  h3: HawkNativeProps;
  h4: HawkNativeProps;
  h5: HawkNativeProps;
  h6: HawkNativeProps;
}

export namespace JSX {
  export type ElementType = React.JSX.ElementType;
  export interface Element extends React.JSX.Element {}
  export interface ElementClass extends React.JSX.ElementClass {}
  export interface ElementAttributesProperty extends React.JSX.ElementAttributesProperty {}
  export interface ElementChildrenAttribute extends React.JSX.ElementChildrenAttribute {}
  export type LibraryManagedAttributes<C, P> = React.JSX.LibraryManagedAttributes<C, P>;
  export interface IntrinsicAttributes extends React.JSX.IntrinsicAttributes {}
  export interface IntrinsicClassAttributes<T> extends React.JSX.IntrinsicClassAttributes<T> {}
  export interface IntrinsicElements extends HawkIntrinsicElements {}
}

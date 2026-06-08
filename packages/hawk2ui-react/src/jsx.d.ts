import type { HawkNativeProps } from "./nativeTypes.ts";

declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
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
    }
  }
}

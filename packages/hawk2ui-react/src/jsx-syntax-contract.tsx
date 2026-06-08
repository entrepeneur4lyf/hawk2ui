/** @jsxImportSource @hawk2ui/react */
import React from "react";

const handlePress = "handlePress";

export const hawkJsxSyntaxContract = (
  <view id="root" role="group" className="surface.card">
    <text id="title">Ready</text>
    <button id="cta" disabled={false} pressed={false} onPointerPress={handlePress}>
      Go
    </button>
    <input id="name" value="Ada" checked={false} onInput="handleInput" onChange="handleChange" />
    <image id="logo" source="assets/logo.svg" />
    <vector id="icon" source="assets/icon.svg" />
    <custom-surface id="meter" surface_id="meter" />
    <scroll-view id="scroll">
      <list id="items" />
    </scroll-view>
    <div id="panel">
      <span id="caption">Caption</span>
      <p id="body">Body</p>
    </div>
  </view>
);

void React;

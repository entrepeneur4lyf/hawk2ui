type HawkMaybePromise<T> = T | Promise<T>;
type HawkJsonObject = Readonly<Record<string, unknown>>;

declare module "hawk:network" {
  export interface HawkNetworkResponse {
    readonly status?: number;
    readonly headers?: Readonly<Record<string, string>>;
    readonly body?: string;
  }

  export function request(url: string, init?: HawkJsonObject): Promise<HawkNetworkResponse>;
  const network: { readonly request: typeof request };
  export default network;
}

declare module "hawk:api" {
  export function call(name: string, payload?: unknown, options?: HawkJsonObject): Promise<unknown>;
  const api: { readonly call: typeof call };
  export default api;
}

declare module "hawk:storage" {
  export interface HawkStorageMigration {
    readonly version: number;
    readonly up: (context: {
      readonly getItem: (key: string) => HawkMaybePromise<string>;
      readonly setItem: (key: string, value: string) => HawkMaybePromise<void>;
    }) => HawkMaybePromise<void>;
  }

  export function getItem(namespace: string, key: string): HawkMaybePromise<string>;
  export function setItem(namespace: string, key: string, value: string): HawkMaybePromise<void>;
  export function migrate(namespace: string, migrations: readonly HawkStorageMigration[]): Promise<number>;
  export function getDocument(collection: string, key: string): HawkMaybePromise<unknown>;
  export function putDocument(collection: string, key: string, value: unknown): HawkMaybePromise<void>;
  export function transaction(collection: string, keys: readonly string[], callback: (documents: readonly unknown[]) => readonly unknown[]): HawkMaybePromise<readonly unknown[]>;
  const storage: {
    readonly getItem: typeof getItem;
    readonly setItem: typeof setItem;
    readonly migrate: typeof migrate;
    readonly getDocument: typeof getDocument;
    readonly putDocument: typeof putDocument;
    readonly transaction: typeof transaction;
  };
  export default storage;
}

declare module "hawk:secrets" {
  export function read(name: string): Promise<string>;
  export function serializeSecretOptions(options?: HawkJsonObject): HawkJsonObject;
}

declare module "hawk:files" {
  export function readText(path: string): HawkMaybePromise<string>;
  export function writeText(path: string, text: string): HawkMaybePromise<void>;
  export function readBytes(path: string): Uint8Array;
  export function writeBytes(path: string, bytes: Uint8Array | ArrayBuffer | readonly number[]): HawkMaybePromise<void>;
  export function pickFile(): HawkMaybePromise<string>;
  export function pickFolder(): HawkMaybePromise<string>;
  export function watch(path: string): unknown;
  export function importFile(optionsOrDestination?: HawkJsonObject | string, destinationPath?: string): HawkMaybePromise<string>;
  export function exportFile(sourcePath: string, options?: HawkJsonObject): HawkMaybePromise<string>;
}

declare module "hawk:desktop" {
  export function setWindowTitle(title: string): HawkMaybePromise<void>;
  export function showOpenDialog(options?: HawkJsonObject): Promise<readonly string[]>;
  export function readClipboard(): HawkMaybePromise<string>;
  export function writeClipboard(text: string): HawkMaybePromise<void>;
  export function notify(options: HawkJsonObject): HawkMaybePromise<void>;
  export function registerShortcut(shortcut: string): HawkMaybePromise<void>;
  export function openExternal(url: string): HawkMaybePromise<void>;
  export function nextDeepLink(): HawkMaybePromise<string>;
  export function setWindowMode(mode: string): HawkMaybePromise<void>;
  export function closeWindow(): HawkMaybePromise<void>;
}

declare module "hawk:plugin" {
  export function readParameter(parameter: string): number;
  export function writeParameter(parameter: string, value: number): void;
  export function beginAutomationGesture(parameter: string): void;
  export function endAutomationGesture(parameter: string): void;
  export function loadState(): string;
  export function saveState(stateBlob: string): void;
  export function loadPreset(presetId: string): void;
  export function savePreset(presetId: string, stateBlob: string): void;
  export function getTransport(): unknown;
  export function resizeEditor(size: { readonly width: number; readonly height: number }): void;
  export function focusEditor(): void;
}

declare module "hawk:audio" {
  export function subscribeMeters(options?: HawkJsonObject): unknown;
  export function transport(): unknown;
  export function nextControl(options?: HawkJsonObject): unknown;
}

declare module "hawk:dsp" {
  export function sendControl(message: HawkJsonObject): void;
  export function updateParameterGraph(graph?: HawkJsonObject): void;
  export function startAnalysisJob(request?: HawkJsonObject): string;
  export function cancelAnalysisJob(id: string): void;
  export function startOfflineRender(request?: HawkJsonObject): string;
  export function exportOfflineRender(id: string): string;
}

declare module "hawk:runtime" {
  export * as network from "hawk:network";
  export * as api from "hawk:api";
  export * as storage from "hawk:storage";
  export * as secrets from "hawk:secrets";
  export * as files from "hawk:files";
  export * as desktop from "hawk:desktop";
  export * as plugin from "hawk:plugin";
  export * as audio from "hawk:audio";
  export * as dsp from "hawk:dsp";
}

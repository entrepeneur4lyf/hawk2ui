# Spec 0024: Generated Framework Packages And Vue First-Class DX

## Status

Proposed implementation spec.

## Purpose

This spec defines how developers consume Hawk2UI framework adapters when `hawk2ui-cli` is the installable Rust crate and the framework implementation remains sourced from the Hawk2UI GitHub repository. It also defines the remaining work to make Vue a first-class developer option with the same package, scaffold, runtime, release, and documentation treatment as React.

## Source Truth

Suprnova establishes the intended ownership shape:

- `/home/shawn/workspace/nation-x-com/README.md:186-194` states that Suprnova distributes through GitHub, generated apps depend on the GitHub framework source, the CLI installs through `cargo install --git`, and adapter crates follow that model.
- `/home/shawn/workspace/nation-x-com/suprnova-cli/src/templates/files/backend/Cargo.toml.tpl:19-20` emits `suprnova = { git = "https://github.com/entrepeneur4lyf/suprnova.git" }` for generated full-stack apps.
- `/home/shawn/workspace/nation-x-com/suprnova-cli/src/templates/files/api/Cargo.toml.tpl:16-17` emits the same GitHub dependency for generated API apps.
- `/home/shawn/workspace/nation-x-com/suprnova-cli/Cargo.toml:43-54` keeps the CLI lightweight by avoiding a runtime dependency on the full framework and using the framework path only in dev dependencies.

Hawk2UI's JavaScript adapter shape is different from Cargo, so the direct analogue is generated npm packages whose source truth is the GitHub repository:

- `crates/hawk2ui-cli/src/executor.rs:3309-3362` owns generated React and Vue project files.
- `crates/hawk2ui-cli/src/executor.rs:3574-3594` already emits `"@hawk2ui/react": "^0.1.0"`.
- `crates/hawk2ui-cli/src/executor.rs:3774-3795` already emits `"@hawk2ui/vue": "^0.1.0"`.
- `package.json:5-11` shows `@hawk2ui/native`, `@hawk2ui/react`, `@hawk2ui/vue`, and sibling adapters as monorepo workspace packages.
- `packages/hawk2ui-react/src/legacyCompiler.ts`, `packages/hawk2ui-react/src/testkit.ts`, `packages/hawk2ui-vue/src/index.ts`, and `packages/hawk2ui-vue/src/testkit.ts` import native package values through relative monorepo paths, so they are not yet standalone publishable npm packages.

## Product Decision

`hawk2ui-cli` is the installable Rust crate and binary. The framework adapters are JavaScript packages generated from the repository and published under the `@hawk2ui/*` npm scope.

Generated projects may use normal package-manager semver dependencies such as:

```json
{
  "dependencies": {
    "@hawk2ui/vue": "^0.1.0",
    "vue": "^3.5.0"
  }
}
```

That contract is valid only after the release pipeline generates publishable packages for `@hawk2ui/native`, `@hawk2ui/react`, and `@hawk2ui/vue`; verifies the package tarballs; proves generated projects can install and build from those tarballs; and makes npm publishing an explicit release step. The repository, not npm, remains the source of truth. npm is the generated distribution channel.

## Requirements

1. The CLI remains the only installable Rust crate required by Hawk2UI app developers.
2. `@hawk2ui/native`, `@hawk2ui/react`, and `@hawk2ui/vue` must be generated as real npm packages from committed source and package metadata.
3. Generated npm packages must expose JavaScript entrypoints and type declarations that do not depend on monorepo-relative imports.
4. React and Vue packages must depend on the matching generated `@hawk2ui/native` package version when they import runtime values from it.
5. Package versions used in generated project templates must match the release version produced by the npm package generation pipeline.
6. Generated React and Vue apps must build with bun, npm, pnpm, and yarn from the generated `package.json` contract.
7. Vue app and plugin templates must remain selectable through `hawk2ui init --template vue-app` and `hawk2ui init --template vue-plugin`.
8. Vue release evidence must cover package API checks, typechecking, generated npm package installation, sealed Deno runtime execution, desktop smoke execution, and plugin smoke execution.
9. The manual must describe npm as the generated distribution channel only after code and release gates enforce package generation.
10. Development artifacts, temporary tarballs, npm publish dry-run output, and package cache contents must stay under ignored local build directories such as `target/`; committed files may include specs, release manifests, tests, package metadata, package-generation scripts, and manual updates.

## Non-Goals

- This spec does not require turning the framework adapters into Rust crates consumed by generated applications.
- This spec does not require changing application authors' UI code syntax.
- This spec does not require publishing generated tarballs from local developer machines.
- This spec does not remove Svelte or Solid adapter source from the repository.

## Acceptance Criteria

- `hawk2ui init --template react-app`, `react-plugin`, `vue-app`, and `vue-plugin` produce package metadata whose `@hawk2ui/*` dependency ranges match the generated npm package version.
- A release package-generation command produces publishable `.tgz` files for `@hawk2ui/native`, `@hawk2ui/react`, and `@hawk2ui/vue`.
- The generated tarballs contain `package.json`, JavaScript entrypoints, declaration files, and only allowed runtime/source-map assets.
- The generated tarballs do not contain tests, lockfiles, `target/`, raw release evidence, `workspace:*`, `file:`, or `link:` dependency values.
- Generated React and Vue projects can install from the locally generated tarballs and build their release bundles.
- `bun run test:react-package`, `bun run test:vue-package`, `bun run typecheck:react-package`, and `bun run typecheck:vue-package` pass after adapter imports are package-based.
- Release checks verify Vue first-class evidence alongside React evidence.
- Manual source-truth tests fail if docs claim `@hawk2ui/react` or `@hawk2ui/vue` are hand-written external packages instead of generated Hawk2UI release artifacts.

## Modular Implementation Plans

### Plan 1: Normalize Framework Package Boundaries

**Goal:** Make React and Vue adapters publishable outside the monorepo by depending on `@hawk2ui/native` through package imports.

**Files:**

- Modify `packages/hawk2ui-react/package.json`
- Modify `packages/hawk2ui-vue/package.json`
- Modify `packages/hawk2ui-react/src/legacyCompiler.ts`
- Modify `packages/hawk2ui-react/src/testkit.ts`
- Modify `packages/hawk2ui-vue/src/index.ts`
- Modify `packages/hawk2ui-vue/src/testkit.ts`
- Modify `packages/hawk2ui-compiler/src/index.ts`
- Modify `tsconfig.packages.json`
- Test `packages/test/framework-production-api.test.ts`
- Test `packages/test/package-conformance.test.ts`

**Steps:**

- [ ] Run impact analysis before editing exported package code:

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream compileHawkVue
rtk npx gitnexus impact --repo hawk2ui --direction upstream compileHawkReact
```

- [ ] Change React and Vue imports from relative native-package paths to package imports:

```ts
import {
  compilerArtifactForApp,
  recordsForApp,
  type HawkCompilerArtifact,
  type HawkElementSpec,
  type HawkEventSpec,
} from "@hawk2ui/native";
```

- [ ] Change `packages/hawk2ui-compiler/src/index.ts` from relative React/Vue package imports to package imports:

```ts
import { compileHawkReact, type HawkReactCompileOutput } from "@hawk2ui/react/legacy-compiler";
import { compileHawkVue, type HawkVueCompileOutput } from "@hawk2ui/vue/compiler";
```

- [ ] Add local TypeScript path mappings for repository tests:

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@hawk2ui/native": ["packages/hawk2ui-native/src/index.ts"],
      "@hawk2ui/react": ["packages/hawk2ui-react/src/index.ts"],
      "@hawk2ui/react/legacy-compiler": ["packages/hawk2ui-react/src/legacyCompiler.ts"],
      "@hawk2ui/vue": ["packages/hawk2ui-vue/src/index.ts"],
      "@hawk2ui/vue/compiler": ["packages/hawk2ui-vue/src/index.ts"]
    }
  }
}
```

- [ ] Add package dependency metadata that can be rewritten during generated package creation:

```json
{
  "dependencies": {
    "@hawk2ui/native": "0.1.0"
  }
}
```

- [ ] Run the focused package tests:

```bash
rtk bun run test:react-package
rtk bun run test:vue-package
rtk bun run typecheck:react-package
rtk bun run typecheck:vue-package
rtk bun run typecheck:packages
```

- [ ] Commit with:

```bash
rtk git add packages tsconfig.packages.json
rtk git commit -m "fix: make framework adapters package-portable"
```

### Plan 2: Generate Publishable Npm Package Artifacts

**Goal:** Produce clean npm package directories and `.tgz` artifacts from committed adapter source.

**Files:**

- Create `xtask/src/npm_packages.rs`
- Modify `xtask/src/main.rs`
- Modify `xtask/src/release.rs`
- Modify `packages/hawk2ui-native/package.json`
- Modify `packages/hawk2ui-react/package.json`
- Modify `packages/hawk2ui-vue/package.json`
- Create `tsconfig.npm-packages.json`
- Test `xtask/src/npm_packages.rs`

**Generated Layout:**

```text
target/npm-packages/
  native/package.json
  native/dist/index.js
  native/dist/index.d.ts
  react/package.json
  react/dist/index.js
  react/dist/index.d.ts
  vue/package.json
  vue/dist/index.js
  vue/dist/index.d.ts
  hawk2ui-native-0.1.0.tgz
  hawk2ui-react-0.1.0.tgz
  hawk2ui-vue-0.1.0.tgz
```

**Steps:**

- [ ] Run impact analysis before editing release/package-generation symbols:

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream run_release_check
```

- [ ] Add an xtask command that builds adapter packages into `target/npm-packages`.
- [ ] Generate JavaScript with the repository's existing TypeScript/Bun toolchain.
- [ ] Generate declaration files with TypeScript.
- [ ] Rewrite generated package manifests so they contain no `workspace:*`, `file:`, or `link:` values.
- [ ] Preserve package exports:

```json
{
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    }
  },
  "files": ["dist", "package.json", "README.md"]
}
```

- [ ] Add compiler/testkit subpath exports only for packages that intentionally expose those paths.
- [ ] Pack generated directories:

```bash
rtk npm pack target/npm-packages/native --pack-destination target/npm-packages
rtk npm pack target/npm-packages/react --pack-destination target/npm-packages
rtk npm pack target/npm-packages/vue --pack-destination target/npm-packages
```

- [ ] Verify each tarball exists with release-version names:

```text
target/npm-packages/hawk2ui-native-0.1.0.tgz
target/npm-packages/hawk2ui-react-0.1.0.tgz
target/npm-packages/hawk2ui-vue-0.1.0.tgz
```

- [ ] Verify tarball contents include only allowed files and do not include tests, lockfiles, source repository metadata, `target/`, or release evidence.
- [ ] Run focused checks:

```bash
rtk cargo test -p xtask npm_packages
rtk cargo run -p xtask -- npm-packages --verify
```

- [ ] Commit with:

```bash
rtk git add xtask packages tsconfig.npm-packages.json
rtk git commit -m "feat: generate Hawk2UI npm packages"
```

### Plan 3: Align CLI Templates With Generated Package Versions

**Goal:** Keep generated project package metadata tied to the generated npm package release version.

**Files:**

- Create `crates/hawk2ui-cli/src/framework_packages.rs`
- Modify `crates/hawk2ui-cli/src/lib.rs`
- Modify `crates/hawk2ui-cli/src/executor.rs`
- Test `crates/hawk2ui-cli/tests/cli_commands.rs`

**Contract:**

```rust
pub struct FrameworkPackageVersions {
    pub version: &'static str,
}

impl FrameworkPackageVersions {
    pub const fn from_cli_version() -> Self;
    pub fn dependency_range(&self) -> String;
}
```

**Steps:**

- [ ] Run impact analysis before editing scaffold symbols:

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream default_project_files
rtk npx gitnexus impact --repo hawk2ui --direction upstream react_package_json
rtk npx gitnexus impact --repo hawk2ui --direction upstream vue_package_json
```

- [ ] Add failing CLI tests that assert generated package metadata uses the CLI/package version and does not drift:

```rust
assert!(package_json.contains("\"@hawk2ui/react\": \"^0.1.0\""));
assert!(package_json.contains("\"@hawk2ui/vue\": \"^0.1.0\""));
assert!(!package_json.contains("\"@hawk2ui/vue\": \"^0.2.0\""));
```

- [ ] Implement `framework_packages.rs` with dependency range construction from `env!("CARGO_PKG_VERSION")`.
- [ ] Thread `FrameworkPackageVersions::from_cli_version()` into `react_package_json` and `vue_package_json`.
- [ ] Keep package-manager selection behavior unchanged.
- [ ] Run focused tests:

```bash
rtk cargo test -p hawk2ui-cli workspace_init_react_templates_generate_framework_manifests_and_package_metadata
rtk cargo test -p hawk2ui-cli workspace_init_vue_templates_generate_framework_manifests_and_package_metadata
rtk cargo test -p hawk2ui-cli cli_commands_parse_init_template_and_package_manager_options
```

- [ ] Commit with:

```bash
rtk git add crates/hawk2ui-cli
rtk git commit -m "fix: align scaffolds with generated framework package versions"
```

### Plan 4: Verify Generated Projects Install From Generated Packages

**Goal:** Prove generated React and Vue projects can install and build from the package artifacts that will be published to npm.

**Files:**

- Modify `crates/hawk2ui-cli/tests/cli_commands.rs`
- Modify `scripts/check-fast.sh` if the existing fast check should include the focused generated-project test
- Modify `.gitignore` only if generated local package test directories are not already ignored

**Steps:**

- [ ] Add a generated-project test helper that initializes a temp project, rewrites `@hawk2ui/*` semver ranges to local `target/npm-packages/*.tgz` paths for offline determinism, and runs install/build with the selected package manager.
- [ ] Cover React app, React plugin, Vue app, and Vue plugin templates.
- [ ] For Vue app and plugin tests, assert `src/main.ts`, `src/App.vue`, `vite.hawk.config.ts`, and package metadata are generated.
- [ ] Run focused tests:

```bash
rtk cargo test -p hawk2ui-cli generated_react_templates_install_from_generated_packages
rtk cargo test -p hawk2ui-cli generated_vue_templates_install_from_generated_packages
```

- [ ] Run package tests:

```bash
rtk bun run test:packages
rtk bun run typecheck:packages
```

- [ ] Commit with:

```bash
rtk git add crates/hawk2ui-cli scripts .gitignore
rtk git commit -m "test: verify generated framework package installs"
```

### Plan 5: Add Release And Publish Gates

**Goal:** Make package generation and npm publish dry-runs release blockers without publishing from ordinary CI.

**Files:**

- Modify `xtask/src/release.rs`
- Modify `release/release-criteria.toml`
- Modify `release/package-targets.toml`
- Modify `crates/hawk2ui-conformance/tests/verification_gates.rs`
- Modify `.github/workflows/ci.yml`

**Steps:**

- [ ] Add release criteria entries for native, React, and Vue npm package generation.
- [ ] Add package verification to `scripts/release-check.sh --packages-only`.
- [ ] Add npm publish dry-run verification:

```bash
rtk npm publish target/npm-packages/native --dry-run --access public
rtk npm publish target/npm-packages/react --dry-run --access public
rtk npm publish target/npm-packages/vue --dry-run --access public
```

- [ ] Require an explicit release-only environment variable for real publish execution, such as `HAWK2UI_NPM_PUBLISH=1`.
- [ ] Verify real publish commands are not run in normal CI or local release-check mode.
- [ ] Run focused gates:

```bash
rtk cargo test -p xtask
rtk cargo test -p hawk2ui-conformance release_criteria_execute_react_and_vue_developer_experience_gate_tests
rtk scripts/release-check.sh --packages-only
```

- [ ] Commit with:

```bash
rtk git add xtask release crates/hawk2ui-conformance .github/workflows/ci.yml
rtk git commit -m "feat: gate generated npm package releases"
```

### Plan 6: Update Manual And Source-Truth Gates

**Goal:** Update public docs only after code enforces generated npm package distribution.

**Files:**

- Modify `README.md`
- Modify `manual/getting-started.md`
- Modify `manual/runtime-apis.md`
- Modify `manual/project-manifest.md`
- Modify `manual/packaging.md`
- Modify `crates/hawk2ui-conformance/tests/manual_source_truth.rs`

**Steps:**

- [ ] Add manual source-truth assertions that fail if docs imply `@hawk2ui/react` or `@hawk2ui/vue` are manually maintained external packages.
- [ ] Document that generated projects consume npm packages generated from the Hawk2UI repository.
- [ ] Document that the CLI crate and generated npm adapter packages share the release version.
- [ ] Document that React and Vue are first-class package-manager authoring options backed by sealed Deno runtime evidence.
- [ ] Document that temporary package build outputs and tarballs are release artifacts and local verification outputs, not committed source files.
- [ ] Run manual gates:

```bash
rtk cargo test -p hawk2ui-conformance manual_source_truth
rtk cargo test -p hawk2ui-conformance manual_entrypoint
```

- [ ] Commit with:

```bash
rtk git add README.md manual crates/hawk2ui-conformance
rtk git commit -m "docs: document generated npm package consumption"
```

### Plan 7: Final Release Verification

**Goal:** Confirm generated npm packages and Vue first-class support are release-ready.

**Commands:**

```bash
rtk cargo fmt --all --check
rtk bun run test:packages
rtk bun run typecheck:packages
rtk cargo test -p hawk2ui-cli
rtk cargo test -p hawk2ui-conformance
rtk cargo test -p hawk2ui-js-runtime vue_ -- --nocapture
rtk cargo test -p hawk2ui-smoke vue_ -- --nocapture
rtk cargo run -p xtask -- npm-packages --verify
rtk scripts/release-check.sh --packages-only
rtk npx gitnexus detect-changes --repo hawk2ui --scope all
```

**Expected Result:** All checks pass. GitNexus reports only expected changes to CLI scaffolding, framework package boundaries, npm package generation, release gates, and manual truth tests.

## Open Risks

- npm publish can fail because of account, token, scope, or package-name policy. CI should run deterministic `npm publish --dry-run`; the real publish step should be explicit and release-only.
- Generated package output must not hide source-map or declaration drift. Release gates should inspect tarball contents, not only check command success.
- Existing docs already call Vue first-class in several places. Those claims remain acceptable only if the implementation plans above land and release gates continue to prove Vue runtime, generated-package, and plugin evidence.

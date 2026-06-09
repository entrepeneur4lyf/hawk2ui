# Spec 0024: GitHub Framework Consumption And Vue First-Class DX

## Status

Proposed implementation spec.

## Purpose

This spec defines how developers consume Hawk2UI framework adapters when the CLI is the installable crate and the framework implementation is sourced from GitHub. It also defines the remaining work to make Vue a first-class developer option with the same distribution, scaffold, runtime, release, and documentation treatment as React.

## Source Truth

Suprnova establishes the intended distribution shape:

- `/home/shawn/workspace/nation-x-com/README.md:186-194` states that Suprnova distributes through GitHub, generated apps depend on the GitHub framework source, the CLI installs through `cargo install --git`, and adapter crates follow that model.
- `/home/shawn/workspace/nation-x-com/suprnova-cli/src/templates/files/backend/Cargo.toml.tpl:19-20` emits `suprnova = { git = "https://github.com/entrepeneur4lyf/suprnova.git" }` for generated full-stack apps.
- `/home/shawn/workspace/nation-x-com/suprnova-cli/src/templates/files/api/Cargo.toml.tpl:16-17` emits the same GitHub dependency for generated API apps.
- `/home/shawn/workspace/nation-x-com/suprnova-cli/Cargo.toml:43-54` keeps the CLI lightweight by avoiding a runtime dependency on the full framework and using the framework path only in dev dependencies.

Hawk2UI currently diverges from that model:

- `crates/hawk2ui-cli/src/executor.rs:3309-3362` owns generated React and Vue project files.
- `crates/hawk2ui-cli/src/executor.rs:3574-3594` emits `"@hawk2ui/react": "^0.1.0"`, which assumes npm registry publication.
- `crates/hawk2ui-cli/src/executor.rs:3774-3795` emits `"@hawk2ui/vue": "^0.1.0"`, which assumes npm registry publication.
- `package.json:5-11` shows the framework adapters are monorepo workspace packages, not standalone published packages.
- `packages/hawk2ui-react/src/legacyCompiler.ts`, `packages/hawk2ui-react/src/testkit.ts`, `packages/hawk2ui-vue/src/index.ts`, and `packages/hawk2ui-vue/src/testkit.ts` currently import `hawk2ui-native` through relative monorepo paths, so package tarballs must either preserve the monorepo layout or those imports must become package imports before GitHub tarball consumption can work.

## Product Decision

`hawk2ui-cli` is the installable Rust crate and binary. React, Vue, native, and future adapter packages are framework source artifacts hosted from the Hawk2UI GitHub repository, not npm registry dependencies.

The portable JavaScript analogue to Suprnova's Cargo git dependency is GitHub-hosted adapter tarballs generated from the repository release. Generated projects must use GitHub release tarball package URLs for Hawk2UI adapters, not registry semver ranges:

```json
{
  "dependencies": {
    "@hawk2ui/native": "https://github.com/entrepeneur4lyf/hawk2ui/releases/download/v0.1.0/hawk2ui-native-0.1.0.tgz",
    "@hawk2ui/vue": "https://github.com/entrepeneur4lyf/hawk2ui/releases/download/v0.1.0/hawk2ui-vue-0.1.0.tgz",
    "vue": "^3.5.0"
  }
}
```

For unreleased local development, the CLI may accept an override source, but release builds must default to the GitHub release tag that matches `CARGO_PKG_VERSION`, formatted as `v{CARGO_PKG_VERSION}`.

## Requirements

1. The CLI must generate React and Vue package metadata without npm registry assumptions for `@hawk2ui/*`.
2. The generated framework source URL must be deterministic from CLI version unless explicitly overridden for local testing.
3. React and Vue adapters must be packable as independent tarballs from the repository while still depending on `@hawk2ui/native`.
4. Adapter source must not rely on relative imports that only work inside the monorepo checkout.
5. Generated React and Vue apps must build with bun, npm, pnpm, and yarn from the generated `package.json` contract.
6. Vue app and plugin templates must remain selectable through `hawk2ui init --template vue-app` and `hawk2ui init --template vue-plugin`.
7. Vue release evidence must cover package API checks, typechecking, sealed Deno runtime execution, desktop smoke execution, and plugin smoke execution.
8. The manual must describe the GitHub framework consumption model only after code and release gates enforce it.
9. Development artifacts, temporary tarballs, and package cache contents must stay under ignored local build directories such as `target/`; committed files may include specs, release manifests, tests, package metadata, and manual updates.

## Non-Goals

- This spec does not require publishing `@hawk2ui/*` packages to npm.
- This spec does not require turning the framework adapters into Rust crates consumed by generated applications.
- This spec does not require changing application authors' UI code syntax.
- This spec does not remove Svelte or Solid adapter source from the repository.

## Acceptance Criteria

- `hawk2ui init --template react-app` and `hawk2ui init --template vue-app` produce `package.json` files whose Hawk2UI dependencies are GitHub release tarball URLs or explicit local override URLs, never `^0.1.0` registry ranges.
- `hawk2ui init --template react-plugin` and `hawk2ui init --template vue-plugin` follow the same rule.
- `bun run test:react-package`, `bun run test:vue-package`, `bun run typecheck:react-package`, and `bun run typecheck:vue-package` pass after adapter imports are package-based.
- Release checks verify the framework package tarballs exist, are generated from committed package metadata, and contain only allowed package files.
- Release checks verify Vue first-class evidence alongside React evidence.
- Manual source-truth tests fail if the manual says registry publication is required for Hawk2UI framework adapters.

## Modular Implementation Plans

### Plan 1: Normalize Framework Package Boundaries

**Goal:** Make React and Vue adapters packable outside the monorepo by depending on `@hawk2ui/native` through package imports.

**Files:**

- Modify `packages/hawk2ui-react/package.json`
- Modify `packages/hawk2ui-vue/package.json`
- Modify `packages/hawk2ui-react/src/legacyCompiler.ts`
- Modify `packages/hawk2ui-react/src/testkit.ts`
- Modify `packages/hawk2ui-vue/src/index.ts`
- Modify `packages/hawk2ui-vue/src/testkit.ts`
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
import type {
  FrameworkCompilerOutput,
  FrameworkNativeProgram,
  HawkCompilerArtifactWire,
} from "@hawk2ui/native";
```

- [ ] Add `@hawk2ui/native` as a peer dependency in `packages/hawk2ui-react/package.json` and `packages/hawk2ui-vue/package.json`. Generated apps provide the actual GitHub tarball URL, so packed adapters must not ask package managers to resolve `@hawk2ui/native` from npm.

```json
{
  "peerDependencies": {
    "@hawk2ui/native": "0.1.0"
  }
}
```

- [ ] Add a local TypeScript path mapping for repository tests:

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@hawk2ui/native": ["packages/hawk2ui-native/src/index.ts"]
    }
  }
}
```

- [ ] Add package file allowlists to React and Vue:

```json
{
  "files": ["src"]
}
```

- [ ] Run the focused package tests:

```bash
rtk bun run test:react-package
rtk bun run test:vue-package
rtk bun run typecheck:react-package
rtk bun run typecheck:vue-package
```

- [ ] Commit with:

```bash
rtk git add packages tsconfig.packages.json
rtk git commit -m "fix: make framework adapters package-portable"
```

### Plan 2: Add A CLI Framework Source Contract

**Goal:** Centralize GitHub framework source generation so CLI templates no longer hard-code registry semver dependencies.

**Files:**

- Create `crates/hawk2ui-cli/src/framework_source.rs`
- Modify `crates/hawk2ui-cli/src/lib.rs`
- Modify `crates/hawk2ui-cli/src/executor.rs`
- Test `crates/hawk2ui-cli/tests/cli_commands.rs`

**Contract:**

```rust
pub struct FrameworkPackageSource {
    pub repo: &'static str,
    pub tag: String,
    pub version: &'static str,
}

impl FrameworkPackageSource {
    pub fn from_cli_version() -> Self;
    pub fn package_url(&self, package: FrameworkPackage) -> String;
}

pub enum FrameworkPackage {
    Native,
    React,
    Vue,
}
```

**Steps:**

- [ ] Run impact analysis before editing scaffold symbols:

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream default_project_files
rtk npx gitnexus impact --repo hawk2ui --direction upstream react_package_json
rtk npx gitnexus impact --repo hawk2ui --direction upstream vue_package_json
```

- [ ] Add failing CLI tests that assert generated React and Vue package metadata contains GitHub tarball URLs:

```rust
assert!(package_json.contains(
    "\"@hawk2ui/native\": \"https://github.com/entrepeneur4lyf/hawk2ui/releases/download/v0.1.0/hawk2ui-native-0.1.0.tgz\""
));
assert!(package_json.contains(
    "\"@hawk2ui/vue\": \"https://github.com/entrepeneur4lyf/hawk2ui/releases/download/v0.1.0/hawk2ui-vue-0.1.0.tgz\""
));
assert!(!package_json.contains("\"@hawk2ui/vue\": \"^0.1.0\""));
```

- [ ] Implement `framework_source.rs` with deterministic URL construction from `env!("CARGO_PKG_VERSION")`.
- [ ] Thread `FrameworkPackageSource::from_cli_version()` into `react_package_json` and `vue_package_json`.
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
rtk git commit -m "feat: source framework adapters from GitHub releases"
```

### Plan 3: Release-Pack Framework Adapter Tarballs

**Goal:** Make release checks prove that GitHub-hosted adapter tarballs can satisfy generated projects.

**Files:**

- Modify `xtask/src/release.rs`
- Modify `release/release-criteria.toml`
- Modify `crates/hawk2ui-conformance/tests/verification_gates.rs`
- Create `target/framework-packages/` only during local verification; do not commit generated tarballs

**Steps:**

- [ ] Run impact analysis before editing release symbols:

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream run_release_check
```

- [ ] Add an xtask package check that runs these commands from the repository root:

```bash
rtk npm pack ./packages/hawk2ui-native --pack-destination target/framework-packages
rtk npm pack ./packages/hawk2ui-react --pack-destination target/framework-packages
rtk npm pack ./packages/hawk2ui-vue --pack-destination target/framework-packages
```

- [ ] Verify each tarball exists with the exact release asset names used by CLI templates:

```text
target/framework-packages/hawk2ui-native-0.1.0.tgz
target/framework-packages/hawk2ui-react-0.1.0.tgz
target/framework-packages/hawk2ui-vue-0.1.0.tgz
```

- [ ] Verify tarball contents include `package.json` and `src/`, and exclude tests, lockfiles, `target/`, and generated release evidence.
- [ ] Verify packed adapter manifests do not contain `workspace:*`, `file:`, or `link:` dependency values.
- [ ] Add release criteria entries for React and Vue framework package tarball verification.
- [ ] Run focused gates:

```bash
rtk cargo test -p xtask
rtk cargo test -p hawk2ui-conformance release_criteria_execute_react_and_vue_developer_experience_gate_tests
rtk scripts/release-check.sh --packages-only
```

- [ ] Commit with:

```bash
rtk git add xtask release crates/hawk2ui-conformance
rtk git commit -m "feat: verify GitHub framework adapter packages"
```

### Plan 4: Verify Generated Projects Install From GitHub Sources

**Goal:** Prove generated React and Vue projects can install and build from the new package contract.

**Files:**

- Modify `crates/hawk2ui-cli/tests/cli_commands.rs`
- Modify `scripts/check-fast.sh` if the existing fast check should include the focused generated-project test
- Modify `.gitignore` only if generated local package test directories are not already ignored

**Steps:**

- [ ] Add a generated-project test helper that initializes a temp project, rewrites GitHub tarball URLs to local `target/framework-packages/*.tgz` paths for offline test determinism, and runs the selected package manager install/build command.
- [ ] Cover React app, React plugin, Vue app, and Vue plugin templates.
- [ ] For Vue app and plugin tests, assert `src/main.ts`, `src/App.vue`, `vite.hawk.config.ts`, and package metadata are generated.
- [ ] Run focused tests:

```bash
rtk cargo test -p hawk2ui-cli generated_react_templates_install_from_framework_tarballs
rtk cargo test -p hawk2ui-cli generated_vue_templates_install_from_framework_tarballs
```

- [ ] Run package tests:

```bash
rtk bun run test:packages
rtk bun run typecheck:packages
```

- [ ] Commit with:

```bash
rtk git add crates/hawk2ui-cli scripts .gitignore
rtk git commit -m "test: verify generated framework package consumption"
```

### Plan 5: Update Manual And Source-Truth Gates

**Goal:** Update public docs only after the code enforces the GitHub framework consumption model.

**Files:**

- Modify `README.md`
- Modify `manual/getting-started.md`
- Modify `manual/runtime-apis.md`
- Modify `manual/project-manifest.md`
- Modify `manual/packaging.md`
- Modify `crates/hawk2ui-conformance/tests/manual_source_truth.rs`

**Steps:**

- [ ] Add manual source-truth assertions that fail if docs claim `@hawk2ui/react` or `@hawk2ui/vue` require npm registry publication.
- [ ] Document that generated projects receive GitHub framework package URLs from the CLI.
- [ ] Document that React and Vue are first-class package-manager authoring options backed by sealed Deno runtime evidence.
- [ ] Document that temporary framework package tarballs are release artifacts and local verification outputs, not committed source files.
- [ ] Run manual gates:

```bash
rtk cargo test -p hawk2ui-conformance manual_source_truth
rtk cargo test -p hawk2ui-conformance manual_entrypoint
```

- [ ] Commit with:

```bash
rtk git add README.md manual crates/hawk2ui-conformance
rtk git commit -m "docs: document GitHub framework consumption"
```

### Plan 6: Final Release Verification

**Goal:** Confirm the distribution correction and Vue first-class path are release-ready.

**Commands:**

```bash
rtk cargo fmt --all --check
rtk bun run test:packages
rtk bun run typecheck:packages
rtk cargo test -p hawk2ui-cli
rtk cargo test -p hawk2ui-conformance
rtk cargo test -p hawk2ui-js-runtime vue_ -- --nocapture
rtk cargo test -p hawk2ui-smoke vue_ -- --nocapture
rtk scripts/release-check.sh --packages-only
rtk npx gitnexus detect-changes --repo hawk2ui --scope all
```

**Expected Result:** All checks pass. GitNexus reports only expected changes to CLI scaffolding, framework package boundaries, release gates, and manual truth tests.

## Open Risks

- GitHub release asset availability can fail transiently. Release and generated-project tests should use local tarballs for deterministic CI, while package metadata still points at the GitHub URLs used by real users.
- `npm pack` must preserve enough TypeScript source for Vite, bun, npm, pnpm, and yarn consumers. Package `files` fields should explicitly include `src` and `package.json`.
- Existing docs already call Vue first-class in several places. Those claims remain acceptable only if the implementation plans above land and release gates continue to prove Vue runtime and plugin evidence.

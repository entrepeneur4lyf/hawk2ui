# Generated Framework Packages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate real `@hawk2ui/native`, `@hawk2ui/react`, and `@hawk2ui/vue` npm packages from the Hawk2UI repository, wire generated React/Vue apps to those versions, and make Vue a first-class release-gated developer option.

**Architecture:** The repository remains the source of truth. `xtask` produces clean npm package directories and tarballs under `target/npm-packages`; release gates verify tarball contents and dry-run publishability; CLI templates depend on the generated package version. React and Vue adapter source imports `@hawk2ui/native` as a package, not by monorepo-relative paths.

**Tech Stack:** Rust `xtask`, Cargo tests, Bun package tests, TypeScript compiler, npm pack/publish dry-run, Hawk2UI CLI scaffolding, GitNexus impact checks.

---

## File Structure

- `packages/hawk2ui-native/package.json`: source package metadata; exports remain public package contract.
- `packages/hawk2ui-react/package.json`: source package metadata for React; depends on generated `@hawk2ui/native`; exposes runtime entrypoints plus the compiler/testkit subpaths listed in Task 1.
- `packages/hawk2ui-vue/package.json`: source package metadata for Vue; depends on generated `@hawk2ui/native`; exposes runtime entrypoints plus the compiler/testkit subpaths listed in Task 1.
- `packages/hawk2ui-react/src/*`: React package imports move from `../../hawk2ui-native/src/index.ts` to `@hawk2ui/native`.
- `packages/hawk2ui-vue/src/*`: Vue package imports move from `../../hawk2ui-native/src/index.ts` to `@hawk2ui/native`.
- `packages/hawk2ui-compiler/src/index.ts`: compiler package imports React/Vue through package subpaths.
- `tsconfig.packages.json`: local path aliases for package imports during repository tests.
- `tsconfig.npm-packages.json`: declaration emit configuration for generated npm package artifacts.
- `xtask/src/npm_packages.rs`: package generation, manifest rewriting, tarball verification, publish dry-run command construction.
- `xtask/src/main.rs`: exposes `npm-packages --verify`.
- `xtask/src/release.rs`: runs npm package verification in release package checks.
- `crates/hawk2ui-cli/src/framework_packages.rs`: computes generated package dependency ranges from CLI version.
- `crates/hawk2ui-cli/src/executor.rs`: uses generated package version helper for React/Vue template package metadata.
- `crates/hawk2ui-cli/tests/cli_commands.rs`: verifies scaffold package metadata and local tarball install flow.
- `release/release-criteria.toml`, `release/package-targets.toml`, `.github/workflows/ci.yml`: release and CI gates.
- `manual/*`, `README.md`, `crates/hawk2ui-conformance/tests/manual_source_truth.rs`: documentation after implementation is enforced by code.

---

### Task 1: Package Import Boundaries

**Files:**

- Modify: `packages/hawk2ui-react/src/legacyCompiler.ts`
- Modify: `packages/hawk2ui-react/src/testkit.ts`
- Modify: `packages/hawk2ui-vue/src/index.ts`
- Modify: `packages/hawk2ui-vue/src/testkit.ts`
- Modify: `packages/hawk2ui-compiler/src/index.ts`
- Modify: `packages/hawk2ui-react/package.json`
- Modify: `packages/hawk2ui-vue/package.json`
- Modify: `tsconfig.packages.json`

- [ ] **Step 1: Run impact analysis**

Run:

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream compileHawkReact
rtk npx gitnexus impact --repo hawk2ui --direction upstream compileHawkVue
rtk npx gitnexus impact --repo hawk2ui --direction upstream createHawkReactRoot
rtk npx gitnexus impact --repo hawk2ui --direction upstream createHawkVueRenderer
```

Expected: impact is limited to package tests, compiler package, conformance tests, and framework integration fixtures. Stop and report before editing if GitNexus reports HIGH or CRITICAL risk.

- [ ] **Step 2: Write failing package-boundary assertions**

Add assertions in `packages/test/package-conformance.test.ts`:

```ts
import reactPackage from "../hawk2ui-react/package.json" with { type: "json" };
import vuePackage from "../hawk2ui-vue/package.json" with { type: "json" };

test("React and Vue packages depend on native through package metadata", () => {
  expect(reactPackage.dependencies["@hawk2ui/native"]).toBe("0.1.0");
  expect(vuePackage.dependencies["@hawk2ui/native"]).toBe("0.1.0");
});
```

Add a source-shape assertion in the same file:

```ts
import { readFileSync } from "node:fs";

test("React and Vue package source does not import native through monorepo-relative paths", () => {
  for (const path of [
    "packages/hawk2ui-react/src/legacyCompiler.ts",
    "packages/hawk2ui-react/src/testkit.ts",
    "packages/hawk2ui-vue/src/index.ts",
    "packages/hawk2ui-vue/src/testkit.ts",
  ]) {
    const source = readFileSync(path, "utf8");
    expect(source).not.toContain("../../hawk2ui-native/src/index.ts");
    expect(source).toContain("@hawk2ui/native");
  }
});
```

- [ ] **Step 3: Run tests and confirm failure**

Run:

```bash
rtk bun test packages/test/package-conformance.test.ts
rtk bun run typecheck:packages
```

Expected: package conformance fails until metadata/imports change; typecheck may fail until path aliases exist.

- [ ] **Step 4: Change adapter imports**

Replace React/Vue native relative imports with package imports. The imports should follow this shape:

```ts
import {
  compilerArtifactForApp,
  recordsForApp,
  type HawkCompilerArtifact,
  type HawkCompilerDynamicBindingWire,
  type HawkCompilerDynamicValueWire,
  type HawkCompilerEventHandlerActionWire,
  type HawkCompilerEventHandlerWire,
  type HawkCompilerInitialDynamicValueWire,
  type HawkCompilerListTemplateNodeWire,
  type HawkCompilerListTemplateWire,
  type HawkCompilerReactiveBindingWire,
  type HawkCompilerTemplateScalarWire,
  type HawkElementSpec,
  type HawkEventSpec,
  type HawkLifecycleSpec,
} from "@hawk2ui/native";
```

Use narrower imports in `testkit.ts` files:

```ts
import {
  recordsForApp,
  type HawkElementSpec,
  type HawkEventSpec,
  type HawkLifecycleSpec,
} from "@hawk2ui/native";
```

- [ ] **Step 5: Change compiler package imports**

Update `packages/hawk2ui-compiler/src/index.ts`:

```ts
import { compileHawkReact, type HawkReactCompileOutput } from "@hawk2ui/react/compiler";
import { compileHawkVue, type HawkVueCompileOutput } from "@hawk2ui/vue/compiler";
```

- [ ] **Step 6: Add source package metadata**

Add `@hawk2ui/native` to React dependencies:

```json
"dependencies": {
  "@hawk2ui/native": "0.1.0",
  "react-reconciler": "0.33.0"
}
```

Add `@hawk2ui/native` to Vue dependencies:

```json
"dependencies": {
  "@babel/parser": "7.29.7",
  "@vue/compiler-dom": "3.5.35",
  "@vue/compiler-sfc": "3.5.35",
  "@hawk2ui/native": "0.1.0"
}
```

Add the React subpath exports used by repository packages and tests:

```json
"exports": {
  ".": "./src/index.ts",
  "./compiler": "./src/legacyCompiler.ts",
  "./testkit": "./src/testkit.ts",
  "./jsx-runtime": "./src/jsx-runtime.ts",
  "./jsx-dev-runtime": "./src/jsx-dev-runtime.ts"
}
```

For Vue:

```json
"exports": {
  ".": "./src/index.ts",
  "./compiler": "./src/index.ts",
  "./testkit": "./src/testkit.ts"
}
```

- [ ] **Step 7: Add TypeScript path aliases**

Update `tsconfig.packages.json`:

```json
"baseUrl": ".",
"paths": {
  "@hawk2ui/native": ["packages/hawk2ui-native/src/index.ts"],
  "@hawk2ui/react": ["packages/hawk2ui-react/src/index.ts"],
  "@hawk2ui/react/compiler": ["packages/hawk2ui-react/src/legacyCompiler.ts"],
  "@hawk2ui/react/testkit": ["packages/hawk2ui-react/src/testkit.ts"],
  "@hawk2ui/vue": ["packages/hawk2ui-vue/src/index.ts"],
  "@hawk2ui/vue/compiler": ["packages/hawk2ui-vue/src/index.ts"],
  "@hawk2ui/vue/testkit": ["packages/hawk2ui-vue/src/testkit.ts"]
}
```

- [ ] **Step 8: Verify package boundary**

Run:

```bash
rtk bun test packages/test/package-conformance.test.ts
rtk bun run test:react-package
rtk bun run test:vue-package
rtk bun run typecheck:react-package
rtk bun run typecheck:vue-package
rtk bun run typecheck:packages
```

Expected: all pass.

- [ ] **Step 9: Commit**

```bash
rtk git add packages tsconfig.packages.json
rtk git commit -m "fix: make framework packages importable"
```

---

### Task 2: Npm Package Generator

**Files:**

- Create: `xtask/src/npm_packages.rs`
- Modify: `xtask/src/main.rs`
- Create: `tsconfig.npm-packages.json`
- Modify: `packages/hawk2ui-native/package.json`
- Modify: `packages/hawk2ui-react/package.json`
- Modify: `packages/hawk2ui-vue/package.json`

- [ ] **Step 1: Run impact analysis**

Run:

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream run_command
rtk npx gitnexus impact --repo hawk2ui --direction upstream parse_command
```

Expected: affected scope is xtask command parsing tests and release command invocations.

- [ ] **Step 2: Add failing xtask parser tests**

Add tests to `xtask/src/main.rs`:

```rust
#[test]
fn parses_npm_packages_verify_command() {
    let command = parse_command(["xtask", "npm-packages", "--verify"]);
    assert_eq!(command, Ok(Command::NpmPackagesVerify));
}

#[test]
fn rejects_npm_packages_without_verify_flag() {
    let error = parse_command(["xtask", "npm-packages"]).expect_err("flag is required");
    assert!(error.contains("npm-packages requires --verify"));
}
```

Expected failure: `NpmPackagesVerify` does not exist.

- [ ] **Step 3: Add command enum variant and parser branch**

Update `xtask/src/main.rs`:

```rust
mod npm_packages;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    CheckFast,
    Check,
    ReleaseCheck(release::ReleaseCheckMode),
    NpmPackagesVerify,
}
```

Add parser branch:

```rust
"npm-packages" if rest == ["--verify"] => Ok(Command::NpmPackagesVerify),
"npm-packages" => Err(format!("npm-packages requires --verify\n{}", usage())),
```

Add runner branch:

```rust
Command::NpmPackagesVerify => return npm_packages::verify_generated_packages(),
```

Update usage:

```rust
"Usage: xtask <check-fast|check|release-check [--version-only|--packages-only|--changelog-only]|npm-packages --verify>"
```

- [ ] **Step 4: Create declaration emit config**

Create `tsconfig.npm-packages.json`:

```json
{
  "extends": "./tsconfig.packages.json",
  "compilerOptions": {
    "allowImportingTsExtensions": false,
    "declaration": true,
    "emitDeclarationOnly": true,
    "noEmit": false,
    "outDir": "target/npm-packages/types",
    "rootDir": "packages"
  },
  "include": [
    "packages/hawk2ui-native/src/**/*.ts",
    "packages/hawk2ui-react/src/**/*.ts",
    "packages/hawk2ui-react/src/**/*.tsx",
    "packages/hawk2ui-vue/src/**/*.ts"
  ]
}
```

- [ ] **Step 5: Implement package generator skeleton**

Create `xtask/src/npm_packages.rs`:

```rust
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PACKAGES: &[PackageSpec] = &[
    PackageSpec {
        id: "native",
        name: "@hawk2ui/native",
        source: "packages/hawk2ui-native",
        entries: &[EntrySpec { source: "src/index.ts", output: "index" }],
    },
    PackageSpec {
        id: "react",
        name: "@hawk2ui/react",
        source: "packages/hawk2ui-react",
        entries: &[
            EntrySpec { source: "src/index.ts", output: "index" },
            EntrySpec { source: "src/legacyCompiler.ts", output: "compiler" },
            EntrySpec { source: "src/testkit.ts", output: "testkit" },
            EntrySpec { source: "src/jsx-runtime.ts", output: "jsx-runtime" },
            EntrySpec { source: "src/jsx-dev-runtime.ts", output: "jsx-dev-runtime" },
        ],
    },
    PackageSpec {
        id: "vue",
        name: "@hawk2ui/vue",
        source: "packages/hawk2ui-vue",
        entries: &[
            EntrySpec { source: "src/index.ts", output: "index" },
            EntrySpec { source: "src/testkit.ts", output: "testkit" },
        ],
    },
];

struct PackageSpec {
    id: &'static str,
    name: &'static str,
    source: &'static str,
    entries: &'static [EntrySpec],
}

struct EntrySpec {
    source: &'static str,
    output: &'static str,
}

pub(crate) fn verify_generated_packages() -> Result<(), String> {
    let root = workspace_root();
    let out = root.join("target/npm-packages");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|error| format!("failed to clean {}: {error}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|error| format!("failed to create {}: {error}", out.display()))?;
    build_packages(&root, &out)?;
    pack_packages(&root, &out)?;
    verify_tarballs(&out)
}

fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().map_or_else(|| manifest_dir.to_path_buf(), Path::to_path_buf)
}
```

- [ ] **Step 6: Add command helpers**

Add to `xtask/src/npm_packages.rs`:

```rust
fn run(root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to run {program} {}: {error}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}
```

- [ ] **Step 7: Implement package build**

Add build steps:

```rust
fn build_packages(root: &Path, out: &Path) -> Result<(), String> {
    run(root, "bun", &[
        "x",
        "tsc",
        "-p",
        "tsconfig.npm-packages.json",
    ])?;

    for package in PACKAGES {
        let target = out.join(package.id);
        fs::create_dir_all(target.join("dist"))
            .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
        write_generated_package_json(root, out, package)?;
        build_package_javascript(root, out, package)?;
        copy_package_declarations(root, out, package)?;
    }
    Ok(())
}
```

Add JavaScript build and declaration copy helpers:

```rust
fn build_package_javascript(root: &Path, out: &Path, package: &PackageSpec) -> Result<(), String> {
    for entry in package.entries {
        let source = format!("{}/{}", package.source, entry.source);
        let outfile = out
            .join(package.id)
            .join("dist")
            .join(format!("{}.js", entry.output));
        let outfile = outfile
            .to_str()
            .ok_or("generated JavaScript path is not valid UTF-8")?
            .to_owned();
        run(root, "bun", &[
            "build",
            &source,
            "--format",
            "esm",
            "--target",
            "browser",
            "--packages",
            "external",
            "--external",
            "@hawk2ui/native",
            "--external",
            "react",
            "--external",
            "vue",
            "--outfile",
            &outfile,
        ])?;
    }
    Ok(())
}

fn copy_package_declarations(root: &Path, out: &Path, package: &PackageSpec) -> Result<(), String> {
    for entry in package.entries {
        let package_type_dir = package
            .source
            .strip_prefix("packages/")
            .unwrap_or(package.source);
        let source = root
            .join("target/npm-packages/types")
            .join(package_type_dir)
            .join(entry.source)
            .with_extension("d.ts");
        let target = out
            .join(package.id)
            .join("dist")
            .join(format!("{}.d.ts", entry.output));
        fs::copy(&source, &target).map_err(|error| {
            format!(
                "failed to copy declaration {} to {}: {error}",
                source.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}
```

- [ ] **Step 8: Implement generated manifest writing**

Generated package manifests must use this shape:

```json
{
  "name": "@hawk2ui/vue",
  "version": "0.1.0",
  "type": "module",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    },
    "./compiler": {
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    },
    "./testkit": {
      "types": "./dist/testkit.d.ts",
      "import": "./dist/testkit.js"
    }
  },
  "files": ["dist", "package.json", "README.md"],
  "dependencies": {
    "@hawk2ui/native": "0.1.0"
  },
  "peerDependencies": {
    "vue": ">=3.5"
  }
}
```

React keeps `react-reconciler` as a dependency and `react` as a peer dependency. Native has no `@hawk2ui/*` dependency.

- [ ] **Step 9: Implement pack and verification**

Add:

```rust
fn pack_packages(root: &Path, out: &Path) -> Result<(), String> {
    for package in PACKAGES {
        let package_dir = out.join(package.id);
        run(root, "npm", &[
            "pack",
            package_dir.to_str().ok_or("package path is not valid UTF-8")?,
            "--pack-destination",
            out.to_str().ok_or("output path is not valid UTF-8")?,
        ])?;
    }
    Ok(())
}

fn verify_tarballs(out: &Path) -> Result<(), String> {
    for package in PACKAGES {
        let file_name = format!("hawk2ui-{}-{VERSION}.tgz", package.id);
        let path = out.join(file_name);
        if !path.is_file() {
            return Err(format!("missing generated package tarball {}", path.display()));
        }
    }
    Ok(())
}
```

- [ ] **Step 10: Run focused tests**

Run:

```bash
rtk cargo test -p xtask parses_npm_packages_verify_command
rtk cargo test -p xtask rejects_npm_packages_without_verify_flag
rtk cargo run -p xtask -- npm-packages --verify
```

Expected: parser tests pass; package verification creates only `target/npm-packages/*`.

- [ ] **Step 11: Commit**

```bash
rtk git add xtask packages tsconfig.npm-packages.json
rtk git commit -m "feat: generate npm adapter packages"
```

---

### Task 3: CLI Template Version Contract

**Files:**

- Create: `crates/hawk2ui-cli/src/framework_packages.rs`
- Modify: `crates/hawk2ui-cli/src/lib.rs`
- Modify: `crates/hawk2ui-cli/src/executor.rs`
- Modify: `crates/hawk2ui-cli/tests/cli_commands.rs`

- [ ] **Step 1: Run impact analysis**

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream default_project_files
rtk npx gitnexus impact --repo hawk2ui --direction upstream react_package_json
rtk npx gitnexus impact --repo hawk2ui --direction upstream vue_package_json
```

- [ ] **Step 2: Add tests for generated version ranges**

In `crates/hawk2ui-cli/tests/cli_commands.rs`, extend React/Vue init tests:

```rust
let package_json = fs::read_to_string(app_root.join("package.json")).expect("package should read");
assert!(package_json.contains("\"@hawk2ui/react\": \"^0.1.0\""));
assert!(!package_json.contains("\"@hawk2ui/react\": \"^0.2.0\""));
```

For Vue:

```rust
let package_json = fs::read_to_string(app_root.join("package.json")).expect("package should read");
assert!(package_json.contains("\"@hawk2ui/vue\": \"^0.1.0\""));
assert!(!package_json.contains("\"@hawk2ui/vue\": \"^0.2.0\""));
```

- [ ] **Step 3: Add helper module**

Create `crates/hawk2ui-cli/src/framework_packages.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameworkPackageVersions {
    version: &'static str,
}

impl FrameworkPackageVersions {
    pub(crate) const fn from_cli_version() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub(crate) fn dependency_range(self) -> String {
        format!("^{}", self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_range_uses_cli_package_version() {
        assert_eq!(
            FrameworkPackageVersions::from_cli_version().dependency_range(),
            "^0.1.0"
        );
    }
}
```

- [ ] **Step 4: Wire helper into templates**

Add module in `crates/hawk2ui-cli/src/lib.rs`:

```rust
mod framework_packages;
```

Update package JSON helpers:

```rust
let framework_versions = crate::framework_packages::FrameworkPackageVersions::from_cli_version();
let framework_range = framework_versions.dependency_range();
```

Use `framework_range` for `@hawk2ui/react` and `@hawk2ui/vue`.

- [ ] **Step 5: Verify**

```bash
rtk cargo test -p hawk2ui-cli framework_packages
rtk cargo test -p hawk2ui-cli workspace_init_react_templates_generate_framework_manifests_and_package_metadata
rtk cargo test -p hawk2ui-cli workspace_init_vue_templates_generate_framework_manifests_and_package_metadata
```

- [ ] **Step 6: Commit**

```bash
rtk git add crates/hawk2ui-cli
rtk git commit -m "fix: align scaffold framework package versions"
```

---

### Task 4: Generated Project Install Verification

**Files:**

- Modify: `crates/hawk2ui-cli/tests/cli_commands.rs`
- Modify: `.gitignore`

- [ ] **Step 1: Add local tarball rewrite helper**

In `crates/hawk2ui-cli/tests/cli_commands.rs`, add a helper that rewrites generated project package metadata:

```rust
fn rewrite_hawk_package_deps_to_local_tarballs(project: &Path, package_dir: &Path) {
    let package_path = project.join("package.json");
    let mut package_json = fs::read_to_string(&package_path).expect("package should read");
    for package in ["native", "react", "vue"] {
        let dependency = format!("@hawk2ui/{package}");
        let tarball = package_dir
            .join(format!("hawk2ui-{package}-0.1.0.tgz"))
            .display()
            .to_string();
        let pattern = format!(r#""{dependency}": "^0.1.0""#);
        let replacement = format!(r#""{dependency}": "file:{tarball}""#);
        package_json = package_json.replace(&pattern, &replacement);
    }
    fs::write(package_path, package_json).expect("package should write");
}
```

- [ ] **Step 2: Add tarball generation helper**

```rust
fn generate_npm_packages() -> PathBuf {
    let status = std::process::Command::new("cargo")
        .args(["run", "-p", "xtask", "--", "npm-packages", "--verify"])
        .status()
        .expect("xtask should run");
    assert!(status.success(), "npm package generation must pass");
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("target/npm-packages")
}
```

- [ ] **Step 3: Add React generated install test**

```rust
#[test]
fn generated_react_templates_install_from_generated_packages() {
    let package_dir = generate_npm_packages();
    let app_root = temp_cli_workspace("react-generated-install");
    let created = WorkspaceCommandRunner::new(&app_root).execute(CliCommand::NewProject {
        template: CliProjectTemplate::ReactApp,
        package_manager: CliPackageManager::Npm,
    });
    assert_eq!(created.exit_code, CliExitCode::Success);
    rewrite_hawk_package_deps_to_local_tarballs(&app_root, &package_dir);
    assert!(std::process::Command::new("npm")
        .args(["install", "--ignore-scripts"])
        .current_dir(&app_root)
        .status()
        .expect("npm install should run")
        .success());
}
```

- [ ] **Step 4: Add Vue generated install test**

```rust
#[test]
fn generated_vue_templates_install_from_generated_packages() {
    let package_dir = generate_npm_packages();
    let app_root = temp_cli_workspace("vue-generated-install");
    let created = WorkspaceCommandRunner::new(&app_root).execute(CliCommand::NewProject {
        template: CliProjectTemplate::VueApp,
        package_manager: CliPackageManager::Npm,
    });
    assert_eq!(created.exit_code, CliExitCode::Success);
    assert!(app_root.join("src/main.ts").is_file());
    assert!(app_root.join("src/App.vue").is_file());
    assert!(app_root.join("vite.hawk.config.ts").is_file());
    rewrite_hawk_package_deps_to_local_tarballs(&app_root, &package_dir);
    assert!(std::process::Command::new("npm")
        .args(["install", "--ignore-scripts"])
        .current_dir(&app_root)
        .status()
        .expect("npm install should run")
        .success());
}
```

- [ ] **Step 5: Verify focused generated install path**

```bash
rtk cargo test -p hawk2ui-cli generated_react_templates_install_from_generated_packages
rtk cargo test -p hawk2ui-cli generated_vue_templates_install_from_generated_packages
```

- [ ] **Step 6: Commit**

```bash
rtk git add crates/hawk2ui-cli .gitignore
rtk git commit -m "test: verify generated package installs"
```

---

### Task 5: Release And Publish Gates

**Files:**

- Modify: `xtask/src/release.rs`
- Modify: `release/release-criteria.toml`
- Modify: `release/package-targets.toml`
- Modify: `crates/hawk2ui-conformance/tests/verification_gates.rs`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Run impact analysis**

```bash
rtk npx gitnexus impact --repo hawk2ui --direction upstream run_release_check
rtk npx gitnexus impact --repo hawk2ui --direction upstream validate_repository_package_targets
```

- [ ] **Step 2: Add package verification to packages-only release check**

In `xtask/src/release.rs`, update `run_release_check`:

```rust
ReleaseCheckMode::PackagesOnly => {
    validate_repository_package_targets()?;
    crate::npm_packages::verify_generated_packages()
}
```

- [ ] **Step 3: Add publish dry-run helper**

In `xtask/src/npm_packages.rs`, add:

```rust
pub(crate) fn verify_publish_dry_run() -> Result<(), String> {
    verify_generated_packages()?;
    let root = workspace_root();
    for package in ["native", "react", "vue"] {
        let path = root.join("target/npm-packages").join(package);
        run(&root, "npm", &[
            "publish",
            path.to_str().ok_or("publish path is not valid UTF-8")?,
            "--dry-run",
            "--access",
            "public",
        ])?;
    }
    Ok(())
}
```

- [ ] **Step 4: Add release criteria entries**

Add to `release/release-criteria.toml`:

```toml
[[checks]]
id = "generated-npm-packages"
title = "Generated npm package artifacts"
command = "rtk cargo run -p xtask -- npm-packages --verify"
evidence = "target/release-evidence/generated-npm-packages.txt"
```

- [ ] **Step 5: Add conformance gate assertions**

In `crates/hawk2ui-conformance/tests/verification_gates.rs`, assert:

```rust
assert_contains(&criteria, "id = \"generated-npm-packages\"");
assert_contains(&criteria, "cargo run -p xtask -- npm-packages --verify");
```

- [ ] **Step 6: Verify release gates**

```bash
rtk cargo test -p xtask
rtk cargo test -p hawk2ui-conformance verification_gates
rtk scripts/release-check.sh --packages-only
```

- [ ] **Step 7: Commit**

```bash
rtk git add xtask release crates/hawk2ui-conformance .github/workflows/ci.yml
rtk git commit -m "feat: gate generated npm packages"
```

---

### Task 6: Manual And Source-Truth Update

**Files:**

- Modify: `README.md`
- Modify: `manual/getting-started.md`
- Modify: `manual/runtime-apis.md`
- Modify: `manual/project-manifest.md`
- Modify: `manual/packaging.md`
- Modify: `crates/hawk2ui-conformance/tests/manual_source_truth.rs`

- [ ] **Step 1: Add failing manual source-truth checks**

In `manual_source_truth.rs`, add assertions:

```rust
#[test]
fn manual_documents_generated_npm_framework_packages() {
    let getting_started = read_workspace_file("manual/getting-started.md");
    let packaging = read_workspace_file("manual/packaging.md");
    for required in [
        "generated `@hawk2ui/react` and `@hawk2ui/vue` npm packages",
        "`hawk2ui-cli` remains the installable Rust CLI",
        "generated from the Hawk2UI repository",
    ] {
        assert!(
            getting_started.contains(required) || packaging.contains(required),
            "manual missing generated npm package claim: {required}"
        );
    }
}
```

- [ ] **Step 2: Update docs**

Document these exact claims:

```md
`hawk2ui-cli` is the installable Rust CLI. React and Vue projects consume generated `@hawk2ui/react` and `@hawk2ui/vue` npm packages. Those packages are built from the Hawk2UI repository during release; npm is the distribution channel, not a separate source of truth.
```

Add packaging text:

```md
Release packaging generates `@hawk2ui/native`, `@hawk2ui/react`, and `@hawk2ui/vue` package tarballs under `target/npm-packages`, verifies tarball contents, and runs npm publish dry-run checks. Generated tarballs and package staging directories are release artifacts and are not committed.
```

- [ ] **Step 3: Verify manual gates**

```bash
rtk cargo test -p hawk2ui-conformance manual_source_truth
rtk cargo test -p hawk2ui-conformance manual_entrypoint
```

- [ ] **Step 4: Commit**

```bash
rtk git add README.md manual crates/hawk2ui-conformance
rtk git commit -m "docs: document generated npm package consumption"
```

---

### Task 7: Final Verification And Push

**Files:**

- Verify only; create a follow-up fix commit if any command fails.

- [ ] **Step 1: Run full focused verification**

```bash
rtk cargo fmt --all --check
rtk bun run test:packages
rtk bun run typecheck:packages
rtk cargo test -p xtask
rtk cargo test -p hawk2ui-cli
rtk cargo test -p hawk2ui-conformance
rtk cargo test -p hawk2ui-js-runtime vue_ -- --nocapture
rtk cargo test -p hawk2ui-smoke vue_ -- --nocapture
rtk cargo run -p xtask -- npm-packages --verify
rtk scripts/release-check.sh --packages-only
```

- [ ] **Step 2: Run GitNexus changed-scope review**

```bash
rtk npx gitnexus detect-changes --repo hawk2ui --scope all
```

Expected: affected scope includes package adapters, CLI scaffolding, npm package generation, release gates, and manual truth checks only.

- [ ] **Step 3: Inspect public repo artifacts**

Run:

```bash
rtk git status --short
rtk git ls-files target npm-debug.log package-lock.json pnpm-lock.yaml yarn.lock
```

Expected: no generated tarballs, no target files, no package-manager debug logs, and no generated lockfiles staged or tracked from test projects.

- [ ] **Step 4: Push**

```bash
rtk git push
```

Expected: branch pushes cleanly to `origin/main`.

---

## Execution Notes

- Use `apply_patch` for all manual file edits.
- Keep generated package directories and tarballs in `target/npm-packages`.
- Do not commit generated tarballs, generated install sandboxes, npm debug logs, or package-manager lockfiles created inside temp projects.
- Run `rtk npx gitnexus analyze` if GitNexus reports the index is stale before impact or changed-scope checks.

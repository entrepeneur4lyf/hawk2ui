use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

const PACKAGES: &[PackageSpec] = &[
    PackageSpec {
        id: "native",
        name: "@hawk2ui/native",
        source: "packages/hawk2ui-native",
        entries: &[EntrySpec {
            source: "src/index.ts",
            output: "index",
        }],
    },
    PackageSpec {
        id: "react",
        name: "@hawk2ui/react",
        source: "packages/hawk2ui-react",
        entries: &[
            EntrySpec {
                source: "src/index.ts",
                output: "index",
            },
            EntrySpec {
                source: "src/legacyCompiler.ts",
                output: "compiler",
            },
            EntrySpec {
                source: "src/testkit.ts",
                output: "testkit",
            },
            EntrySpec {
                source: "src/jsx-runtime.ts",
                output: "jsx-runtime",
            },
            EntrySpec {
                source: "src/jsx-dev-runtime.ts",
                output: "jsx-dev-runtime",
            },
        ],
    },
    PackageSpec {
        id: "vue",
        name: "@hawk2ui/vue",
        source: "packages/hawk2ui-vue",
        entries: &[
            EntrySpec {
                source: "src/index.ts",
                output: "index",
            },
            EntrySpec {
                source: "src/testkit.ts",
                output: "testkit",
            },
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
    let version = cli_package_version(&root)?;
    let out = root.join("target/npm-packages");

    if out.exists() {
        fs::remove_dir_all(&out)
            .map_err(|error| format!("failed to clean {}: {error}", out.display()))?;
    }
    fs::create_dir_all(&out)
        .map_err(|error| format!("failed to create {}: {error}", out.display()))?;

    build_packages(&root, &out, &version)?;
    pack_packages(&root, &out)?;
    verify_tarballs(&out, &version)
}

fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map_or_else(|| manifest_dir.to_path_buf(), Path::to_path_buf)
}

#[derive(Deserialize)]
struct CargoManifest {
    package: CargoPackage,
}

#[derive(Deserialize)]
struct CargoPackage {
    version: String,
}

fn cli_package_version(root: &Path) -> Result<String, String> {
    let manifest_path = root.join("crates/hawk2ui-cli/Cargo.toml");
    let source = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let manifest: CargoManifest = toml::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", manifest_path.display()))?;
    Ok(manifest.package.version)
}

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

fn build_packages(root: &Path, out: &Path, version: &str) -> Result<(), String> {
    run(
        root,
        "bun",
        &["x", "tsc", "-p", "tsconfig.npm-packages.json"],
    )?;

    for package in PACKAGES {
        let target = out.join(package.id);
        fs::create_dir_all(target.join("dist"))
            .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
        write_generated_package_json(out, package, version)?;
        build_package_javascript(root, out, package)?;
        copy_package_declarations(root, out, package)?;
    }
    Ok(())
}

fn build_package_javascript(root: &Path, out: &Path, package: &PackageSpec) -> Result<(), String> {
    for entry in package.entries {
        let source = format!("{}/{}", package.source, entry.source);
        let outfile = out
            .join(package.id)
            .join("dist")
            .join(format!("{}.js", entry.output));
        let outfile = path_to_str(&outfile, "generated JavaScript path")?.to_owned();
        run(
            root,
            "bun",
            &[
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
            ],
        )?;
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

fn write_generated_package_json(
    out: &Path,
    package: &PackageSpec,
    version: &str,
) -> Result<(), String> {
    let manifest = match package.id {
        "native" => format!(
            r#"{{
  "name": "{name}",
  "version": "{version}",
  "type": "module",
  "exports": {{
    ".": {{
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    }}
  }},
  "files": ["dist", "package.json"],
  "description": "Direct native authoring package for Hawk2UI records."
}}
"#,
            name = package.name,
            version = version
        ),
        "react" => format!(
            r#"{{
  "name": "{name}",
  "version": "{version}",
  "type": "module",
  "exports": {{
    ".": {{
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    }},
    "./compiler": {{
      "types": "./dist/compiler.d.ts",
      "import": "./dist/compiler.js"
    }},
    "./testkit": {{
      "types": "./dist/testkit.d.ts",
      "import": "./dist/testkit.js"
    }},
    "./jsx-runtime": {{
      "types": "./dist/jsx-runtime.d.ts",
      "import": "./dist/jsx-runtime.js"
    }},
    "./jsx-dev-runtime": {{
      "types": "./dist/jsx-dev-runtime.d.ts",
      "import": "./dist/jsx-dev-runtime.js"
    }}
  }},
  "files": ["dist", "package.json"],
  "dependencies": {{
    "@hawk2ui/native": "{version}",
    "react-reconciler": "0.33.0"
  }},
  "peerDependencies": {{
    "react": ">=19"
  }},
  "description": "React 19 custom renderer integration for Hawk2UI scene operations."
}}
"#,
            name = package.name,
            version = version
        ),
        "vue" => format!(
            r#"{{
  "name": "{name}",
  "version": "{version}",
  "type": "module",
  "exports": {{
    ".": {{
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    }},
    "./compiler": {{
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    }},
    "./testkit": {{
      "types": "./dist/testkit.d.ts",
      "import": "./dist/testkit.js"
    }}
  }},
  "files": ["dist", "package.json"],
  "dependencies": {{
    "@babel/parser": "7.29.7",
    "@vue/compiler-dom": "3.5.35",
    "@vue/compiler-sfc": "3.5.35",
    "@hawk2ui/native": "{version}"
  }},
  "peerDependencies": {{
    "vue": ">=3.5"
  }},
  "description": "Vue 3.5+ native runtime renderer for Hawk2UI scene operations."
}}
"#,
            name = package.name,
            version = version
        ),
        other => return Err(format!("unknown package id {other}")),
    };

    let path = out.join(package.id).join("package.json");
    fs::write(&path, manifest)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn pack_packages(root: &Path, out: &Path) -> Result<(), String> {
    let out_str = path_to_str(out, "output path")?;
    for package in PACKAGES {
        let package_dir = out.join(package.id);
        let package_dir = path_to_str(&package_dir, "package path")?;
        run(
            root,
            "npm",
            &["pack", package_dir, "--pack-destination", out_str],
        )?;
    }
    Ok(())
}

fn verify_tarballs(out: &Path, version: &str) -> Result<(), String> {
    for package in PACKAGES {
        let file_name = format!("hawk2ui-{}-{version}.tgz", package.id);
        let path = out.join(file_name);
        if !path.is_file() {
            return Err(format!(
                "missing generated package tarball {}",
                path.display()
            ));
        }
        verify_generated_manifest(out, package)?;
        verify_tarball_contents(&path)?;
    }
    Ok(())
}

fn verify_generated_manifest(out: &Path, package: &PackageSpec) -> Result<(), String> {
    let path = out.join(package.id).join("package.json");
    let manifest = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for forbidden in ["workspace:", "file:", "link:", "packages/", "target/"] {
        if manifest.contains(forbidden) {
            return Err(format!(
                "{} contains forbidden value {forbidden}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn verify_tarball_contents(path: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .args(["-tzf", path_to_str(path, "tarball path")?])
        .output()
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if !output.status.success() {
        return Err(format!("tar inspection failed for {}", path.display()));
    }
    let listing = String::from_utf8(output.stdout)
        .map_err(|error| format!("tar listing for {} was not UTF-8: {error}", path.display()))?;
    for required in [
        "package/package.json",
        "package/dist/index.js",
        "package/dist/index.d.ts",
    ] {
        if !listing.lines().any(|line| line == required) {
            return Err(format!(
                "{} missing required entry {required}",
                path.display()
            ));
        }
    }
    for forbidden in [
        "/test/",
        ".test.",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "target/",
        "release-evidence",
    ] {
        if listing.contains(forbidden) {
            return Err(format!(
                "{} contains forbidden entry matching {forbidden}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn path_to_str<'a>(path: &'a Path, name: &str) -> Result<&'a str, String> {
    path.to_str()
        .ok_or_else(|| format!("{name} is not valid UTF-8: {}", path.display()))
}

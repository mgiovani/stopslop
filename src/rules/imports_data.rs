//! SLOP010 support data: `DepIndex` (per-language declared+builtin package/module names,
//! discovered from manifests under the scanned roots) plus the embedded stdlib/alias/builtin
//! lists from importdata.md. `imports.rs` does AST extraction only; all resolution logic
//! (stdlib/alias/normalization) lives here so the two files stay disjoint in concern.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Per-language declared+builtin package/module names, discovered from manifests under `roots`.
#[derive(Debug, Default, Clone)]
pub struct DepIndex {
    pub ts: HashSet<String>, // package.json dependency names (bare + @scope/pkg)
    pub ts_path_aliases: HashSet<String>, // tsconfig.json compilerOptions.paths keys, "/*" stripped
    pub python: HashSet<String>, // PEP503-normalized declared package names
    pub go: HashSet<String>, // go.mod module path + require entries
    pub rust: HashSet<String>, // Cargo.toml dep keys + own package name, '-'->'_'
}

impl DepIndex {
    pub fn discover(roots: &[PathBuf]) -> DepIndex {
        let mut manifests = Vec::new();
        for root in roots {
            find_manifests(root, &mut manifests);
        }
        let mut idx = DepIndex::default();
        for path in manifests {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            match path.file_name().and_then(|n| n.to_str()).unwrap_or("") {
                "pyproject.toml" => idx.python.extend(parse_pyproject(&text)),
                "requirements.txt" => idx.python.extend(parse_requirements(&text)),
                "package.json" => idx.ts.extend(parse_package_json(&text)),
                "tsconfig.json" => idx.ts_path_aliases.extend(parse_tsconfig(&text)),
                "go.mod" => idx.go.extend(parse_go_mod(&text)),
                "Cargo.toml" => idx.rust.extend(parse_cargo_toml(&text)),
                _ => {}
            }
        }
        idx
    }

    pub fn empty() -> Self {
        DepIndex::default()
    }
}

// ponytail: bounded recursive dir walk instead of pulling in a manifest-discovery crate; fixture
// trees are tiny and this only runs once per invocation under --check-imports.
const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "vendor"];
const MANIFEST_NAMES: &[&str] = &[
    "pyproject.toml",
    "requirements.txt",
    "package.json",
    "tsconfig.json",
    "go.mod",
    "Cargo.toml",
];

fn find_manifests(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        if is_manifest_name(root) {
            out.push(root.to_path_buf());
        }
        // A file argument has no subtree to search, so climb instead: `stopslop --check-imports
        // src/a.ts` (pre-commit hooks, `git diff --name-only | xargs`) would otherwise resolve
        // against an empty dep index and silently flag nothing at all.
        for dir in root.ancestors().skip(1) {
            for name in MANIFEST_NAMES {
                let p = dir.join(name);
                if p.is_file() {
                    out.push(p);
                }
            }
            if dir.join(".git").exists() {
                break; // repo root: manifests above this belong to someone else's project
            }
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    // `entry.file_type()` reads the type `readdir` already returned; `entry.path().is_dir()`
    // allocated a `PathBuf` and issued a `stat(2)` for every entry in the tree instead. The one
    // behavioral difference is that a symlinked DIRECTORY is no longer descended, which is what
    // the lint walk itself does (`ignore`'s `follow_links` is off) -- a symlinked manifest FILE
    // still matches, since it lands in the name check below either way.
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if file_type.is_dir() {
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            find_manifests(&entry.path(), out);
        } else if MANIFEST_NAMES.contains(&name) {
            out.push(entry.path());
        }
    }
}

fn is_manifest_name(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| MANIFEST_NAMES.contains(&n))
}

// --- normalization ---

/// PEP 503: lowercase, runs of `-`/`_`/`.` collapse to a single `-` for matching purposes.
/// (Simplified to a 1:1 char map — good enough since inputs are already single separators.)
fn pep503(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c == '_' || c == '.' { '-' } else { c })
        .collect()
}

// --- Python: pyproject.toml / requirements.txt ---

fn extract_pep508_name(spec: &str) -> Option<String> {
    let end = spec
        .find(|c: char| "[;<>=!@ ".contains(c))
        .unwrap_or(spec.len());
    let name = spec[..end].trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn parse_pyproject(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(doc) = text.parse::<toml::Value>() else {
        return set;
    };
    let project = doc.get("project");
    if let Some(deps) = project
        .and_then(|p| p.get("dependencies"))
        .and_then(|v| v.as_array())
    {
        for d in deps {
            if let Some(name) = d.as_str().and_then(extract_pep508_name) {
                set.insert(pep503(&name));
            }
        }
    }
    if let Some(groups) = project
        .and_then(|p| p.get("optional-dependencies"))
        .and_then(|v| v.as_table())
    {
        for arr in groups.values().filter_map(|v| v.as_array()) {
            for d in arr {
                if let Some(name) = d.as_str().and_then(extract_pep508_name) {
                    set.insert(pep503(&name));
                }
            }
        }
    }
    set
}

fn parse_requirements(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        if let Some(idx) = line.find("#egg=") {
            if let Some(name) = extract_pep508_name(&line[idx + 5..]) {
                set.insert(pep503(&name));
            }
            continue;
        }
        if line.contains("://") {
            continue; // VCS url w/o #egg=: can't resolve a name, skip (no FP)
        }
        if let Some(name) = extract_pep508_name(line) {
            set.insert(pep503(&name));
        }
    }
    set
}

// --- Node/TS: package.json / tsconfig.json ---

fn parse_package_json(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return set;
    };
    for field in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(obj) = v.get(field).and_then(|x| x.as_object()) {
            set.extend(obj.keys().cloned());
        }
    }
    set
}

fn parse_tsconfig(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return set;
    };
    if let Some(paths) = v
        .get("compilerOptions")
        .and_then(|c| c.get("paths"))
        .and_then(|p| p.as_object())
    {
        for k in paths.keys() {
            let alias = k.trim_end_matches("/*").trim_end_matches('*');
            if !alias.is_empty() {
                set.insert(alias.to_string());
            }
        }
    }
    set
}

// --- Go: go.mod ---

fn go_mod_require_path(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") {
        return None;
    }
    line.split_whitespace().next().map(|s| s.to_string())
}

// ponytail: `replace` directives ignored — the module-self + require set covers the fixtures
// and the common non-monorepo case; add replace-target handling only if it FPs in practice.
fn parse_go_mod(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut in_require_block = false;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if let Some(rest) = line.strip_prefix("module ") {
            set.insert(rest.trim().to_string());
            continue;
        }
        if line.starts_with("require (") {
            in_require_block = true;
            continue;
        }
        if in_require_block {
            if line.starts_with(')') {
                in_require_block = false;
            } else if let Some(p) = go_mod_require_path(line) {
                set.insert(p);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("require ") {
            if let Some(p) = go_mod_require_path(rest) {
                set.insert(p);
            }
        }
    }
    set
}

// --- Rust: Cargo.toml ---

fn add_dep_table_keys(table: Option<&toml::Value>, out: &mut HashSet<String>) {
    if let Some(t) = table.and_then(|v| v.as_table()) {
        out.extend(t.keys().map(|k| k.replace('-', "_")));
    }
}

fn parse_cargo_toml(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(doc) = text.parse::<toml::Value>() else {
        return set;
    };
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        add_dep_table_keys(doc.get(section), &mut set);
    }
    if let Some(ws) = doc.get("workspace") {
        add_dep_table_keys(ws.get("dependencies"), &mut set);
    }
    if let Some(target) = doc.get("target").and_then(|v| v.as_table()) {
        for cfg_table in target.values() {
            for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                add_dep_table_keys(cfg_table.get(section), &mut set);
            }
        }
    }
    if let Some(name) = doc
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
    {
        set.insert(name.replace('-', "_"));
    }
    set
}

// --- embedded lists (importdata.md) ---

const PY_STDLIB_RAW: &str =
    "__future__ __main__ _aix_support _ast _asyncio _bisect _blake2 _codecs _codecs_iso2022 \
_codecs_jp _codecs_kr _codecs_tw _collections _colorsys _compat_pickle _crypt _csv _ctypes \
_curses _curses_panel _dataclasses _dbm _decimal _elementtree _functools _gdbm _hashlib _heapq \
_imp _importlib _io _json _locale _lsprof _lzma _markupbase _md5 _mmap _multibytecodec \
_multiprocessing _opcode _operator _osx_support _overlapped _pickle _posixsubprocess _posixshmem \
_queue _random _readline _scproxy _select _selectors _sha1 _sha256 _sha3 _sha512 _signal _site \
_socket _sqlite3 _ssl _stat _statistics _string _strptime _struct _symtable _sysconfig \
_testbuffer _testcapi _testclinic _testhelp _testimportmultiple _testinternalcapi \
_testlimitedcapi _testmultiphase _thread _threading_local _tkinter _tracemalloc _warnings \
_weakref _weakrefset _winreg abc aifc argparse array ast asynchat asyncio asyncore atexit \
audioop base64 bdb binascii binhex bisect builtins bz2 cProfile calendar cgi cgitb chunk cmath \
cmd code codecs codeop collections colorsys compileall concurrent configparser contextlib \
contextvars copy copyreg crypt csv ctypes curses dataclasses datetime dbm decimal difflib dis \
distutils doctest dummy_threading email encodings enum errno faulthandler fcntl filecmp \
fileinput fnmatch ftplib functools gc getopt getpass gettext glob grp gzip hashlib heapq hmac \
html http idlelib imaplib imghdr imp importlib inspect io ipaddress itertools json keyword \
lib2to3 linecache locale logging lzma mailbox mailcap marshal math mimetypes mmap modulefinder \
msilib msvcrt multiprocessing netrc nis nntplib numbers operator optparse os ossaudiodev parser \
pathlib pdb pickle pickletools pipes pkgutil platform plistlib poplib posix posixpath pprint \
profile pstats pty pwd py_compile pyclbr pydoc pyexpat queue quopri random re readline reprlib \
resource rlcompleter runpy sched secrets select selectors shelve shlex shutil signal site smtpd \
smtplib sndhdr socket socketserver spwd sqlite3 ssl stat statistics string stringprep struct \
subprocess sunau symbol symtable sys sysconfig syslog tabnanny tarfile telnetlib tempfile \
termios test textwrap threading time timeit tkinter token tokenize trace traceback tracemalloc \
tty turtle types typing typing_extensions unicodedata unittest urllib uu uuid venv warnings \
wave weakref webbrowser winreg winsound wsgiref xdrlib xml xmlrpc zipapp zipfile zipimport zlib";

static PY_STDLIB: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| PY_STDLIB_RAW.split_whitespace().collect());

/// import-name -> PyPI package-name for the common mismatches (importdata.md top-25).
/// `"google-cloud"` is a sentinel: many `google-cloud-*` sub-packages exist, so we accept any
/// declared dep whose normalized name starts with `google-`.
static PY_ALIASES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        ("PIL", "pillow"),
        ("cv2", "opencv-python"),
        ("sklearn", "scikit-learn"),
        ("bs4", "beautifulsoup4"),
        ("yaml", "PyYAML"),
        ("dotenv", "python-dotenv"),
        ("dateutil", "python-dateutil"),
        ("magic", "python-magic"),
        ("setuptools_scm", "setuptools-scm"),
        ("pkg_resources", "setuptools"),
        ("jwt", "PyJWT"),
        ("google", "google-cloud"),
    ])
});

const NODE_BUILTINS_RAW: &str =
    "assert buffer child_process cluster console crypto dgram dns domain \
events fs http http2 https inspector module net os path \
punycode querystring readline repl stream string_decoder sys timers \
tls trace_events tty url util v8 vm wasi worker_threads zlib";

static NODE_BUILTINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut set: HashSet<&'static str> = NODE_BUILTINS_RAW.split_whitespace().collect();
    set.insert("fs/promises");
    set
});

const RUST_BUILTINS: &[&str] = &["std", "core", "alloc", "proc_macro", "test"];

// --- resolution (used by imports.rs) ---

pub fn python_resolved(idx: &DepIndex, raw_name: &str) -> bool {
    if PY_STDLIB.contains(raw_name) {
        return true;
    }
    if let Some(&pkg) = PY_ALIASES.get(raw_name) {
        if pkg == "google-cloud" {
            return idx.python.iter().any(|d| d.starts_with("google-"));
        }
        return idx.python.contains(&pep503(pkg));
    }
    idx.python.contains(&pep503(raw_name))
}

/// `@scope/pkg` for scoped packages, else the first path segment.
pub fn ts_package_name(raw_path: &str) -> String {
    if let Some(rest) = raw_path.strip_prefix('@') {
        let name = rest.split('/').next().unwrap_or("");
        return format!("@{name}/{}", rest.split('/').nth(1).unwrap_or(""))
            .trim_end_matches('/')
            .to_string();
    }
    raw_path.split('/').next().unwrap_or(raw_path).to_string()
}

pub fn ts_resolved(idx: &DepIndex, raw_path: &str) -> bool {
    if raw_path.starts_with('.') {
        return true; // relative import, always valid
    }
    if idx
        .ts_path_aliases
        .iter()
        .any(|a| raw_path == a || raw_path.starts_with(&format!("{a}/")))
    {
        return true; // tsconfig path alias -> local, not a package
    }
    let stripped = raw_path.strip_prefix("node:").unwrap_or(raw_path);
    let first = stripped.split('/').next().unwrap_or(stripped);
    if NODE_BUILTINS.contains(first) {
        return true;
    }
    idx.ts.contains(&ts_package_name(raw_path))
}

pub fn go_resolved(idx: &DepIndex, path: &str) -> bool {
    let first = path.split('/').next().unwrap_or(path);
    if !first.contains('.') {
        return true; // first-segment-no-dot heuristic: stdlib
    }
    idx.go
        .iter()
        .any(|d| path == d || path.starts_with(&format!("{d}/")))
}

pub fn rust_resolved(idx: &DepIndex, name: &str) -> bool {
    RUST_BUILTINS.contains(&name) || idx.rust.contains(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyproject_parses_pep508_specifiers() {
        let text = r#"
[project]
name = "x"
dependencies = [
    "requests>=2.28.0",
    "click[extra]>=8.0",
    "numpy;python_version>='3.9'",
]
"#;
        let deps = parse_pyproject(text);
        assert_eq!(
            deps,
            HashSet::from(["requests".into(), "click".into(), "numpy".into()])
        );
    }

    #[test]
    fn requirements_txt_parses_lines() {
        let text = "requests==2.28.0\nclick>=8.0,<9.0\nDjango[postgres]\n# a comment\n-r other.txt\ngit+https://github.com/user/repo.git@v1.0#egg=mylib\n";
        let deps = parse_requirements(text);
        assert_eq!(
            deps,
            HashSet::from([
                "requests".into(),
                "click".into(),
                "django".into(),
                "mylib".into()
            ])
        );
    }

    #[test]
    fn package_json_collects_all_dep_fields() {
        let text = r#"{"dependencies":{"react":"^18.0.0"},"devDependencies":{"typescript":"^5.0.0"},
            "peerDependencies":{"react-dom":">=16.0"},"optionalDependencies":{"sqlite3":"^5.1.0"}}"#;
        let deps = parse_package_json(text);
        assert_eq!(
            deps,
            HashSet::from([
                "react".into(),
                "typescript".into(),
                "react-dom".into(),
                "sqlite3".into()
            ])
        );
    }

    #[test]
    fn tsconfig_extracts_path_alias_prefixes() {
        let text =
            r#"{"compilerOptions":{"paths":{"@app/*":["src/*"],"utils/*":["src/utils/*"]}}}"#;
        let aliases = parse_tsconfig(text);
        assert_eq!(aliases, HashSet::from(["@app".into(), "utils".into()]));
    }

    #[test]
    fn go_mod_parses_module_and_require_block() {
        let text = "module github.com/user/project\n\ngo 1.21\n\nrequire (\n\tgithub.com/foo/bar v1.2.3\n\tgolang.org/x/tools v0.10.0 // indirect\n)\n";
        let deps = parse_go_mod(text);
        assert_eq!(
            deps,
            HashSet::from([
                "github.com/user/project".into(),
                "github.com/foo/bar".into(),
                "golang.org/x/tools".into()
            ])
        );
    }

    #[test]
    fn go_mod_parses_single_line_require() {
        let text = "module m\n\nrequire github.com/foo/bar v1.2.3\n";
        let deps = parse_go_mod(text);
        assert_eq!(
            deps,
            HashSet::from(["m".into(), "github.com/foo/bar".into()])
        );
    }

    #[test]
    fn cargo_toml_normalizes_hyphens_and_collects_all_sections() {
        let text = r#"
[package]
name = "my-app"

[dependencies]
serde-json = "1.0"

[dev-dependencies]
proptest = "1.0"

[target.'cfg(windows)'.dependencies]
winapi = "0.3"

[workspace.dependencies]
log = { version = "0.4" }
"#;
        let deps = parse_cargo_toml(text);
        assert_eq!(
            deps,
            HashSet::from([
                "my_app".into(),
                "serde_json".into(),
                "proptest".into(),
                "winapi".into(),
                "log".into()
            ])
        );
    }

    #[test]
    fn pep503_normalization_examples() {
        assert_eq!(pep503("Scikit-Learn"), "scikit-learn");
        assert_eq!(pep503("scikit_learn"), "scikit-learn");
        assert_eq!(pep503("scikit-learn"), "scikit-learn");
    }

    #[test]
    fn discover_walks_temp_dir_and_merges_manifests() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"app\"\n\n[dependencies]\nserde = \"1.0\"\n",
        )
        .unwrap();
        let sub = tmp.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(
            sub.join("pyproject.toml"),
            "[project]\ndependencies = [\"requests\"]\n",
        )
        .unwrap();

        let idx = DepIndex::discover(&[tmp.path().to_path_buf()]);
        assert!(idx.rust.contains("app"));
        assert!(idx.rust.contains("serde"));
        assert!(idx.python.contains("requests"));
        assert!(idx.go.is_empty());
    }

    /// Linting single files (pre-commit hooks, `git diff --name-only | xargs`) used to resolve
    /// against an empty index, so SLOP010 silently found nothing.
    #[test]
    fn discover_climbs_to_the_manifest_when_the_root_is_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".git"), "").unwrap();
        std::fs::write(
            tmp.path().join("package.json"),
            r#"{"dependencies":{"react":"^18"}}"#,
        )
        .unwrap();
        let sub = tmp.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        let file = sub.join("a.ts");
        std::fs::write(&file, "import x from 'react';\n").unwrap();

        let idx = DepIndex::discover(&[file]);
        assert!(idx.ts.contains("react"));
    }

    /// Reading `readdir`'s entry type instead of stat-ing each path must still descend real
    /// subdirectories, still skip `SKIP_DIRS`, and still pick up a manifest reached by symlink.
    #[test]
    fn discover_descends_subdirs_skips_vendor_dirs_and_follows_symlinked_manifests() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("crates/inner");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Cargo.toml"), "[dependencies]\nserde = \"1\"\n").unwrap();

        let skipped = tmp.path().join("node_modules/pkg");
        std::fs::create_dir_all(&skipped).unwrap();
        std::fs::write(
            skipped.join("package.json"),
            r#"{"dependencies":{"left-pad":"1"}}"#,
        )
        .unwrap();

        let real = tmp.path().join("elsewhere.json");
        std::fs::write(&real, r#"{"dependencies":{"react":"18"}}"#).unwrap();
        std::os::unix::fs::symlink(&real, tmp.path().join("package.json")).unwrap();

        let idx = DepIndex::discover(&[tmp.path().to_path_buf()]);
        assert!(idx.rust.contains("serde"), "must descend into crates/inner");
        assert!(
            idx.ts.contains("react"),
            "a symlinked manifest still counts"
        );
        assert!(!idx.ts.contains("left-pad"), "node_modules stays skipped");
    }

    #[test]
    fn discover_empty_dir_yields_empty_index() {
        let tmp = tempfile::tempdir().unwrap();
        let idx = DepIndex::discover(&[tmp.path().to_path_buf()]);
        assert!(
            idx.python.is_empty() && idx.ts.is_empty() && idx.go.is_empty() && idx.rust.is_empty()
        );
    }
}

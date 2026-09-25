//! Agent-readiness score: how easy is this repository for an AI agent to
//! work in? Eight checks, 100 points, each with concrete fixes.

use crate::index::{Index, Role};
use crate::lang::Lang;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

pub const BADGE_BASE: &str = "https://mark.sylphx.com/badge/";
pub const BADGE_LINK: &str = "https://github.com/SylphxAI/repomap#agent-readiness-score";

#[derive(Debug, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub name: &'static str,
    pub score: u32,
    pub max: u32,
    pub detail: String,
    /// (points this fix would add, what to do)
    pub fixes: Vec<(u32, String)>,
}

#[derive(Debug, Serialize)]
pub struct Score {
    pub score: u32,
    pub color: &'static str,
    pub badge_url: String,
    pub badge_markdown: String,
    pub checks: Vec<Check>,
}

fn read(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

fn exists(root: &Path, rel: &str) -> bool {
    root.join(rel).exists()
}

pub fn color_for(score: u32) -> &'static str {
    match score {
        85.. => "brightgreen",
        70..=84 => "green",
        55..=69 => "yellow",
        40..=54 => "orange",
        _ => "red",
    }
}

fn color_hex(color: &str) -> &'static str {
    match color {
        "brightgreen" => "#4c1",
        "green" => "#97ca00",
        "yellow" => "#dfb317",
        "orange" => "#fe7d37",
        _ => "#e05d44",
    }
}

/// A self-contained, shields-style SVG badge (no network needed to render).
pub fn badge_svg(score: u32) -> String {
    let label = "agent-ready";
    let value = format!("{score}/100");
    // Verdana 11px averages about 6.5px per character in these strings.
    let w = |t: &str| (t.len() as f32 * 6.5 + 10.0).round() as u32;
    let (lw, vw) = (w(label), w(&value));
    let total = lw + vw;
    let color = color_hex(color_for(score));
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="20" role="img" aria-label="{label}: {value}"><title>{label}: {value}</title><linearGradient id="s" x2="0" y2="100%"><stop offset="0" stop-color="#bbb" stop-opacity=".1"/><stop offset="1" stop-opacity=".1"/></linearGradient><clipPath id="r"><rect width="{total}" height="20" rx="3" fill="#fff"/></clipPath><g clip-path="url(#r)"><rect width="{lw}" height="20" fill="#555"/><rect x="{lw}" width="{vw}" height="20" fill="{color}"/><rect width="{total}" height="20" fill="url(#s)"/></g><g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" font-size="11"><text x="{lx}" y="15" fill="#010101" fill-opacity=".3">{label}</text><text x="{lx}" y="14">{label}</text><text x="{vx}" y="15" fill="#010101" fill-opacity=".3">{value}</text><text x="{vx}" y="14">{value}</text></g></svg>"##,
        lx = lw / 2,
        vx = lw + vw / 2,
    )
}

/// Markdown for a badge image (hosted on mark.sylphx.com, or a committed SVG file).
pub fn badge_markdown(score: u32, image: &str) -> String {
    format!("[![agent-ready {score}/100]({image})]({BADGE_LINK})")
}

pub fn badge_url(score: u32) -> String {
    format!("{BADGE_BASE}agent--ready-{score}%2F100-{}", color_for(score))
}

fn pts(max: u32, frac: f32) -> u32 {
    (max as f32 * frac.clamp(0.0, 1.0)).round() as u32
}

const AGENT_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", ".github/copilot-instructions.md", ".cursorrules", "GEMINI.md", ".windsurfrules", "agents.md", ".claude/CLAUDE.md"];

fn has_command_words(text: &str) -> bool {
    let l = text.to_ascii_lowercase();
    (l.contains("```") || l.contains('`'))
        && ["test", "build", "lint", "cargo ", "npm ", "pnpm ", "yarn ", "bun ", "pytest", "go test", "make ", "just ", "gradle", "mvn "].iter().any(|w| l.contains(w))
}

impl Index {
    pub fn agent_score(&self) -> Score {
        let root = self.root.as_path();
        let mut checks = Vec::new();
        let readme_path = ["README.md", "README.rst", "README.txt", "README", "readme.md", "Readme.md"].into_iter().find(|p| exists(root, p));
        let readme = readme_path.and_then(|p| read(root, p)).unwrap_or_default();

        // 1. Agent instructions (20)
        {
            let found: Vec<(&str, String)> = AGENT_FILES.iter().filter_map(|f| read(root, f).map(|t| (*f, t))).collect();
            let mut rules = std::fs::read_dir(root.join(".cursor/rules")).map(|d| d.count()).unwrap_or(0);
            rules += std::fs::read_dir(root.join(".claude/skills")).map(|d| d.count()).unwrap_or(0);
            let text: String = found.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n");
            let mut s = 0;
            let mut fixes = Vec::new();
            if !found.is_empty() || rules > 0 {
                s += 8;
            } else {
                fixes.push((8, "Add AGENTS.md (and CLAUDE.md pointing to it) at the repo root: what the project is, how to build and test it, and where things live. `repomap map` output is a good outline.".to_string()));
            }
            if text.len() >= 400 {
                s += 4;
            } else if !found.is_empty() {
                fixes.push((4, format!("Expand {}: it is only {} characters. Say what the project does, its conventions, and what not to touch.", found[0].0, text.len())));
            }
            if has_command_words(&text) {
                s += 4;
            } else if !found.is_empty() {
                fixes.push((4, "List the exact build, test and lint commands in the agent instructions, in code blocks.".into()));
            }
            let dirs_named = self
                .communities
                .iter()
                .filter(|c| c.kind == "core")
                .filter(|c| c.name.split(['/', ' ']).next().map_or(false, |seg| seg.len() > 2 && text.contains(seg)))
                .count();
            let lower = text.to_ascii_lowercase();
            if dirs_named >= 2 || lower.contains("architecture") || lower.contains("layout") || lower.contains("structure") {
                s += 4;
            } else if !found.is_empty() {
                fixes.push((4, "Describe the repo layout in the agent instructions (main directories and what each owns).".into()));
            }
            let detail = if found.is_empty() && rules == 0 {
                "no AGENTS.md, CLAUDE.md, Copilot or Cursor instructions".to_string()
            } else {
                format!("{}{}", found.iter().map(|(f, t)| format!("{f} ({} chars)", t.len())).collect::<Vec<_>>().join(", "), if rules > 0 { format!(" + {rules} rule/skill files") } else { String::new() })
            };
            checks.push(Check { id: "instructions", name: "Agent instructions", score: s, max: 20, detail, fixes });
        }

        // 2. Build & test commands (15)
        {
            let mut test_cmd: Option<String> = None;
            let mut build_cmd: Option<String> = None;
            if let Some(pj) = read(root, "package.json").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
                if let Some(scripts) = pj.get("scripts").and_then(|v| v.as_object()) {
                    if scripts.get("test").and_then(|v| v.as_str()).map_or(false, |v| !v.contains("no test specified")) {
                        test_cmd = Some("npm test".into());
                    }
                    if scripts.contains_key("build") || scripts.contains_key("lint") || scripts.contains_key("typecheck") {
                        build_cmd = Some("npm run build".into());
                    }
                }
            }
            if exists(root, "Cargo.toml") {
                test_cmd.get_or_insert("cargo test".into());
                build_cmd.get_or_insert("cargo build".into());
            }
            if exists(root, "go.mod") {
                test_cmd.get_or_insert("go test ./...".into());
                build_cmd.get_or_insert("go build ./...".into());
            }
            for f in ["pyproject.toml", "setup.cfg", "tox.ini", "pytest.ini", "noxfile.py"] {
                if let Some(t) = read(root, f) {
                    if t.contains("pytest") || t.contains("unittest") || f == "pytest.ini" || f == "tox.ini" || f == "noxfile.py" {
                        test_cmd.get_or_insert("pytest".into());
                    }
                    if f == "pyproject.toml" {
                        build_cmd.get_or_insert("pip install -e .".into());
                    }
                }
            }
            for f in ["Makefile", "justfile", "Justfile", "Taskfile.yml", "Rakefile"] {
                if let Some(t) = read(root, f) {
                    let l = t.to_ascii_lowercase();
                    if l.contains("test") {
                        test_cmd.get_or_insert(format!("{} test", if f.starts_with('M') { "make" } else if f.starts_with('j') || f.starts_with('J') { "just" } else if f.starts_with('T') { "task" } else { "rake" }));
                    }
                    build_cmd.get_or_insert(format!("see {f}"));
                }
            }
            for f in ["pom.xml", "build.gradle", "build.gradle.kts", "Package.swift", "composer.json", "Gemfile", "CMakeLists.txt", "meson.build"] {
                if exists(root, f) {
                    build_cmd.get_or_insert(format!("see {f}"));
                    if matches!(f, "pom.xml" | "build.gradle" | "build.gradle.kts" | "Package.swift") {
                        test_cmd.get_or_insert(match f {
                            "pom.xml" => "mvn test".into(),
                            "Package.swift" => "swift test".into(),
                            _ => "gradle test".into(),
                        });
                    }
                }
            }
            let mut s = 0;
            let mut fixes = Vec::new();
            if test_cmd.is_some() {
                s += 9;
            } else {
                fixes.push((9, "Make the test command discoverable: a `test` script in package.json, a `test` target in the Makefile/justfile, or pytest configuration in pyproject.toml.".into()));
            }
            if build_cmd.is_some() {
                s += 3;
            } else {
                fixes.push((3, "Add a standard build/lint entry point (package.json scripts, Makefile or justfile).".into()));
            }
            let agent_text: String = AGENT_FILES.iter().filter_map(|f| read(root, f)).collect::<Vec<_>>().join("\n") + &readme;
            if has_command_words(&agent_text) {
                s += 3;
            } else {
                fixes.push((3, "Write the build and test commands in README.md or AGENTS.md so agents do not have to guess.".into()));
            }
            let detail = format!("test: {} · build: {}", test_cmd.as_deref().unwrap_or("not found"), build_cmd.as_deref().unwrap_or("not found"));
            checks.push(Check { id: "commands", name: "Build & test commands", score: s, max: 15, detail, fixes });
        }

        // 3. Tests (15)
        {
            let core = self.code_files().filter(|(_, f)| f.role == Role::Core).count();
            // Rust (and similar) keep unit tests inside source files: count those too.
            let inline = self
                .code_files()
                .filter(|(_, f)| f.role == Role::Core && f.lang == Some(Lang::Rust))
                .filter(|(_, f)| read(root, &f.path).map_or(false, |t| t.contains("#[test]") || t.contains("#[cfg(test)]")))
                .count();
            let tests = self.code_files().filter(|(_, f)| f.role == Role::Test).count() + inline;
            let ratio = if core == 0 { 1.0 } else { tests as f32 / core as f32 };
            let s = if tests == 0 { 0 } else { pts(15, ratio / 0.4) };
            let mut fixes = Vec::new();
            if s < 15 {
                // Suggest the most central untested files.
                let tested: std::collections::HashSet<u32> = self
                    .file_edges
                    .iter()
                    .filter(|e| self.files[e.from as usize].role == Role::Test)
                    .map(|e| e.to)
                    .collect();
                let mut untested: Vec<u32> = self.code_files().filter(|(i, f)| f.role == Role::Core && !tested.contains(i)).map(|(i, _)| i).collect();
                untested.sort_by(|a, b| self.file_rank[*b as usize].partial_cmp(&self.file_rank[*a as usize]).unwrap_or(std::cmp::Ordering::Equal));
                let names: Vec<&str> = untested.iter().take(3).map(|f| self.files[*f as usize].path.as_str()).collect();
                let target = ((core as f32 * 0.4).ceil() as usize).saturating_sub(tests);
                fixes.push((15 - s, format!("Add tests (about {target} more test files for a 0.4 test/source ratio). Start with the most central untested files: {}.", if names.is_empty() { "-".into() } else { names.join(", ") })));
            }
            let detail = if inline > 0 { format!("{tests} test files (incl. {inline} with inline tests) for {core} source files ({ratio:.2})") } else { format!("{tests} test files for {core} source files ({ratio:.2})") };
            checks.push(Check { id: "tests", name: "Tests", score: s, max: 15, detail, fixes });
        }

        // 4. CI (10)
        {
            let mut ci_text = String::new();
            let mut found = Vec::new();
            if let Ok(rd) = std::fs::read_dir(root.join(".github/workflows")) {
                for e in rd.flatten() {
                    if let Ok(t) = std::fs::read_to_string(e.path()) {
                        found.push(format!(".github/workflows/{}", e.file_name().to_string_lossy()));
                        ci_text.push_str(&t);
                    }
                }
            }
            for f in [".gitlab-ci.yml", ".circleci/config.yml", "azure-pipelines.yml", "Jenkinsfile", ".buildkite/pipeline.yml", ".travis.yml", "bitbucket-pipelines.yml"] {
                if let Some(t) = read(root, f) {
                    found.push(f.to_string());
                    ci_text.push_str(&t);
                }
            }
            let mut s = 0;
            let mut fixes = Vec::new();
            if !found.is_empty() {
                s += 6;
            } else {
                fixes.push((10, "Add CI (e.g. .github/workflows/ci.yml) that builds and runs the tests on every pull request, so agents get a verdict on their changes.".into()));
            }
            if ci_text.to_ascii_lowercase().contains("test") {
                s += 4;
            } else if !found.is_empty() {
                fixes.push((4, "Run the test suite in CI, not only builds or lint.".into()));
            }
            checks.push(Check { id: "ci", name: "CI", score: s, max: 10, detail: if found.is_empty() { "no CI configuration".into() } else { format!("{} pipeline file(s)", found.len()) }, fixes });
        }

        // 5. Module boundaries (10)
        {
            let core: Vec<u32> = self.code_files().filter(|(_, f)| f.role == Role::Core).map(|(i, _)| i).collect();
            let (mut inside, mut total) = (0f32, 0f32);
            let mut pairs: HashMap<(u32, u32), f32> = HashMap::new();
            for e in &self.file_edges {
                let (a, b) = (self.community[e.from as usize], self.community[e.to as usize]);
                if self.files[e.from as usize].role != Role::Core || self.files[e.to as usize].role != Role::Core {
                    continue;
                }
                total += e.weight();
                if a == b {
                    inside += e.weight();
                } else {
                    *pairs.entry((a.min(b), a.max(b))).or_insert(0.0) += e.weight();
                }
            }
            let share = if total == 0.0 { 1.0 } else { inside / total };
            let small = core.len() < 15;
            let s = if small { 10 } else { pts(10, (share - 0.35) / 0.35) };
            let mut fixes = Vec::new();
            if s < 10 {
                if let Some(((a, b), _)) = pairs.iter().max_by(|x, y| x.1.partial_cmp(y.1).unwrap()) {
                    let n = |c: u32| self.communities.get(c as usize).map(|x| x.name.clone()).unwrap_or_default();
                    fixes.push((10 - s, format!("Untangle modules: only {:.0}% of dependencies stay inside their module. The heaviest cross-module traffic is {} ↔ {}. Move shared code into one place or add a clear interface.", share * 100.0, n(*a), n(*b))));
                }
            }
            let modules = self.communities.iter().filter(|c| c.kind == "core").count();
            checks.push(Check { id: "modules", name: "Module boundaries", score: s, max: 10, detail: if small { format!("small codebase ({} source files)", core.len()) } else { format!("{modules} modules, {:.0}% of dependencies stay inside a module", share * 100.0) }, fixes });
        }

        // 6. File sizes (10)
        {
            let mut core: Vec<(u32, u32)> = self.code_files().filter(|(_, f)| f.role == Role::Core).map(|(i, f)| (i, f.lines)).collect();
            let big = core.iter().filter(|(_, l)| *l > 1000).count();
            let frac = if core.is_empty() { 0.0 } else { big as f32 / core.len() as f32 };
            let s = pts(10, 1.0 - frac / 0.2);
            core.sort_by(|a, b| b.1.cmp(&a.1));
            let mut fixes = Vec::new();
            if s < 10 {
                let top: Vec<String> = core.iter().take(4).map(|(f, l)| format!("{} ({l} lines)", self.files[*f as usize].path)).collect();
                fixes.push((10 - s, format!("Split very large files (agents read whole files; {big} are over 1000 lines): {}.", top.join(", "))));
            }
            let largest = core.first().map(|(f, l)| format!("; largest {} ({l})", self.files[*f as usize].path)).unwrap_or_default();
            checks.push(Check { id: "files", name: "File sizes", score: s, max: 10, detail: format!("{big} of {} source files over 1000 lines{largest}", core.len()), fixes });
        }

        // 7. Docs (10)
        {
            let mut s = 0;
            let mut fixes = Vec::new();
            let headings = readme.lines().filter(|l| l.starts_with("## ") || l.starts_with("### ")).count();
            if readme.len() >= 1500 {
                s += 5;
            } else if readme.len() >= 300 {
                s += 2;
                fixes.push((3, "Grow the README: what it is, quick start, how to develop and test, and the project layout.".into()));
            } else {
                fixes.push((5, "Write a README: what it is, quick start, how to develop and test, and the project layout.".into()));
            }
            if headings >= 3 {
                s += 2;
            } else if !readme.is_empty() {
                fixes.push((2, "Structure the README with sections (## Install, ## Usage, ## Development).".into()));
            }
            let extra = ["docs", "doc", "ARCHITECTURE.md", "CONTRIBUTING.md", "docs/architecture.md", "DESIGN.md"].iter().filter(|p| exists(root, p)).count();
            if extra > 0 {
                s += 3;
            } else {
                fixes.push((3, "Add ARCHITECTURE.md or CONTRIBUTING.md (or a docs/ folder) describing the design and workflow.".into()));
            }
            checks.push(Check { id: "docs", name: "Docs", score: s, max: 10, detail: format!("README {} chars, {headings} sections{}", readme.len(), if extra > 0 { ", design/contributing docs" } else { "" }), fixes });
        }

        // 8. Types (10)
        {
            let mut lines_by: HashMap<Lang, u32> = HashMap::new();
            for (_, f) in self.code_files().filter(|(_, f)| f.role == Role::Core) {
                *lines_by.entry(f.lang.unwrap()).or_insert(0) += f.lines;
            }
            let total: u32 = lines_by.values().sum();
            let tsconfig = read(root, "tsconfig.json").unwrap_or_default();
            let ts_strict = tsconfig.contains("\"strict\": true") || tsconfig.contains("\"strict\":true");
            let py_typed = ["mypy.ini", ".mypy.ini", "pyrightconfig.json"].iter().any(|p| exists(root, p))
                || read(root, "pyproject.toml").map_or(false, |t| t.contains("[tool.mypy]") || t.contains("[tool.pyright]"));
            let mut weighted = 0f32;
            for (lang, lines) in &lines_by {
                let q = match lang {
                    Lang::TypeScript | Lang::Tsx => if ts_strict { 1.0 } else { 0.75 },
                    Lang::JavaScript => if exists(root, "jsconfig.json") || tsconfig.contains("checkJs") { 0.5 } else { 0.2 },
                    Lang::Python => if py_typed { 0.9 } else { 0.4 },
                    Lang::Ruby => if exists(root, "sorbet") { 0.8 } else { 0.2 },
                    Lang::Php => 0.5,
                    _ => 1.0,
                };
                weighted += q * *lines as f32;
            }
            let frac = if total == 0 { 1.0 } else { weighted / total as f32 };
            let s = pts(10, frac);
            let mut fixes = Vec::new();
            if s < 10 {
                let mut tips = Vec::new();
                if lines_by.contains_key(&Lang::TypeScript) && !ts_strict {
                    tips.push("turn on \"strict\" in tsconfig.json");
                }
                if lines_by.contains_key(&Lang::JavaScript) {
                    tips.push("migrate JavaScript to TypeScript, or enable checkJs");
                }
                if lines_by.contains_key(&Lang::Python) && !py_typed {
                    tips.push("add type hints and run mypy or pyright (configure it in pyproject.toml)");
                }
                if !tips.is_empty() {
                    fixes.push((10 - s, format!("Types help agents change code safely: {}.", tips.join("; "))));
                }
            }
            let mut mix: Vec<(Lang, u32)> = lines_by.into_iter().collect();
            mix.sort_by(|a, b| b.1.cmp(&a.1));
            let detail = mix.iter().take(3).map(|(l, n)| format!("{} {}%", l.name(), if total == 0 { 0 } else { n * 100 / total })).collect::<Vec<_>>().join(", ");
            checks.push(Check { id: "types", name: "Types", score: s, max: 10, detail, fixes });
        }

        let score: u32 = checks.iter().map(|c| c.score).sum();
        let badge_url = badge_url(score);
        let badge_markdown = badge_markdown(score, &badge_url);
        Score { score, color: color_for(score), badge_url, badge_markdown, checks }
    }
}

impl Score {
    pub fn text(&self) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "# Agent-readiness: {}/100 ({})\n", self.score, self.color);
        for c in &self.checks {
            let mark = if c.score == c.max { "✓" } else if c.score * 2 >= c.max { "~" } else { "✗" };
            let _ = writeln!(o, "  {mark} {:<22} {:>2}/{:<2}  {}", c.name, c.score, c.max, c.detail);
        }
        let mut fixes: Vec<&(u32, String)> = self.checks.iter().flat_map(|c| c.fixes.iter()).collect();
        fixes.sort_by(|a, b| b.0.cmp(&a.0));
        if !fixes.is_empty() {
            let _ = writeln!(o, "\n## Fixes (most points first)");
            for (i, (p, f)) in fixes.iter().enumerate() {
                let _ = writeln!(o, "{}. +{p}  {f}", i + 1);
            }
        }
        let _ = writeln!(o, "\n## Badge\n{}", self.badge_markdown);
        o
    }
}

/// Replace an existing agent-ready badge in `readme` (between
/// `<!-- repomap:agent-ready -->` markers, or any mark.sylphx.com agent-ready
/// badge). Returns the new text, or None when nothing was found to replace.
pub fn update_badge(readme: &str, markdown: &str, insert: bool) -> Option<String> {
    let start = "<!-- repomap:agent-ready -->";
    let end = "<!-- /repomap:agent-ready -->";
    if let (Some(a), Some(b)) = (readme.find(start), readme.find(end)) {
        if a < b {
            return Some(format!("{}{start}{markdown}{end}{}", &readme[..a], &readme[b + end.len()..]));
        }
    }
    let re = regex::Regex::new(r"\[!\[agent-ready[^\]]*\]\((?:https://mark\.sylphx\.com/badge/agent--ready-[^)]*|[^)]*agent-ready\.svg)\)\]\([^)]*\)").unwrap();
    if re.is_match(readme) {
        return Some(re.replace(readme, regex::NoExpand(markdown)).to_string());
    }
    if insert {
        // After the first heading line, or at the top.
        let block = format!("{start}{markdown}{end}\n");
        if let Some(pos) = readme.find('\n').filter(|_| readme.starts_with('#')) {
            return Some(format!("{}\n\n{block}{}", &readme[..pos], readme[pos + 1..].trim_start_matches('\n')));
        }
        return Some(format!("{block}\n{readme}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_helpers() {
        assert_eq!(badge_url(87), "https://mark.sylphx.com/badge/agent--ready-87%2F100-brightgreen");
        assert_eq!(color_for(60), "yellow");
        let md = "[![agent-ready 90/100](https://mark.sylphx.com/badge/agent--ready-90%2F100-brightgreen)](x)";
        let r = update_badge("# T\n<!-- repomap:agent-ready -->old<!-- /repomap:agent-ready -->\nbody", md, false).unwrap();
        assert!(r.contains(md) && !r.contains("old"));
        let r2 = update_badge("# T\n[![agent-ready 50/100](https://mark.sylphx.com/badge/agent--ready-50%2F100-orange)](y) more", md, false).unwrap();
        assert!(r2.contains("90%2F100") && r2.ends_with(" more"));
        assert!(update_badge("# T\nnothing", md, false).is_none());
        let stat = badge_markdown(91, ".github/agent-ready.svg");
        assert!(update_badge(&r2, &stat, false).unwrap().contains(".github/agent-ready.svg"));
        let svg = badge_svg(91);
        assert!(svg.starts_with("<svg") && svg.contains("91/100") && svg.contains("#4c1"));
        assert!(update_badge("# T\nnothing", md, true).unwrap().starts_with("# T\n\n<!-- repomap:agent-ready -->"));
    }

    #[test]
    fn scores_fixture() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture");
        let idx = Index::build(&root, &crate::BuildOptions { use_cache: false }).unwrap();
        let s = idx.agent_score();
        assert_eq!(s.checks.len(), 8);
        assert_eq!(s.checks.iter().map(|c| c.max).sum::<u32>(), 100);
        assert!(s.score < 100);
        assert!(s.text().contains("## Fixes"));
    }
}

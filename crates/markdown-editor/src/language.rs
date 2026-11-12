//! FORK: which languages a Code Block can highlight, and the list its Language Menu offers.

use std::sync::LazyLock;

use syntect::parsing::{SyntaxReference, SyntaxSet};

/// One row of the Language Menu: display name, fence name, and extra names that find it.
#[derive(Debug, PartialEq, Eq)]
pub struct LanguageEntry {
    pub display: &'static str,
    pub fence: &'static str,
    pub aliases: &'static [&'static str],
}

const fn entry(
    display: &'static str,
    fence: &'static str,
    aliases: &'static [&'static str],
) -> LanguageEntry {
    LanguageEntry {
        display,
        fence,
        aliases,
    }
}

/// The languages the Language Menu offers. `shell` is an alias for searching only;
/// it doesn't highlight.
pub const CODE_BLOCK_LANGUAGES: &[LanguageEntry] = &[
    entry("Bash", "bash", &["sh", "zsh", "shell"]),
    entry("C", "c", &[]),
    entry("C++", "cpp", &["c++"]),
    entry("C#", "csharp", &["cs"]),
    entry("CSS", "css", &["scss"]),
    entry("Diff", "diff", &["patch"]),
    entry("Dockerfile", "dockerfile", &[]),
    entry("Go", "go", &[]),
    entry("HTML", "html", &[]),
    entry("Java", "java", &[]),
    entry("JavaScript", "javascript", &["js"]),
    entry("JSON", "json", &["jsonc"]),
    entry("Kotlin", "kotlin", &["kt"]),
    entry("Lua", "lua", &[]),
    entry("Markdown", "markdown", &["md"]),
    entry("PHP", "php", &[]),
    entry("Python", "python", &["py"]),
    entry("Ruby", "ruby", &["rb"]),
    entry("Rust", "rust", &["rs"]),
    entry("SQL", "sql", &[]),
    entry("Swift", "swift", &[]),
    entry("TOML", "toml", &[]),
    entry("TypeScript", "typescript", &["ts"]),
    entry("YAML", "yaml", &["yml"]),
    entry("Zig", "zig", &[]),
];

/// The display name for a fence name or alias, ignoring case.
pub fn display_name(lang: &str) -> Option<&'static str> {
    CODE_BLOCK_LANGUAGES
        .iter()
        .find(|e| {
            e.fence.eq_ignore_ascii_case(lang)
                || e.aliases.iter().any(|a| a.eq_ignore_ascii_case(lang))
        })
        .map(|e| e.display)
}

/// Whether a Code Block tagged `lang` gets syntax highlighting. Ignores case.
pub fn highlights(lang: &str) -> bool {
    find_syntax(lang).is_some()
}

// Deserialising the syntax set takes tens of milliseconds, so it waits for the first lookup.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

// FORK: a copy of gpui-component 0.9.1's `language_name`
// (its `language_name` function), so we agree with what gpui-kit highlights.
fn canonical_name(name: &str) -> String {
    let name = name.to_lowercase();
    match name.as_str() {
        "plain" | "plaintext" => "text",
        "jsonc" => "json",
        "sh" => "bash",
        "c++" => "cpp",
        "cs" => "csharp",
        "scss" => "css",
        "ex" => "elixir",
        "js" => "javascript",
        "kt" | "kts" | "ktm" => "kotlin",
        "makefile" => "make",
        "md" | "mdx" => "markdown",
        "markdown-inline" => "markdown_inline",
        "php3" | "php4" | "php5" | "phtml" => "php",
        "protobuf" => "proto",
        "py" | "pyi" => "python",
        "rb" => "ruby",
        "rs" => "rust",
        "ts" => "typescript",
        "yml" => "yaml",
        _ => return name,
    }
    .to_string()
}

// FORK: a copy of gpui-component 0.9.1's three-step `find_syntax`
// (its syntect backend): the name as given, its canonical form, then
// syntect's own spelling. Plain Text is not a highlight.
fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let canonical = canonical_name(lang);
    let alias = match canonical.as_str() {
        "text" => return None,
        "csharp" => "cs",
        "make" => "Makefile",
        "markdown_inline" => "md",
        "embedded_template" | "ejs" => "erb",
        other => other,
    };
    [lang, canonical.as_str(), alias]
        .into_iter()
        .find_map(|token| SYNTAXES.find_syntax_by_token(token))
        .filter(|syntax| syntax.name != "Plain Text")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_known_languages_and_aliases_ignoring_case() {
        for name in ["rust", "Rust", "rs", "yml", "sh", "haskell", "csharp"] {
            assert!(highlights(name), "{name} should highlight");
        }
    }

    #[test]
    fn does_not_highlight_unknown_or_plain_names() {
        for name in [
            "foo",
            "jsx",
            "shell",
            "text",
            "txt",
            "plain",
            "plaintext",
            "",
        ] {
            assert!(!highlights(name), "{name:?} should not highlight");
        }
    }

    #[test]
    fn display_name_matches_fence_name_or_alias_ignoring_case() {
        assert_eq!(display_name("RS"), Some("Rust"));
        assert_eq!(display_name("rust"), Some("Rust"));
        assert_eq!(display_name("C++"), Some("C++"));
        assert_eq!(display_name("shell"), Some("Bash"));
        assert_eq!(display_name("haskell"), None);
        assert_eq!(display_name(""), None);
    }

    #[test]
    fn curated_list_has_25_entries_and_every_fence_name_highlights() {
        assert_eq!(CODE_BLOCK_LANGUAGES.len(), 25);
        for e in CODE_BLOCK_LANGUAGES {
            assert!(
                highlights(e.fence),
                "{} ({}) should highlight",
                e.display,
                e.fence
            );
        }
    }

    /// Outcomes recorded by reading gpui-component 0.9.1's `find_syntax`
    /// (`highlighter/syntect_backend.rs`), which gpui-kit doesn't export, so we
    /// can't call it. If a gpui-kit bump changes its alias table or search, this
    /// list should be re-recorded and the lookup updated.
    const GPUI_KIT_OUTCOMES: &[(&str, bool)] = &[
        ("rust", true),
        ("rs", true),
        ("RS", true),
        ("yml", true),
        ("yaml", true),
        ("sh", true),
        ("bash", true),
        ("js", true),
        ("ts", true),
        ("kt", true),
        ("py", true),
        ("rb", true),
        ("md", true),
        ("c++", true),
        ("cs", true),
        ("csharp", true),
        ("scss", true),
        ("jsonc", true),
        ("php3", true),
        ("haskell", true),
        ("go", true),
        ("sql", true),
        ("toml", true),
        ("dockerfile", true),
        ("diff", true),
        ("zig", true),
        ("lua", true),
        ("text", false),
        ("plain", false),
        ("plaintext", false),
        ("txt", false),
        ("Plain Text", false),
        ("foo", false),
        ("jsx", false),
        ("shell", false),
        ("no-such-language", false),
        ("", false),
    ];

    #[test]
    fn lookup_agrees_with_gpui_kit() {
        for (name, expected) in GPUI_KIT_OUTCOMES {
            assert_eq!(highlights(name), *expected, "{name:?}");
        }
    }
}

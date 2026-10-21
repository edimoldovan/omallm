//! Omarchy's scripts: their names and header lines, and what a model answer means.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Script {
    pub path: PathBuf,
    pub summary: String,
    pub sudo: bool,
}

pub struct Scripts {
    map: BTreeMap<String, Script>,
}

impl Scripts {
    /// Every `omarchy-*` file in ~/.local/share/omarchy/bin with its header lines.
    pub fn load() -> Scripts {
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = PathBuf::from(home).join(".local/share/omarchy/bin");
        let mut map = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();
                if !name.starts_with("omarchy-") || !path.is_file() {
                    continue;
                }
                let head = read_head(&path);
                let script = Script {
                    path,
                    summary: meta(&head, "summary"),
                    sudo: meta(&head, "requires-sudo") == "true",
                };
                map.insert(name, script);
            }
        }
        Scripts { map }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn get(&self, name: &str) -> Option<&Script> {
        self.map.get(name)
    }
}

fn read_head(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    let end = bytes.len().min(4096);
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn meta(head: &str, key: &str) -> String {
    let prefix = format!("# omarchy:{key}=");
    head.lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .trim()
        .to_string()
}

pub enum Answer {
    /// A script that exists, its arguments after dropping the `[...]` parts, and the `<...>` parts still to fill.
    Command {
        name: String,
        args: Vec<String>,
        placeholders: Vec<String>,
    },
    Keybinding(String),
    /// A sentence or more from the manual: shown, never run.
    Prose(String),
    Unknown(String),
}

pub fn classify(scripts: &Scripts, text: &str) -> Answer {
    let text = text.trim();
    if is_keybinding(text) {
        return Answer::Keybinding(text.to_string());
    }
    let stripped = strip_optional(text);
    let words = split(&stripped);
    let Some(first) = words.first() else {
        return Answer::Unknown(text.to_string());
    };
    if scripts.get(first).is_none() {
        if words.len() >= 4 && !first.starts_with("omarchy-") {
            return Answer::Prose(text.to_string());
        }
        return Answer::Unknown(text.to_string());
    }
    let args: Vec<String> = words[1..].to_vec();
    let placeholders = args.iter().filter(|a| is_placeholder(a)).cloned().collect();
    Answer::Command {
        name: first.clone(),
        args,
        placeholders,
    }
}

pub fn is_placeholder(word: &str) -> bool {
    word.contains('<') && word.contains('>')
}

fn is_keybinding(text: &str) -> bool {
    ["SUPER", "CTRL", "ALT", "SHIFT", "XF86", "switch:"]
        .iter()
        .any(|p| text.starts_with(p))
}

/// Shell-style split; falls back to whitespace when quotes are unbalanced.
pub fn split(text: &str) -> Vec<String> {
    shell_words::split(text).unwrap_or_else(|_| text.split_whitespace().map(String::from).collect())
}

/// The text without its `[...]` optional parts, spaces collapsed.
pub fn strip_optional(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The command as one line: arguments with spaces or quotes are double-quoted.
pub fn command_line(name: &str, args: &[String]) -> String {
    let mut parts = vec![name.to_string()];
    for arg in args {
        if arg.contains(char::is_whitespace) || arg.contains('"') {
            parts.push(format!("\"{}\"", arg.replace('"', "\\\"")));
        } else {
            parts.push(arg.clone());
        }
    }
    parts.join(" ")
}

//! The starter `.obsidian/` config and `.gitignore` (ADR-2042 decision 3).
//!
//! These are written once. In `--in-place` mode an existing `.obsidian/` is
//! never overwritten: it holds the owner's real settings.

/// `(vault-relative path, content)` pairs, in deterministic order.
pub fn config_files() -> Vec<(&'static str, String)> {
    vec![
        (".obsidian/app.json", app_json()),
        (".obsidian/appearance.json", appearance_json()),
        (".obsidian/core-plugins.json", core_plugins_json()),
        (".obsidian/daily-notes.json", daily_notes_json()),
        (".gitignore", gitignore()),
    ]
}

fn j(v: serde_json::Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(&v).unwrap())
}

fn app_json() -> String {
    j(serde_json::json!({
        "newLinkFormat": "shortest",
        "attachmentFolderPath": "assets",
        "useMarkdownLinks": false,
        "showFrontmatter": true
    }))
}

fn appearance_json() -> String {
    j(serde_json::json!({ "theme": "obsidian" }))
}

fn core_plugins_json() -> String {
    j(serde_json::json!([
        "file-explorer",
        "global-search",
        "graph",
        "backlink",
        "outgoing-link",
        "tag-pane",
        "page-preview",
        "daily-notes",
        "templates",
        "command-palette",
        "properties",
        "outline",
        "word-count"
    ]))
}

fn daily_notes_json() -> String {
    j(serde_json::json!({ "folder": "journals", "format": "YYYY-MM-DD" }))
}

fn gitignore() -> String {
    "\
.obsidian/workspace.json
.obsidian/workspace-mobile.json
.obsidian/cache
.trash/
"
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_the_five_starter_files() {
        let f = config_files();
        assert_eq!(f.len(), 5);
        let names: Vec<&str> = f.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&".obsidian/app.json"));
        assert!(names.contains(&".gitignore"));
    }

    #[test]
    fn app_json_carries_the_adr_2042_keys() {
        let v: serde_json::Value = serde_json::from_str(&app_json()).unwrap();
        assert_eq!(v["newLinkFormat"], "shortest");
        assert_eq!(v["attachmentFolderPath"], "assets");
        assert_eq!(v["useMarkdownLinks"], false);
        assert_eq!(v["showFrontmatter"], true);
    }

    #[test]
    fn daily_notes_points_at_the_journals_folder() {
        let v: serde_json::Value = serde_json::from_str(&daily_notes_json()).unwrap();
        assert_eq!(v["folder"], "journals");
        assert_eq!(v["format"], "YYYY-MM-DD");
    }

    #[test]
    fn core_plugins_enables_properties_and_daily_notes() {
        let v: serde_json::Value = serde_json::from_str(&core_plugins_json()).unwrap();
        let a = v.as_array().unwrap();
        assert!(a.iter().any(|x| x == "properties"));
        assert!(a.iter().any(|x| x == "daily-notes"));
        assert_eq!(a.len(), 13);
    }
}

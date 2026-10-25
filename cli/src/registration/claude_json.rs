//! The MCP server's user-scope entry in `~/.claude.json`, written directly when the `claude`
//! command cannot be found. Running sessions rewrite this file often, so it is read, edited,
//! and written back at once, and only when the entry is missing or wrong.

use std::path::Path;

use anyhow::Context;

use super::{
    MCP_NAME,
    json::{Json, parse, text},
};

/// `config` (the file's text, empty when it does not exist) with the MCP server set to run
/// `exe`, as `claude mcp add --scope user` writes it.
///
/// # Errors
///
/// Fails when the text is not JSON or `mcpServers` is not an object.
pub(super) fn set_server(config: &str, exe: &Path) -> anyhow::Result<String> {
    let mut tree = parse(config)?;
    let servers = tree
        .entry("mcpServers", Json::Object(Vec::new()))
        .context("the file is not a JSON object")?;
    let server = Json::object(vec![
        ("type", Json::string("stdio")),
        ("command", Json::string(exe.to_string_lossy())),
        ("args", Json::Array(Vec::new())),
        ("env", Json::Object(Vec::new())),
    ]);
    match servers.field_mut(MCP_NAME) {
        Some(existing) => *existing = server,
        None => servers
            .object_mut()
            .context("\"mcpServers\" is not an object")?
            .push((MCP_NAME.to_owned(), server)),
    }
    Ok(text(&tree, config))
}

/// `config` without the MCP server called `name`, or `None` when it has none at user scope.
///
/// # Errors
///
/// Fails when the text is not JSON.
pub(super) fn remove_server(config: &str, name: &str) -> anyhow::Result<Option<String>> {
    let mut tree = parse(config)?;
    let removed = tree
        .field_mut("mcpServers")
        .is_some_and(|servers| servers.remove(name));
    Ok(removed.then(|| text(&tree, config)))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::{Value, json};

    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from(r"C:\Users\someone\.allot\bin\allot-mcp.exe")
    }

    #[test]
    fn test_set_server_adds_it_beside_others_and_keeps_order() {
        let config = r#"{"numStartups": 3, "mcpServers": {"other": {"command": "x"}}, "z": 1}"#;

        let updated = set_server(config, &exe()).unwrap();

        assert!(
            updated.find("numStartups") < updated.find("\"z\""),
            "{updated}"
        );
        let tree: Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(tree["mcpServers"]["other"], json!({ "command": "x" }));
        assert_eq!(
            tree["mcpServers"]["allot"],
            json!({
                "type": "stdio",
                "command": r"C:\Users\someone\.allot\bin\allot-mcp.exe",
                "args": [],
                "env": {},
            })
        );
        assert!(super::super::mcp_registered(&tree, &exe()));
    }

    #[test]
    fn test_set_server_replaces_one_that_runs_another_exe() {
        let config = r#"{"mcpServers": {"allot": {"command": "E:/target/allot-mcp.exe"}}}"#;
        let tree: Value = serde_json::from_str(&set_server(config, &exe()).unwrap()).unwrap();
        assert!(super::super::mcp_registered(&tree, &exe()));
        assert_eq!(tree["mcpServers"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn test_remove_server_only_when_present() {
        let added = set_server("", &exe()).unwrap();
        assert_eq!(remove_server(&added, "other").unwrap(), None);
        let removed = remove_server(&added, MCP_NAME).unwrap().unwrap();
        assert_eq!(removed, "{\n  \"mcpServers\": {}\n}\n");
        assert_eq!(remove_server(&removed, MCP_NAME).unwrap(), None);
        assert_eq!(remove_server("{}", MCP_NAME).unwrap(), None);
    }
}

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub desc: String,
    pub script_path: PathBuf,
    pub session_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolDef>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn load(&mut self, scripts_dir: &Path) {
        if !scripts_dir.exists() {
            let _ = fs::create_dir_all(scripts_dir);
        }
        if let Ok(entries) = fs::read_dir(scripts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "sh") {
                    if let Some(tool) = Self::parse_script(&path) {
                        self.tools.insert(tool.name.clone(), tool);
                    }
                }
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&ToolDef> {
        self.tools.get(name)
    }

    pub fn list_for_session(&self, session_type: &str) -> Vec<&ToolDef> {
        let mut tools: Vec<&ToolDef> = self.tools.values()
            .filter(|t| t.session_types.contains(&"all".to_string()) || t.session_types.contains(&session_type.to_string()))
            .collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }

    pub fn build_system_prompt(&self, session_type: &str) -> String {
        let tools = self.list_for_session(session_type);
        if tools.is_empty() { return String::new(); }

        let mut s = String::from("AVAILABLE TOOLS (CLI scripts):\n\n\
            To use a tool, output a tool invocation block exactly like this:\n\
            <tool>\n\
            script_name --arg1 value1 --arg2 \"value with spaces\"\n\
            </tool>\n\n\
            The tool output will be fed back to you. Use tools when you need to search, read files, execute commands, etc.\n\n");

        for t in &tools {
            s.push_str(&format!("## {}\n{}\nPath: {}\n\n", t.name, t.desc, t.script_path.display()));
        }
        s
    }

    fn parse_script(path: &Path) -> Option<ToolDef> {
        let content = fs::read_to_string(path).ok()?;
        let lines: Vec<&str> = content.lines().collect();

        let mut name = String::new();
        let mut desc = String::new();
        let mut session_types = vec!["all".to_string()];

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with("# @name ") {
                name = trimmed[8..].trim().to_string();
            } else if trimmed.starts_with("# @desc ") {
                desc = trimmed[8..].trim().to_string();
            } else if trimmed.starts_with("# @session ") {
                session_types = trimmed[11..].trim().split(',').map(|s| s.trim().to_string()).collect();
            } else if !trimmed.starts_with("#") && !trimmed.is_empty() && !trimmed.starts_with("#!/") {
                break;
            }
        }

        if name.is_empty() {
            name = path.file_stem()?.to_str()?.to_string();
        }
        if desc.is_empty() {
            desc = format!("Execute the {} script", name);
        }

        Some(ToolDef {
            name,
            desc,
            script_path: path.to_path_buf(),
            session_types,
        })
    }
}

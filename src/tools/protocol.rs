use regex::Regex;

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub args: String,
}

/// Parse tool invocations from AI response text.
/// Format:
///   <tool>
///   script_name --arg1 value1 --arg2 "value with spaces"
///   </tool>
pub fn parse_tool_calls(text: &str) -> Vec<ToolCall> {
    let re = Regex::new(r"(?s)<tool>\s*\n?(.*?)\n?\s*</tool>").unwrap();
    let mut calls = Vec::new();

    for cap in re.captures_iter(text) {
        let inner = cap[1].trim().to_string();
        if inner.is_empty() { continue; }

        let mut lines = inner.lines();
        let first_line = lines.next().unwrap_or("").trim().to_string();

        // Split first line into name + rest of args
        let (name, first_args) = if let Some(space_idx) = first_line.find(' ') {
            let (n, a) = first_line.split_at(space_idx);
            (n.trim().to_string(), a.trim().to_string())
        } else {
            (first_line, String::new())
        };

        let mut all_args = first_args;
        for line in lines {
            if !all_args.is_empty() { all_args.push(' '); }
            all_args.push_str(line.trim());
        }

        calls.push(ToolCall { name, args: all_args });
    }

    calls
}

/// Check if AI response contains tool invocations
pub fn has_tool_calls(text: &str) -> bool {
    text.contains("<tool>") && text.contains("</tool>")
}

/// Remove tool blocks from text to get clean response
pub fn strip_tool_blocks(text: &str) -> String {
    let re = Regex::new(r"(?s)<tool>.*?</tool>\s*").unwrap();
    re.replace_all(text, "").trim().to_string()
}

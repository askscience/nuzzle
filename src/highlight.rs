use anyhow::Result;
use chrono::Utc;

use crate::db::repository::Repository;
use crate::types::Highlight;

pub fn export_highlights(repo: &Repository) -> Result<String> {
    let highlights = repo.list_highlights()?;
    let mut output = String::new();

    output.push_str("# Nuzzle Highlights\n\n");
    output.push_str(&format!("_Exported on {}_\n\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

    if highlights.is_empty() {
        output.push_str("No highlights yet.\n");
        return Ok(output);
    }

    for (i, hl) in highlights.iter().enumerate() {
        output.push_str(&format!("## {}. {}\n\n", i + 1, hl.text));
        if let Some(note) = &hl.note {
            output.push_str(&format!("> Note: {}\n\n", note));
        }
        if let Some(created) = hl.created_at {
            output.push_str(&format!("_Saved on {}_\n\n", created.format("%Y-%m-%d")));
        }
        output.push_str("---\n\n");
    }

    Ok(output)
}

pub fn add_highlight(repo: &Repository, entry_id: i64, text: &str, note: Option<&str>) -> Result<()> {
    repo.add_highlight(entry_id, text, note)
}

pub fn list_highlights(repo: &Repository) -> Result<Vec<Highlight>> {
    repo.list_highlights()
}

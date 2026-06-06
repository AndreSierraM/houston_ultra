use crate::error::{CoreError, CoreResult};
use houston_engine_protocol::BootstrapSkill;
use std::fs;
use std::path::Path;

pub fn read_packaged_skills(skills_root: &Path) -> CoreResult<Vec<BootstrapSkill>> {
    if !skills_root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(skills_root)
        .map_err(|e| CoreError::Internal(format!("read skills dir {}: {e}", skills_root.display())))?
    {
        let entry = entry.map_err(|e| CoreError::Internal(e.to_string()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(slug) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let skill_md_path = path.join("SKILL.md");
        if !skill_md_path.exists() {
            continue;
        }
        let skill_md = fs::read_to_string(&skill_md_path).map_err(|e| {
            CoreError::Internal(format!("read {}: {e}", skill_md_path.display()))
        })?;
        out.push(BootstrapSkill {
            slug: slug.to_string(),
            skill_md,
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

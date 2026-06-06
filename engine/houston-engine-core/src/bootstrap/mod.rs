//! Cloud agent bootstrap bundle — export template or live agent state without
//! writing to the local workspace.
//!
//! **MVP (Store → cloud):** callers pass `installedPath` for a Store template,
//! or omit it when `configId` matches a bundled Store agent (skills + seeds
//! resolve from `store/agents/<configId>`). Bundle copies `CLAUDE.md`, packaged
//! skills, and `agentSeeds` from `houston.json` (including bundled routines).
//! Activity seeds are stripped.
//!
//! **Future (local → cloud migration):** `agentPath` reads a live workspace
//! agent and uses [`seeds::gather_migration_seeds`] for on-disk routines and
//! learnings. MVP UI does not expose this mode; the engine keeps it for later.

mod build;
mod seeds;
mod skills;
mod source;

pub use build::build_bootstrap_bundle;

#[cfg(test)]
mod tests;

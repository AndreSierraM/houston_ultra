//! Cloud agent bootstrap bundle — export template or live agent state without
//! writing to the local workspace.

mod build;
mod seeds;
mod skills;
mod source;

pub use build::build_bootstrap_bundle;

#[cfg(test)]
mod tests;

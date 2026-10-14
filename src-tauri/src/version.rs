//! Build identity of this Crosstalk binary. A leaf module: it uses nothing
//! else from the crate (updater, logs and the GPU guard all read it).

/// This build's number (commits on the branch, see build.rs).
pub fn current_build() -> u32 {
    env!("CROSSTALK_BUILD").parse().unwrap_or(0)
}

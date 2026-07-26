mod resolver;
#[cfg(unix)]
mod runtime;

pub use resolver::{PathCapability, PathEnvironment, PathResolutionError, PathResolver};
#[cfg(unix)]
pub use runtime::{
    PrepareRuntimePathsError, PreparedRuntimePaths, RuntimeDirectoryError,
    prepare_private_runtime_directory,
};

#[cfg(test)]
mod tests;

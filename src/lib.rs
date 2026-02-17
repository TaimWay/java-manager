pub mod error;
pub mod info;
pub mod search;
pub mod local;
pub mod execute;

pub use info::JavaInfo;
pub use error::JavaError;
pub use search::quick_search;
pub use search::deep_search;
pub use local::java_home;
pub use execute::JavaRunner;
pub use execute::JavaRedirect;

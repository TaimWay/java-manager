use crate::JavaInfo;
use std::env;

pub fn java_home() -> Option<JavaInfo> {
    env::var("JAVA_HOME")
        .ok()
        .and_then(|path| JavaInfo::new(path).ok())
}

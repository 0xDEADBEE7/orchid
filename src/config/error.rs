use std::path::PathBuf;

#[derive(Debug)]
pub enum ResourceLoadError {
    Missing {
        kind: &'static str,
        path: PathBuf,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid {
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for ResourceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { kind, path } => write!(f, "missing {} at {}", kind, path.display()),
            Self::Read { path, source } => {
                write!(f, "failed to read {}: {}", path.display(), source)
            }
            Self::Parse { path, source } => {
                write!(f, "invalid JSON at {}: {}", path.display(), source)
            }
            Self::Invalid { path, message } => {
                write!(f, "invalid resource at {}: {}", path.display(), message)
            }
        }
    }
}

impl std::error::Error for ResourceLoadError {}

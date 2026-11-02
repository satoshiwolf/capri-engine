use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;

pub type EngineResult<T> = Result<T, EngineError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Asset,
    Gateway,
    Gpu,
    Io,
    Protocol,
    Window,
    Other,
}

#[derive(Debug)]
pub enum EngineError {
    Context {
        category: ErrorCategory,
        operation: &'static str,
        message: String,
    },
    Io {
        operation: &'static str,
        source: io::Error,
    },
    Image {
        operation: &'static str,
        source: image::ImageError,
    },
}

impl EngineError {
    pub fn new(
        category: ErrorCategory,
        operation: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::Context {
            category,
            operation,
            message: message.into(),
        }
    }

    pub fn asset(operation: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Asset, operation, message)
    }

    pub fn gateway(operation: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Gateway, operation, message)
    }

    pub fn gpu(operation: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Gpu, operation, message)
    }

    pub fn protocol(operation: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Protocol, operation, message)
    }

    pub fn window(operation: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Window, operation, message)
    }

    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::Context { category, .. } => *category,
            Self::Io { .. } => ErrorCategory::Io,
            Self::Image { .. } => ErrorCategory::Asset,
        }
    }

    pub fn operation(&self) -> &'static str {
        match self {
            Self::Context { operation, .. }
            | Self::Io { operation, .. }
            | Self::Image { operation, .. } => operation,
        }
    }
}

impl Display for EngineError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Context {
                category,
                operation,
                message,
            } => write!(formatter, "{category:?} error in {operation}: {message}"),
            Self::Io { operation, source } => {
                write!(formatter, "IO error in {operation}: {source}")
            }
            Self::Image { operation, source } => {
                write!(formatter, "asset error in {operation}: {source}")
            }
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Image { source, .. } => Some(source),
            Self::Context { .. } => None,
        }
    }
}

impl From<io::Error> for EngineError {
    fn from(source: io::Error) -> Self {
        Self::Io {
            operation: "io",
            source,
        }
    }
}

impl From<image::ImageError> for EngineError {
    fn from(source: image::ImageError) -> Self {
        Self::Image {
            operation: "image",
            source,
        }
    }
}

impl From<String> for EngineError {
    fn from(message: String) -> Self {
        Self::new(ErrorCategory::Other, "engine", message)
    }
}

impl From<&str> for EngineError {
    fn from(message: &str) -> Self {
        Self::new(ErrorCategory::Other, "engine", message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_errors_expose_category_and_operation() {
        let error = EngineError::gpu("request_device", "no compatible adapter");

        assert_eq!(error.category(), ErrorCategory::Gpu);
        assert_eq!(error.operation(), "request_device");
        assert!(error.source().is_none());
        assert!(error.to_string().contains("request_device"));
        assert!(error.to_string().contains("no compatible adapter"));
    }

    #[test]
    fn io_errors_preserve_the_source_chain() {
        let source = io::Error::new(io::ErrorKind::NotFound, "missing asset");
        let error = EngineError::from(source);

        assert_eq!(error.category(), ErrorCategory::Io);
        assert!(error.source().is_some());
        assert!(error.to_string().contains("missing asset"));
    }
}

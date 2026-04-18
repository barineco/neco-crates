use core::fmt;

/// Errors returned by `route`.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingError {
    /// Requested style needs an opt-in feature.
    FeatureDisabled { style: &'static str },
    /// Input could not be routed.
    InvalidInput { reason: &'static str },
}

impl fmt::Display for RoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeatureDisabled { style } => {
                write!(f, "route style {style} requires an enabled Cargo feature")
            }
            Self::InvalidInput { reason } => write!(f, "invalid routing input: {reason}"),
        }
    }
}

impl core::error::Error for RoutingError {}

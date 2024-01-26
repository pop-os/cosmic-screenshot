use std::{
    error::Error as StdError,
    fmt::{self, Display},
    io,
    path::PathBuf,
};

use ashpd::{desktop::ResponseError, Error as AshpdError, PortalError};
use zbus::Error as ZbusError;

/// Error type for requesting screenshots from the XDG portal.
///
/// The primary purpose of this type is to provide simple user facing messages.
#[derive(Debug)]
pub enum Error {
    /// Screenshot errors from the portal or D-Bus
    Ashpd(AshpdError),
    /// Failure to post a notification
    Notify(ZbusError),
    /// Invalid directory path passed AND no Pictures XDG directory
    MissingSaveDirectory(Option<PathBuf>),
    /// Screenshot succeeded but cannot be saved
    SaveScreenshot {
        error: io::Error,
        context: &'static str,
    },
}

impl StdError for Error {}

// Log facing display messages for programmers or debugging
impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ashpd(e) => e.fmt(f),
            Self::Notify(e) => write!(f, "posting a notification: {e}"),
            Self::MissingSaveDirectory(p) => {
                let msg = p
                    .as_deref()
                    .map(|path| format!("opening `{}` or the Pictures directory", path.display()));

                write!(
                    f,
                    "{}",
                    msg.as_deref().unwrap_or("opening Pictures directory")
                )
            }
            Self::SaveScreenshot { error, context } => write!(f, "{context}: {error}"),
        }
    }
}

impl Error {
    /// Localized, condensed error message for end users
    pub fn to_user_facing(&self) -> String {
        match self {
            _ if self.unsupported() => "Portal does not support screenshots".into(),
            _ if self.cancelled() => "Screenshot cancelled".into(),
            _ if self.zbus() => "Problem communicating with D-Bus".into(),
            Self::MissingSaveDirectory(p) => p
                .as_deref()
                .map(|path| {
                    format!(
                        "Unable to save screenshot to {} or the Pictures directory",
                        path.display()
                    )
                })
                .unwrap_or_else(|| "Unable to save screenshot to the Pictures directory".into()),
            Self::Ashpd(e) => match e {
                AshpdError::Portal(e) => match e {
                    PortalError::NotAllowed(msg) => format!("Screenshot not allowed: {msg}"),
                    _ => "Failed to take screenshot".into(),
                },
                _ => "Failed to take screenshot".into(),
            },
            Self::SaveScreenshot { .. } => "Screenshot succeeded but couldn't be saved".into(),
            _ => "Failed to take screenshot".into(),
        }
    }

    /// Screenshot request cancelled
    pub fn cancelled(&self) -> bool {
        let Self::Ashpd(e) = self else {
            return false;
        };

        match e {
            AshpdError::Response(e) => *e == ResponseError::Cancelled,
            AshpdError::Portal(e) => {
                if let PortalError::Cancelled(_) = e {
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Portal does not support screenshots
    pub fn unsupported(&self) -> bool {
        if let Self::Ashpd(e) = self {
            match e {
                // Requires version `x` but interface only supports version `y`
                AshpdError::RequiresVersion(_, _) => true,
                // Unsupported screenshot method or interface for screenshots not found
                AshpdError::Portal(PortalError::ZBus(e)) => {
                    *e == ZbusError::Unsupported || *e == ZbusError::InterfaceNotFound
                }
                AshpdError::Zbus(e) => {
                    *e == ZbusError::Unsupported || *e == ZbusError::InterfaceNotFound
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// D-Bus communication problem
    ///
    /// [zbus::Error] encapsulates many different problems, many of which are programmer errors
    /// which shouldn't occur during normal operation.
    pub fn zbus(&self) -> bool {
        if let Self::Ashpd(e) = self {
            match e {
                AshpdError::Zbus(_) => true,
                AshpdError::Portal(PortalError::ZBus(_)) => {
                    // if let PortalError::ZBus(_) = e {
                    //     true
                    // } else {
                    //     false
                    // }
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}

impl From<AshpdError> for Error {
    fn from(value: AshpdError) -> Self {
        Self::Ashpd(value)
    }
}

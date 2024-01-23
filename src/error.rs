use std::{
    error::Error as StdError,
    fmt::{self, Display},
    path::PathBuf,
};

use ashpd::{desktop::ResponseError, Error as AshpdError, PortalError};
use zbus::Error as ZbusError;

/// Error type for requesting screenshots from the XDG portal.
///
/// The primary purpose of this type is to provide simple user facing messages.
#[derive(Debug)]
pub enum Error {
    Ashpd(AshpdError),
    /// Invalid directory path passed AND no Pictures XDG directory
    MissingSaveDirectory(PathBuf),
}

impl StdError for Error {}

// Log facing display messages for programmers or debugging
impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ashpd(e) => e.fmt(f),
            Self::MissingSaveDirectory(p) => write!(
                f,
                "unable to save screenshot to {} or the Pictures directory",
                p.display()
            ),
        }
    }
}

impl Error {
    /// Localized, condensed error message for end users
    pub fn to_user_facing(&self) -> String {
        match self {
            Self::MissingSaveDirectory(p) => {
                format!("unable to save screenshot to {} or Pictures", p.display())
            }
            _ if self.cancelled() => "Screenshot cancelled".into(),
            _ if self.zbus() => "Problem communicating with D-Bus".into(),
            _ if self.unsupported() => "Portal does not support screenshots".into(),
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
        unimplemented!()
    }

    /// D-Bus communication problem
    pub fn zbus(&self) -> bool {
        unimplemented!()
    }
}

impl From<AshpdError> for Error {
    fn from(value: AshpdError) -> Self {
        Self::Ashpd(value)
    }
}

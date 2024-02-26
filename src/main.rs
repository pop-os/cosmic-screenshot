#![allow(clippy::too_many_arguments)]

mod error;

use ashpd::desktop::screenshot::Screenshot;
use clap::{command, ArgAction, Parser};
use std::{collections::HashMap, fs, os::unix::fs::MetadataExt, path::PathBuf};
use tracing::{debug, error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use zbus::{dbus_proxy, zvariant::Value, Connection};

use error::Error;

#[derive(Parser, Default, Debug, Clone, PartialEq, Eq)]
#[command(version, about, long_about = None)]
struct Args {
    /// Enable interactive mode in the portal
    #[clap(long,
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set)]
    interactive: bool,
    /// Enable modal mode in the portal
    #[clap(long,
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set,)]
    modal: bool,
    /// Send a notification with the path to the saved screenshot
    #[clap(long,
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set)]
    notify: bool,
    /// The directory to save the screenshot to, if not performing an interactive screenshot
    #[clap(short, long)]
    save_dir: Option<PathBuf>,
}

#[dbus_proxy(assume_defaults = true)]
trait Notifications {
    /// Call the org.freedesktop.Notifications.Notify D-Bus method
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, &Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

// Send a notification for the screenshot app
async fn send_notify(summary: &str, body: &str) -> Result<(), Error> {
    let connection = Connection::session().await.map_err(Error::Notify)?;

    let proxy = NotificationsProxy::new(&connection)
        .await
        .map_err(Error::Notify)?;
    proxy
        .notify(
            "Cosmic Screenshot",
            0,
            "camera-photo-symbolic",
            summary,
            body,
            &[],
            HashMap::new(),
            5000,
        )
        .await
        .map_err(Error::Notify)
        .map(|_| ())
}

#[tracing::instrument]
async fn request_screenshot(args: Args) -> Result<String, Error> {
    let picture_dir = (!args.interactive)
        .then(|| {
            args.save_dir
                .clone()
                .filter(|dir| dir.is_dir())
                .or_else(dirs::picture_dir)
                .ok_or_else(|| Error::MissingSaveDirectory(args.save_dir))
        })
        .transpose()?;

    let response = Screenshot::request()
        .interactive(args.interactive)
        .modal(args.modal)
        .send()
        .await?
        .response()?;

    let uri = response.uri();
    debug!("Screenshot request URI: {uri}");
    match uri.scheme() {
        "file" => {
            if let Some(picture_dir) = picture_dir {
                let date = chrono::Local::now();
                let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
                let path = picture_dir.join(filename);
                let tmp_path = uri.path();
                if fs::metadata(&picture_dir)
                    .map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "metadata for screenshot destination",
                    })?
                    .dev()
                    != fs::metadata(tmp_path)
                        .map_err(|error| Error::SaveScreenshot {
                            error,
                            context: "metadata for temporary path",
                        })?
                        .dev()
                {
                    // copy file instead
                    fs::copy(tmp_path, &path).map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "copying screenshot",
                    })?;
                    fs::remove_file(tmp_path).map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "removing temporary screenshot",
                    })?;
                } else {
                    fs::rename(tmp_path, &path).map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "moving screenshot",
                    })?;
                }

                Ok(path.to_string_lossy().to_string())
            } else {
                Ok(uri.path().to_string())
            }
        }
        scheme => {
            error!("Unsupported URL scheme: {scheme}");
            Err(Error::Ashpd(ashpd::Error::Zbus(zbus::Error::Unsupported)))
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Init tracing but don't panic if it fails
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .try_init();

    let args = Args::parse();
    let notify = args.notify;

    let (summary, body) = match request_screenshot(args).await {
        Ok(path) => {
            info!("Screenshot saved to {path}");
            ("Screenshot captured", path)
        }
        Err(e) => {
            if !e.cancelled() {
                error!("Screenshot failed with {e}");
                ("Screenshot failed", e.to_user_facing())
            } else {
                info!("Screenshot cancelled");
                ("Screenshot cancelled", "".into())
            }
        }
    };

    if notify {
        if let Err(e) = send_notify(summary, &body).await {
            error!("Failed to post notification on completion: {e}");
        }
    }
}

use std::{collections::HashMap, fs, os::unix::fs::MetadataExt, path::PathBuf};

use ashpd::desktop::screenshot::Screenshot;
use clap::{ArgAction, Parser, command};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use zbus::{Connection, dbus_proxy, zvariant::Value};

use error::Error;

mod error;
mod localize;

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

#[proxy(assume_defaults = true)]
trait Notifications {
    /// Call the org.freedesktop.Notifications.Notify D-Bus method
    #[allow(clippy::too_many_arguments)]
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
    crate::localize::localize();

    let args = Args::parse();
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
    let path = match uri.scheme() {
        "file" => {
            let response_path = uri
                .to_file_path()
                .unwrap_or_else(|_| panic!("unsupported response URI '{uri}'"));
            if let Some(picture_dir) = picture_dir {
                let date = chrono::Local::now();
                let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
                let path = picture_dir.join(filename);
                if fs::metadata(&picture_dir)
                    .map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "metadata for screenshot destination",
                    })?
                    .dev()
                    != fs::metadata(&response_path)
                        .map_err(|error| Error::SaveScreenshot {
                            error,
                            context: "metadata for temporary path",
                        })?
                        .dev()
                {
                    // copy file instead
                    fs::copy(&response_path, &path).map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "copying screenshot",
                    })?;
                    fs::remove_file(&response_path).map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "removing temporary screenshot",
                    })?;
                } else {
                    fs::rename(&response_path, &path).map_err(|error| Error::SaveScreenshot {
                        error,
                        context: "moving screenshot",
                    })?;
                }

                path.to_string_lossy().to_string()
            } else {
                response_path.to_string_lossy().to_string()
            }
        }
        "clipboard" => String::new(),
        scheme => {
            error!("Unsupported URL scheme: {scheme}");
            return Err(Error::Ashpd(ashpd::Error::Zbus(zbus::Error::Unsupported)));
        }
    };

    info!("Saving screenshot to {path}");

    if args.notify {
        let connection = Connection::session().await.map_err(Error::Notify)?;

        let message = if path.is_empty() {
            fl!("screenshot-saved-to-clipboard")
        } else {
            fl!("screenshot-saved-to")
        };
        let proxy = NotificationsProxy::new(&connection)
            .await
            .map_err(Error::Notify)?;
        _ = proxy
            .notify(
                &fl!("cosmic-screenshot"),
                0,
                "com.system76.CosmicScreenshot",
                &message,
                &path,
                &[],
                HashMap::from([("transient", &Value::Bool(true))]),
                5000,
            )
            .await
            .map_err(Error::Notify)?;
    }

    Ok(())
}

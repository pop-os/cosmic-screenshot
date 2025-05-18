use ashpd::desktop::{screenshot::Screenshot, ResponseError};
use ashpd::Error;
use clap::{command, ArgAction, Parser};
use std::{collections::HashMap, fs, os::unix::fs::MetadataExt, path::PathBuf};
use zbus::{dbus_proxy, zvariant::Value, Connection};

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

async fn send_notification(args: &Args, summary: &str, body: &str) {
    if args.notify {
        let connection = Connection::session()
            .await
            .expect("failed to connect to session bus");

        let proxy = NotificationsProxy::new(&connection)
            .await
            .expect("failed to create proxy");
        _ = proxy
            .notify(
                "COSMIC Screenshot",
                0,
                "com.system76.CosmicScreenshot",
                summary,
                body,
                &[],
                HashMap::from([("transient", &Value::Bool(true))]),
                5000,
            )
            .await
            .expect("failed to send notification");
    }
}

//TODO: better error handling
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let args = Args::parse();
    let picture_dir = (!args.interactive).then(|| {
        args.clone()
            .save_dir
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(|| dirs::picture_dir().expect("failed to locate picture directory"))
    });

    let response_wrapped = Screenshot::request()
        .interactive(args.interactive)
        .modal(args.modal)
        .send()
        .await
        .expect("failed to send screenshot request")
        .response();

    let response = match response_wrapped {
        Ok(resp) => resp,

        Err(Error::Response(ResponseError::Cancelled)) => {
            let message = "Screenshot has been cancelled";
            send_notification(
                &args,
                message,
                "Your screenshot request has been cancelled.",
            )
            .await;

            return Err(String::from("screenshot cancelled"));
        }

        err => {
            let message =
                String::from(format!("Couldn't get screenshot result, error: {:?}", err,));
            send_notification(&args, "Error while capturing screenshot", &message).await;

            return Err(message);
        }
    };

    let uri = response.uri();
    let path = match uri.scheme() {
        "file" => {
            if let Some(picture_dir) = picture_dir {
                let date = chrono::Local::now();
                let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
                let path = picture_dir.join(filename);
                let tmp_path = uri.path();
                if fs::metadata(&picture_dir)
                    .expect("Failed to get medatata on filesystem for screenshot destination")
                    .dev()
                    != fs::metadata(tmp_path)
                        .expect("Failed to get metadata on filesystem for temporary path")
                        .dev()
                {
                    // copy file instead
                    fs::copy(tmp_path, &path).expect("failed to move screenshot");
                    fs::remove_file(tmp_path).expect("failed to remove temporary screenshot");
                } else {
                    fs::rename(tmp_path, &path).expect("failed to move screenshot");
                }

                path.to_string_lossy().to_string()
            } else {
                uri.path().to_string()
            }
        }
        "clipboard" => String::new(),
        scheme => panic!("unsupported scheme '{}'", scheme),
    };

    println!("{path}");

    let message = if path.is_empty() {
        "Screenshot saved to clipboard"
    } else {
        "Screenshot saved to:"
    };
    send_notification(&args, message, &path).await;

    Ok(())
}

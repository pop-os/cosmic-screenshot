use ashpd::desktop::screenshot::Screenshot;
use clap::{command, ArgAction, Parser};
use std::{collections::HashMap, fs, os::unix::fs::MetadataExt, path::PathBuf};
use strfmt::strfmt;
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
    /// The format of the new screenshot file's name
    #[clap(long, default_value("Screenshot_{date}.png"))]
    filename_format: String,
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

//TODO: better error handling
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let picture_dir = (!args.interactive).then(|| {
        args.save_dir
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(|| dirs::picture_dir().expect("failed to locate picture directory"))
    });

    let response = Screenshot::request()
        .interactive(args.interactive)
        .modal(args.modal)
        .send()
        .await
        .expect("failed to send screenshot request")
        .response()
        .expect("failed to receive screenshot response");

    let uri = response.uri();
    let path = match uri.scheme() {
        "file" => {
            if let Some(picture_dir) = picture_dir {
                let date = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                let filename =
                    strfmt!(&args.filename_format, date).expect("failed to format file name");
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

    if args.notify {
        let connection = Connection::session()
            .await
            .expect("failed to connect to session bus");

        let message = if path.is_empty() {
            "Screenshot saved to clipboard"
        } else {
            "Screenshot saved to:"
        };
        let proxy = NotificationsProxy::new(&connection)
            .await
            .expect("failed to create proxy");
        _ = proxy
            .notify(
                "COSMIC Screenshot",
                0,
                "com.system76.CosmicScreenshot",
                message,
                &path,
                &[],
                HashMap::from([("transient", &Value::Bool(true))]),
                5000,
            )
            .await
            .expect("failed to send notification");
    }
}

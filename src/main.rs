use ashpd::desktop::screenshot::Screenshot;
use clap::{ArgAction, Parser, command};
use std::{collections::HashMap, fs, os::unix::fs::MetadataExt, path::PathBuf};
use wl_clipboard_rs::copy::{MimeType, Options, Source, copy};
use zbus::{Connection, proxy, zvariant::Value};

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
    /// Copy the screenshot to clipboard when saving to file
    #[clap(long,
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set)]
    copy_to_clipboard: bool,
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

//TODO: better error handling
#[tokio::main(flavor = "current_thread")]
async fn main() {
    crate::localize::localize();

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
        .response();

    let response = match response {
        Err(err) => {
            if err.to_string().contains("Cancelled") {
                println!("Screenshot cancelled by user");
                std::process::exit(0);
            }
            eprintln!("Error taking screenshot: {}", err);
            std::process::exit(1);
        }
        Ok(response) => response,
    };

    let uri = response.uri();
    let (path, copied_to_clipboard) = match uri.scheme() {
        "file" => {
            let response_path = uri
                .to_file_path()
                .unwrap_or_else(|_| panic!("unsupported response URI '{uri}'"));
            if let Some(picture_dir) = picture_dir {
                let date = chrono::Local::now();
                let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
                let path = picture_dir.join(filename);
                if fs::metadata(&picture_dir)
                    .expect("Failed to get medatata on filesystem for screenshot destination")
                    .dev()
                    != fs::metadata(&response_path)
                        .expect("Failed to get metadata on filesystem for temporary path")
                        .dev()
                {
                    // copy file instead
                    fs::copy(&response_path, &path).expect("failed to move screenshot");
                    fs::remove_file(&response_path).expect("failed to remove temporary screenshot");
                } else {
                    fs::rename(&response_path, &path).expect("failed to move screenshot");
                }

                let mut copied = false;
                if args.copy_to_clipboard {
                    // Read file asynchronously then perform blocking clipboard copy in spawn_blocking
                    match tokio::fs::read(&path).await {
                        Ok(bytes) => {
                            let bytes_boxed = bytes.into_boxed_slice();
                            let result = tokio::task::spawn_blocking(move || {
                                let opts = Options::new();
                                let source = Source::Bytes(bytes_boxed);
                                copy(opts, source, MimeType::Autodetect)
                            })
                            .await;
                            match result {
                                Ok(Ok(())) => {
                                    copied = true;
                                }
                                Ok(Err(err)) => {
                                    eprintln!("Failed to copy screenshot to clipboard: {}", err);
                                }
                                Err(join_err) => {
                                    eprintln!(
                                        "Clipboard copy task panicked or was cancelled: {}",
                                        join_err
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read screenshot file for clipboard: {}", e);
                        }
                    }
                }

                (path.to_string_lossy().to_string(), copied)
            } else {
                (response_path.to_string_lossy().to_string(), false)
            }
        }
        "clipboard" => (String::new(), true),
        scheme => panic!("unsupported scheme '{}'", scheme),
    };

    println!("{path}");

    if args.notify {
        let connection = Connection::session()
            .await
            .expect("failed to connect to session bus");

        let message = if copied_to_clipboard && !path.is_empty() {
            fl!("screenshot-saved-to-and-clipboard")
        } else if copied_to_clipboard {
            fl!("screenshot-saved-to-clipboard")
        } else {
            fl!("screenshot-saved-to")
        };
        let proxy = NotificationsProxy::new(&connection)
            .await
            .expect("failed to create proxy");
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
            .expect("failed to send notification");
    }
}

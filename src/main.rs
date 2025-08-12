use ashpd::desktop::screenshot::Screenshot;
use clap::{command, ArgAction, Parser, Subcommand};
use cosmic_screenshot::screenshot::{ScreenshotKind, ScreenshotManager, ScreenshotOptions};
use cosmic_screenshot::dbus::{ScreenshotService, ScreenshotServiceInterface};
use cosmic_screenshot::app::CosmicScreenshotApp;
use cosmic_screenshot::settings::APP_ID;
use cosmic_screenshot::error_handling::{self, report_error, report_success, ErrorSeverity};
use cosmic_screenshot::notifications;
use cosmic::app::Settings;
use std::{collections::HashMap, fs, os::unix::fs::MetadataExt, path::PathBuf};
use zbus::{proxy, zvariant::Value, Connection};


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Enable interactive mode in the portal (legacy mode)
    #[clap(long,
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set)]
    interactive: bool,
    /// Enable modal mode in the portal (legacy mode)
    #[clap(long,
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set,)]
    modal: bool,
    /// Send a notification with the path to the saved screenshot (legacy mode)
    #[clap(long,
        default_missing_value("true"),
        default_value("true"),
        num_args(0..=1),
        require_equals(true),
        action = ArgAction::Set)]
    notify: bool,
    /// The directory to save the screenshot to, if not performing an interactive screenshot (legacy mode)
    #[clap(short, long)]
    save_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the D-Bus service
    Service,
    /// Take a screenshot using the new interface
    Take {
        /// Screenshot type: all, screen, window, select, region
        #[clap(short, long, default_value = "all")]
        kind: String,
        /// Delay in milliseconds
        #[clap(short, long, default_value = "0")]
        delay: u32,
        /// Save to clipboard
        #[clap(short = 'c', long)]
        clipboard: bool,
        /// Directory to save screenshot
        #[clap(short = 'o', long)]
        output_dir: Option<PathBuf>,
        /// Force specific backend: auto, kwin, portal
        #[clap(short = 'b', long, default_value = "auto")]
        backend: String,
    },
    /// Launch GUI application
    Gui,
    /// List available screenshot backends
    Backends,
    /// Test D-Bus service methods
    TestDbus,
    /// Generate D-Bus interface XML from implementation
    GenerateDbusXml,
}

#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
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

#[proxy(
    interface = "com.system76.CosmicScreenshot",
    default_service = "com.system76.CosmicScreenshot",
    default_path = "/com/system76/CosmicScreenshot"
)]
trait CosmicScreenshotProxy {
    /// Take a screenshot with the specified options
    fn take_screenshot(
        &self,
        kind: &str,
        delay_ms: u32,
        save_to_clipboard: bool,
        save_dir: &str,
    ) -> zbus::Result<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>;

    /// Take a screenshot with backend selection
    fn take_screenshot_with_backend(
        &self,
        kind: &str,
        delay_ms: u32,
        save_to_clipboard: bool,
        save_dir: &str,
        backend: &str,
    ) -> zbus::Result<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>;

    /// Get available screenshot backends
    fn get_available_backends(&self) -> zbus::Result<Vec<String>>;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Create a single tokio runtime for all async operations
    let rt = tokio::runtime::Runtime::new()?;

    match args.command {
        Some(Commands::Service) => {
            run_dbus_service(&rt)
        }
        Some(Commands::Gui) => {
            println!("Starting GUI mode...");
            
            // Enable GUI mode for error handling
            error_handling::set_gui_mode(true);
            
            // Initialize system notifications
            rt.block_on(notifications::init_notification_manager());
            
            // Force fastest GPU backend for performance
            std::env::set_var("WGPU_BACKEND", "primary");
            std::env::set_var("WGPU_POWER_PREF", "high-performance");
            
            let settings = Settings::default()
                .antialiasing(true);
            cosmic::app::run::<CosmicScreenshotApp>(settings, ())?;
            Ok(())
        }
        Some(Commands::Take { kind, delay, clipboard, output_dir, backend }) => {
            run_screenshot_command(&rt, &kind, delay, clipboard, output_dir, &backend)
        }
        Some(Commands::Backends) => {
            run_backends_command(&rt)
        }
        Some(Commands::TestDbus) => {
            run_test_dbus_command(&rt)
        }
        Some(Commands::GenerateDbusXml) => {
            generate_dbus_xml(&rt)
        }
        None => {
            run_legacy_screenshot(&rt, args)
        }
    }
}

/// Check if the D-Bus service is available
async fn is_dbus_service_available() -> bool {
    match Connection::session().await {
        Ok(conn) => {
            // Check if our service name is available on the bus
            match conn.call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &(APP_ID,),
            ).await {
                Ok(response) => {
                    response.body().deserialize::<bool>().unwrap_or_default()
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

/// Delegate screenshot command to existing D-Bus service
async fn delegate_to_dbus_service(
    kind: &str,
    delay: u32,
    clipboard: bool,
    output_dir: Option<PathBuf>,
    backend: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if this is region selection - D-Bus service doesn't support it
    if kind == "region" {
        if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
            eprintln!("Region selection not supported via D-Bus, falling back to direct execution");
        }
        return Err("Region selection not supported via D-Bus".into());
    }
    
    let connection = Connection::session().await?;
    let proxy = CosmicScreenshotProxyProxy::new(&connection).await?;
    
    let save_dir = output_dir
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    
    let result = if backend == "auto" {
        proxy.take_screenshot(kind, delay, clipboard, &save_dir).await?
    } else {
        proxy.take_screenshot_with_backend(kind, delay, clipboard, &save_dir, backend).await?
    };
    
    // Parse the response
    if let Some(path_value) = result.get("path") {
        if let Ok(path) = <&str>::try_from(path_value) {
            println!("Screenshot saved to: {path}");
            report_success("Screenshot Saved", &format!("Screenshot saved to {path}"));
        }
    }
    if let Some(clipboard_value) = result.get("saved_to_clipboard") {
        if let Ok(saved_to_clipboard) = bool::try_from(clipboard_value) {
            if saved_to_clipboard {
                println!("Screenshot saved to clipboard");
                report_success("Clipboard", "Screenshot copied to clipboard");
            }
        }
    }
    
    Ok(())
}

fn run_dbus_service(rt: &tokio::runtime::Runtime) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting D-Bus service...");
    
    rt.block_on(async {
        let service = ScreenshotServiceInterface::new().await?;
        println!("D-Bus service started on {APP_ID}");
        service.run().await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    })?;
    
    Ok(())
}

fn run_screenshot_command(
    rt: &tokio::runtime::Runtime,
    kind: &str,
    delay: u32,
    clipboard: bool,
    output_dir: Option<PathBuf>,
    backend: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    
    // Check if D-Bus service is available and delegate to it
    let should_delegate = rt.block_on(async {
        is_dbus_service_available().await
    });
    
    if should_delegate {
        if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
            eprintln!("Delegating to existing D-Bus service");
        }
        // Try to delegate, but fall back to direct execution if it fails (e.g., for region selection)
        match rt.block_on(async {
            delegate_to_dbus_service(kind, delay, clipboard, output_dir.clone(), backend).await
        }) {
            Ok(()) => return Ok(()),
            Err(_) => {
                if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
                    eprintln!("D-Bus delegation failed, falling back to direct execution");
                }
                // Continue with direct execution below
            }
        }
    }

    if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
        eprintln!("No D-Bus service available, running directly");
    }

    let screenshot_kind = match kind {
        "all" => ScreenshotKind::AllScreens,
        "screen" => ScreenshotKind::ScreenUnderCursor,
        "window" => ScreenshotKind::WindowUnderCursor,
        "select" => ScreenshotKind::SelectScreen,
        "region" => ScreenshotKind::RectangularRegion,
        _ => {
            report_error(ErrorSeverity::Error, "Invalid Input", &format!("Invalid screenshot kind: {kind}"));
            std::process::exit(1);
        }
    };

    // For region selection, we need to launch GUI mode
    if screenshot_kind == ScreenshotKind::RectangularRegion {
        println!("Region selection requires GUI mode, launching snipper...");
        
        // Enable GUI mode for error handling
        error_handling::set_gui_mode(true);
        
        // Initialize system notifications
        rt.block_on(notifications::init_notification_manager());
        
        // Set CLI mode options as environment variables for the GUI to pick up
        if delay > 0 {
            std::env::set_var("CLI_DELAY", delay.to_string());
        }
        if clipboard {
            std::env::set_var("CLI_CLIPBOARD", "true");
        }
        if let Some(ref output_dir) = output_dir {
            std::env::set_var("CLI_OUTPUT_DIR", output_dir.to_string_lossy().to_string());
        }
        std::env::set_var("CLI_BACKEND", backend);
        std::env::set_var("CLI_MODE_REGION", "true");
        
        // Force fastest GPU backend for performance
        std::env::set_var("WGPU_BACKEND", "primary");
        std::env::set_var("WGPU_POWER_PREF", "high-performance");
        
        let settings = Settings::default()
            .antialiasing(true);
        return Ok(cosmic::app::run::<CosmicScreenshotApp>(settings, ())?);
    }

    rt.block_on(async {
        let manager = ScreenshotManager::new();
        let options = ScreenshotOptions {
            kind: screenshot_kind,
            delay_ms: delay,
            save_to_clipboard: clipboard,
            save_dir: output_dir,
        };

        let backend_name = if backend == "auto" { None } else { Some(backend) };
        match manager.take_screenshot_with_backend(&options, backend_name).await {
            Ok(result) => {
                if let Some(path) = result.path {
                    println!("Screenshot saved to: {}", path.display());
                    report_success("Screenshot Saved", &format!("Screenshot saved to {}", path.display()));
                }
                if result.saved_to_clipboard {
                    println!("Screenshot saved to clipboard");
                    report_success("Clipboard", "Screenshot copied to clipboard");
                }
                Ok(())
            }
            Err(err) => {
                report_error(ErrorSeverity::Error, "Screenshot Failed", &format!("Failed to take screenshot: {err}"));
                std::process::exit(1);
            }
        }
    })
}

fn run_backends_command(rt: &tokio::runtime::Runtime) -> Result<(), Box<dyn std::error::Error>> {
    rt.block_on(async {
        // Check if D-Bus service is available and delegate to it
        if is_dbus_service_available().await {
            if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
                eprintln!("Delegating backends query to existing D-Bus service");
            }
            
            let connection = Connection::session().await?;
            let proxy = CosmicScreenshotProxyProxy::new(&connection).await?;
            
            let backends = proxy.get_available_backends().await?;
            println!("Available screenshot backends:");
            for backend in backends {
                println!("  - {backend}");
            }
            return Ok(());
        }

        if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
            eprintln!("No D-Bus service available, querying backends directly");
        }

        let manager = ScreenshotManager::new();
        let backends = manager.get_available_grabbers().await;
        println!("Available screenshot backends:");
        for backend in backends {
            println!("  - {backend}");
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}

fn run_test_dbus_command(rt: &tokio::runtime::Runtime) -> Result<(), Box<dyn std::error::Error>> {
    rt.block_on(async {
    println!("Testing D-Bus API functionality...");
    let manager = ScreenshotManager::new();
    
    println!("\n1. Available backends:");
    let backends = manager.get_available_grabbers().await;
    for backend in &backends {
        println!("   - {backend}");
    }
    
    println!("\n2. Backend capabilities:");
    let capabilities = manager.get_backend_capabilities().await;
    for (backend, kinds) in capabilities {
        println!("   {backend}: {kinds:?}");
    }
    
    println!("\n3. Testing individual backend capabilities:");
    for backend in &backends {
        for kind in ["all", "screen", "window", "select", "region"] {
            let screenshot_kind = match kind {
                "all" => ScreenshotKind::AllScreens,
                "screen" => ScreenshotKind::ScreenUnderCursor,
                "window" => ScreenshotKind::WindowUnderCursor,
                "select" => ScreenshotKind::SelectScreen,
                "region" => ScreenshotKind::RectangularRegion,
                _ => continue,
            };
            let supports = manager.supports_kind_with_backend(screenshot_kind, backend).await;
            println!("   {backend} supports {kind}: {supports}");
        }
    }
    Ok::<(), Box<dyn std::error::Error>>(())
    })
}

fn run_legacy_screenshot(rt: &tokio::runtime::Runtime, args: Args) -> Result<(), Box<dyn std::error::Error>> {
    rt.block_on(async {
    let picture_dir = (!args.interactive).then(|| {
        args.save_dir
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(|| dirs::picture_dir().expect("failed to locate picture directory"))
    });

    let response = Screenshot::request()
        .interactive(args.interactive)
        .modal(args.modal)
        .send()
        .await?
        .response()?;

    let uri = response.uri();
    let path = match uri.scheme() {
        "file" => {
            if let Some(picture_dir) = picture_dir {
                let date = chrono::Local::now();
                let filename = format!("Screenshot_{}.png", date.format("%Y-%m-%d_%H-%M-%S"));
                let path = picture_dir.join(filename);
                let tmp_path = uri.path();
                if fs::metadata(&picture_dir)?.dev() == fs::metadata(tmp_path)?.dev() {
                    fs::rename(tmp_path, &path)?;
                } else {
                    fs::copy(tmp_path, &path)?;
                    fs::remove_file(tmp_path)?;
                }
                path.to_string_lossy().to_string()
            } else {
                uri.path().to_string()
            }
        }
        "clipboard" => String::new(),
        scheme => return Err(format!("unsupported scheme '{scheme}'").into()),
    };

    println!("{path}");

    if args.notify {
        let connection = Connection::session().await?;
        let message = if path.is_empty() {
            "Screenshot saved to clipboard"
        } else {
            "Screenshot saved to:"
        };
        let proxy = NotificationsProxy::new(&connection).await?;
        proxy
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
            .await?;
    }

    Ok::<(), Box<dyn std::error::Error>>(())
    })
}

fn generate_dbus_xml(rt: &tokio::runtime::Runtime) -> Result<(), Box<dyn std::error::Error>> {
    rt.block_on(async {
        // Create connection with a unique service name
        let connection = Connection::session().await?;
        
        // Create and register our service at the object server
        let service = ScreenshotService::new();
        let _interface_ref = connection
            .object_server()
            .at("/com/system76/CosmicScreenshot", service)
            .await?;
        
        // Request a service name to ensure the service is properly registered
        connection.request_name("com.system76.CosmicScreenshotTemp").await?;
        
        // Give a moment for the service to be fully registered
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        // Get introspection XML by calling the D-Bus introspection method
        let introspection_xml = connection
            .call_method(
                Some("com.system76.CosmicScreenshotTemp"),
                "/com/system76/CosmicScreenshot", 
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await?;
        
        // Extract the XML string from the response
        let xml: String = introspection_xml.body().deserialize()?;
        println!("{xml}");
        
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
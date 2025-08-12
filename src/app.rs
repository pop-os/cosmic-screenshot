// SPDX-License-Identifier: GPL-3.0-only

use crate::ui::{ScreenshotMessage, ScreenshotWidget};
use cosmic::app::ApplicationExt;
use cosmic::iced::{event, window};
use cosmic::{app, Element};

// GUI Application Implementation
pub struct CosmicScreenshotApp {
    core: app::Core,
    screenshot_widget: ScreenshotWidget,
    snipper_window: Option<window::Id>,
    cli_region_mode: bool,
}

impl app::Application for CosmicScreenshotApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = ScreenshotMessage;

    const APP_ID: &'static str = "com.system76.CosmicScreenshot";

    fn core(&self) -> &app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn init(
        core: app::Core,
        _flags: Self::Flags,
    ) -> (Self, cosmic::Task<cosmic::Action<Self::Message>>) {
        let cli_region_mode = std::env::var("CLI_MODE_REGION").is_ok();

        let app = Self {
            core,
            screenshot_widget: ScreenshotWidget::new(),
            snipper_window: None,
            cli_region_mode,
        };

        (app, ScreenshotWidget::init().map(cosmic::Action::App))
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn view(&self) -> Element<'_, Self::Message> {
        if self.cli_region_mode {
            // In CLI region mode, show minimal empty view since main window is hidden
            cosmic::widget::container(cosmic::widget::text(""))
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .into()
        } else {
            self.screenshot_widget.view()
        }
    }

    fn view_window(&self, window_id: cosmic::iced::window::Id) -> Element<'_, Self::Message> {
        if Some(window_id) == self.snipper_window {
            // This is the snipper window - show fullscreen snipper interface
            if let Some(ref snipper) = self.screenshot_widget.snipper {
                return snipper.view();
            }
            // Fallback for snipper window if snipper is None
            return cosmic::widget::container(cosmic::widget::text("Snipper loading..."))
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .style(|_theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Color::from_rgba(0.0, 0.0, 0.0, 0.8).into()),
                    ..Default::default()
                })
                .into();
        }

        if Some(window_id) == self.screenshot_widget.error_dialog_window_id {
            // This is the error dialog window
            if let Some((ref title, ref message)) = self.screenshot_widget.error_dialog {
                let spacing = cosmic::theme::active().cosmic().spacing;
                return cosmic::widget::container(
                    cosmic::widget::column()
                        .push(cosmic::widget::text::title3(title))
                        .push(cosmic::widget::vertical_space())
                        .push(cosmic::widget::text(message))
                        .push(cosmic::widget::vertical_space())
                        .push(
                            cosmic::widget::row()
                                .push(cosmic::widget::horizontal_space())
                                .push(
                                    cosmic::widget::button::standard("OK")
                                        .on_press(ScreenshotMessage::DismissErrorDialog),
                                ),
                        )
                        .spacing(spacing.space_s)
                        .max_width(400),
                )
                .padding(spacing.space_m)
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .into();
            }
        }

        // This is the main window - show main interface only
        self.view()
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Self::Message) -> cosmic::Task<cosmic::Action<Self::Message>> {
        // Handle window lifecycle messages
        match message {
            ScreenshotMessage::MainWindowOpened(window_id) => {
                // Handle OS-level window open events - used for CLI mode logic only
                // Note: This receives ALL window opens (main + snipper windows)
                // In CLI region mode, minimize the main window immediately
                if self.cli_region_mode {
                    // Check if this is actually the main window
                    if Some(window_id) == self.core.main_window_id() {
                        println!("CLI mode: hiding main window");
                        return window::minimize(window_id, true).map(cosmic::Action::App);
                    }
                }
            }
            ScreenshotMessage::SnipperWindowOpened(window_id) => {
                // Handle application-level snipper window creation (not OS window events)
                // This is sent immediately when creating a snipper window to set up state
                self.snipper_window = Some(window_id);
                // Set unique title for KWin to identify this window
                return self.set_window_title("cosmic-screenshot-snipper".to_string(), window_id);
            }
            ScreenshotMessage::SnipperWindowClosed(window_id) => {
                if Some(window_id) == self.snipper_window {
                    self.snipper_window = None;
                    // Also clear the window ID in the screenshot widget
                    self.screenshot_widget.snipper_window_id = None;
                }
            }
            ScreenshotMessage::ErrorDialogClosed(window_id) => {
                if Some(window_id) == self.screenshot_widget.error_dialog_window_id {
                    self.screenshot_widget.error_dialog_window_id = None;
                    self.screenshot_widget.error_dialog = None;
                }
            }
            ScreenshotMessage::WindowCloseRequested(window_id) => {
                // Route to specific handlers based on window type
                if Some(window_id) == self.screenshot_widget.error_dialog_window_id {
                    return cosmic::Task::perform(
                        async move { ScreenshotMessage::ErrorDialogClosed(window_id) },
                        cosmic::Action::App,
                    );
                } else if Some(window_id) == self.snipper_window {
                    return cosmic::Task::perform(
                        async move { ScreenshotMessage::SnipperWindowClosed(window_id) },
                        cosmic::Action::App,
                    );
                }
            }
            ScreenshotMessage::WindowClosed(window_id) => {
                // Route to specific handlers based on window type
                if Some(window_id) == self.screenshot_widget.error_dialog_window_id {
                    return cosmic::Task::perform(
                        async move { ScreenshotMessage::ErrorDialogClosed(window_id) },
                        cosmic::Action::App,
                    );
                } else if Some(window_id) == self.snipper_window {
                    return cosmic::Task::perform(
                        async move { ScreenshotMessage::SnipperWindowClosed(window_id) },
                        cosmic::Action::App,
                    );
                }
            }
            ScreenshotMessage::CloseSnipperWindow => {
                // Actually close the snipper window (only when truly closing, not hiding)
                if let Some(window_id) = self.snipper_window {
                    println!("Main app closing snipper window: {window_id:?}");
                    self.snipper_window = None;
                    self.screenshot_widget.snipper_window_id = None;
                    return window::close(window_id).map(cosmic::Action::App);
                }
            }
            ScreenshotMessage::HideSnipperWindow => {
                // Hide the snipper window by minimizing it
                if let Some(window_id) = self.snipper_window {
                    println!("Main app hiding snipper window: {window_id:?}");
                    return window::minimize(window_id, true).map(cosmic::Action::App);
                }
            }
            ScreenshotMessage::ShowSnipperWindow => {
                // Show the snipper window by unminimizing it and bringing it to front
                if let Some(window_id) = self.snipper_window {
                    println!("Main app showing snipper window: {window_id:?}");

                    // Check if we're on Wayland or X11
                    let is_wayland = std::env::var("XDG_SESSION_TYPE")
                        .map(|session_type| session_type == "wayland")
                        .unwrap_or(false);

                    // Check if we're running under KWin
                    let is_kwin = std::env::var("DESKTOP_SESSION")
                        .map(|session| session.contains("plasma") || session.contains("kde"))
                        .unwrap_or(false)
                        || std::env::var("XDG_CURRENT_DESKTOP")
                            .map(|desktop| desktop.contains("KDE"))
                            .unwrap_or(false);

                    if is_wayland && is_kwin {
                        // KWin on Wayland: Use KWin scripting API for proper window raising
                        return cosmic::Task::batch([
                            window::minimize(window_id, false).map(cosmic::Action::App),
                            window::maximize(window_id, true).map(cosmic::Action::App),
                            cosmic::Task::perform(raise_window_kwin(window_id), |result| {
                                if let Err(e) = result {
                                    println!("Failed to raise window via KWin: {e}");
                                } else {
                                    println!("Successfully raised window via KWin");
                                }
                                // Return a dummy message that won't trigger anything
                                ScreenshotMessage::BackendsLoaded(vec![])
                            })
                            .map(cosmic::Action::App),
                        ]);
                    } else if is_wayland {
                        // Other Wayland compositors: Use activation token approach
                        return cosmic::Task::batch([
                            window::minimize(window_id, false).map(cosmic::Action::App),
                            window::maximize(window_id, true).map(cosmic::Action::App),
                            cosmic::iced_winit::platform_specific::wayland::commands::activation::request_token(
                                Some("cosmic-screenshot".to_string()),
                                Some(window_id)
                            ).then(move |token| {
                                if let Some(token) = token {
                                    cosmic::iced_winit::platform_specific::wayland::commands::activation::activate(window_id, token)
                                } else {
                                    cosmic::Task::none()
                                }
                            }),
                        ]);
                    }
                    // X11: Use gain_focus approach
                    return cosmic::Task::batch([
                        window::minimize(window_id, false).map(cosmic::Action::App),
                        window::maximize(window_id, true).map(cosmic::Action::App),
                        window::gain_focus(window_id).map(cosmic::Action::App),
                    ]);
                }
            }
            ScreenshotMessage::Exit => {
                // Exit the application gracefully by closing main window
                if let Some(main_window) = self.core.main_window_id() {
                    return window::close(main_window).map(cosmic::Action::App);
                }
            }
            ScreenshotMessage::OpenErrorDialog(title, message) => {
                // Store the error dialog content first
                self.screenshot_widget.error_dialog = Some((title.clone(), message.clone()));

                // Create error dialog window
                let window_settings = window::Settings {
                    size: cosmic::iced::Size::new(400.0, 200.0),
                    position: window::Position::Centered,
                    resizable: false,
                    decorations: true,
                    ..Default::default()
                };

                return cosmic::Task::batch([window::open(window_settings)
                    .1
                    .map(ScreenshotMessage::ErrorDialogOpened)])
                .map(cosmic::Action::App);
            }
            ScreenshotMessage::ErrorDialogOpened(window_id) => {
                self.screenshot_widget.error_dialog_window_id = Some(window_id);
                return self.set_window_title("Error".to_string(), window_id);
            }
            ScreenshotMessage::DismissErrorDialog => {
                // Close the error dialog window
                if let Some(window_id) = self.screenshot_widget.error_dialog_window_id {
                    self.screenshot_widget.error_dialog_window_id = None;
                    self.screenshot_widget.error_dialog = None;
                    return window::close(window_id).map(cosmic::Action::App);
                }
            }
            _ => {}
        }

        self.screenshot_widget
            .update(message)
            .map(cosmic::Action::App)
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        let mut subscriptions = vec![];

        // Window event subscription - handles OS-level window events
        // Note: This sends MainWindowOpened for ALL windows (main + snipper)
        // MainWindowOpened is used for CLI mode logic, not snipper setup
        subscriptions.push(event::listen_with(|event, _, window_id| {
            if let cosmic::iced::Event::Window(window_event) = event {
                match window_event {
                    cosmic::iced::window::Event::Opened { .. } => {
                        // Send OS window open event - used for CLI mode window hiding
                        // SnipperWindowOpened is sent separately when creating windows
                        Some(ScreenshotMessage::MainWindowOpened(window_id))
                    }
                    cosmic::iced::window::Event::CloseRequested => {
                        // Send generic close event, handlers will check their own windows
                        Some(ScreenshotMessage::WindowCloseRequested(window_id))
                    }
                    cosmic::iced::window::Event::Closed => {
                        // Send generic closed event, handlers will check their own windows
                        Some(ScreenshotMessage::WindowClosed(window_id))
                    }
                    _ => None,
                }
            } else {
                None
            }
        }));

        // Snipper subscription when in region selection mode
        if self.screenshot_widget.region_selection_mode {
            subscriptions.push(
                crate::snipper::Snipper::subscription()
                    .map(|snipper_msg| ScreenshotMessage::SnipperMessage(snipper_msg)),
            );
        }

        cosmic::iced::Subscription::batch(subscriptions)
    }
}

/// Helper function to raise a window using `KWin`'s scripting API
async fn raise_window_kwin(_window_id: cosmic::iced::window::Id) -> Result<(), String> {
    use std::io::Write;
    use zbus::Connection;

    // KWin script to find and raise the window (matching kdotool format)
    let script = r#"
function output_debug(message) {
    // Empty debug for now
}

function output_error(message) {
    print("cosmic-screenshot ERROR", message);
}

function output_result(message) {
    if (message == null) {
        message = "null";
    }
    print("cosmic-screenshot RESULT", message);
}

// KDE 6 functions (assume KDE 6 for now)
workspace_windowList = () => workspace.windowList();
workspace_activeWindow = () => workspace.activeWindow;
workspace_setActiveWindow = (window) => { workspace.activeWindow = window; };
workspace_raiseWindow = (window) => { 
    if (workspace.raiseWindow) {
        workspace.raiseWindow(window); 
    } else {
        output_error("`windowraise` unsupported in this KDE version");
    }
};

function run() {
    output_debug("Looking for cosmic-screenshot-snipper window");
    
    // Find window by checking all clients
    let targetWindow = null;
    let windowList = workspace_windowList();
    
    for (let i = 0; i < windowList.length; i++) {
        let w = windowList[i];
        // Look specifically for the snipper window by its unique title
        if (w.caption && w.caption.includes('cosmic-screenshot-snipper')) {
            targetWindow = w;
            break; // Found the exact window we want
        }
    }
    
    if (targetWindow) {
        output_debug("Found cosmic-screenshot-snipper window, raising it");
        // First activate the window
        workspace_setActiveWindow(targetWindow);
        // Then raise it to front
        workspace_raiseWindow(targetWindow);
        output_result("Snipper window raised successfully");
    } else {
        output_error("cosmic-screenshot-snipper window not found");
    }
}

run();
    "#
    .to_string();

    // Connect to KWin's scripting D-Bus interface
    let connection = Connection::session().await.map_err(|e| e.to_string())?;

    // Create a proxy for KWin's scripting interface
    let proxy = zbus::Proxy::new(
        &connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )
    .await
    .map_err(|e| e.to_string())?;

    // Create a temporary script file (KWin expects a file path, not inline script)
    let mut temp_file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    temp_file
        .write_all(script.as_bytes())
        .map_err(|e| e.to_string())?;
    let temp_path = temp_file.path().to_str().ok_or("Invalid temp path")?;

    // Make script name unique to avoid conflicts
    let script_name = format!(
        "cosmic-screenshot-raise-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    // Generate unique script name to avoid conflicts

    // Load script into KWin
    println!("Loading KWin script from: {temp_path}");
    let result: Result<i32, _> = proxy
        .call("loadScript", &(temp_path, script_name.clone()))
        .await;
    let script_id = match result {
        Ok(id) => {
            println!("KWin script loaded with ID: {id}");
            if id < 0 {
                return Err(format!("KWin returned negative script ID: {id}"));
            }
            id
        }
        Err(e) => {
            return Err(format!("Failed to call loadScript: {e}"));
        }
    };

    // Create a proxy for the specific script instance
    let script_path = format!("/Scripting/Script{script_id}");
    let script_proxy = zbus::Proxy::new(
        &connection,
        "org.kde.KWin",
        script_path.as_str(),
        "org.kde.kwin.Script",
    )
    .await
    .map_err(|e| e.to_string())?;

    // Run the script
    script_proxy
        .call::<_, _, ()>("run", &())
        .await
        .map_err(|e| e.to_string())?;

    // Stop and unload the script
    script_proxy
        .call::<_, _, ()>("stop", &())
        .await
        .map_err(|e| e.to_string())?;
    let _: Result<(), _> = proxy.call("unloadScript", &(script_id,)).await;

    Ok(())
}

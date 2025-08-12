// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::{event, keyboard, mouse, widget::canvas, Color, Point, Rectangle, Size};
use std::collections::HashMap;
use std::time::{Duration, Instant};

type Message = crate::ui::ScreenshotMessage;


#[derive(Debug, Clone)]
pub enum SnipperMessage {
    StartSelection(Point),
    UpdateSelection(Point),
    EndSelection,
    AcceptSelection, // Double-click or Enter to accept
    CancelSelection,
    KeyPressed(keyboard::Key),
    DoubleClick(Point),
}

#[derive(Debug, Clone)]
pub enum DragMode {
    None,
    Creating,        // Creating new selection
    Moving,          // Moving existing selection
    ResizingTopLeft,
    ResizingTop,
    ResizingTopRight,
    ResizingRight,
    ResizingBottomRight,
    ResizingBottom,
    ResizingBottomLeft,
    ResizingLeft,
}

#[derive(Debug, Clone)]
pub struct SnipperState {
    selection: Option<Rectangle>,
    drag_mode: DragMode,
    drag_start: Point,
    initial_selection: Option<Rectangle>,
    current_mouse: Point,
    screen_images: HashMap<String, Vec<u8>>, // Screen name -> image data
    screen_bounds: Rectangle,
    cached_image_handle: Option<cosmic::iced::widget::image::Handle>,
    // Double-click detection
    last_click_time: Option<std::time::Instant>,
    last_click_pos: Option<Point>,
    // Selection memory - remember last selection position and size
    remembered_selection: Option<Rectangle>,
    // Debugging and profiling (compile-time conditional)
    #[cfg(feature = "debug")]
    debug_enabled: bool,
    #[cfg(feature = "debug")]
    event_timestamps: Vec<(String, Instant)>,
    #[cfg(feature = "debug")]
    last_event_time: Option<Instant>,
    #[cfg(feature = "debug")]
    last_mouse_event: Option<Instant>,
    #[cfg(feature = "debug")]
    selection_changed_time: Option<Instant>,
    #[cfg(feature = "debug")]
    last_cache_clear: Option<Instant>,
    #[cfg(feature = "debug")]
    last_significant_selection: Option<Rectangle>,
}

impl Default for SnipperState {
    fn default() -> Self {
        Self {
            selection: None,
            drag_mode: DragMode::None,
            drag_start: Point::ORIGIN,
            initial_selection: None,
            current_mouse: Point::ORIGIN,
            screen_images: HashMap::new(),
            screen_bounds: Rectangle::new(Point::ORIGIN, Size::ZERO),
            cached_image_handle: None,
            last_click_time: None,
            last_click_pos: None,
            remembered_selection: None,
            #[cfg(feature = "debug")]
            debug_enabled: std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok(),
            #[cfg(feature = "debug")]
            event_timestamps: Vec::new(),
            #[cfg(feature = "debug")]
            last_event_time: None,
            #[cfg(feature = "debug")]
            last_mouse_event: None,
            #[cfg(feature = "debug")]
            selection_changed_time: None,
            #[cfg(feature = "debug")]
            last_cache_clear: None,
            #[cfg(feature = "debug")]
            last_significant_selection: None,
        }
    }
}

impl SnipperState {
    #[must_use] 
    pub fn new(screen_images: HashMap<String, Vec<u8>>, screen_bounds: Rectangle) -> Self {
        #[cfg(feature = "debug")]
        let debug_enabled = std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok();
        #[cfg(feature = "debug")]
        let image_processing_start = if debug_enabled { Some(Instant::now()) } else { None };
        
        // Pre-cache the image handle during creation for better performance
        let cached_image_handle = if let Some(screenshot_data) = screen_images.get("primary") {
            #[cfg(feature = "debug")]
            if debug_enabled {
                eprintln!("[SNIPPER DEBUG] Loading image data of {} bytes", screenshot_data.len());
            }
            
            #[cfg(feature = "debug")]
            let decode_start = if debug_enabled { Some(Instant::now()) } else { None };
            
            if let Ok(img) = image::load_from_memory(screenshot_data) {
                #[cfg(feature = "debug")]
                if let Some(decode_start_time) = decode_start {
                    let decode_duration = decode_start_time.elapsed();
                    if decode_duration.as_millis() > 50 {
                        eprintln!("[IMAGE PERF WARNING] Image decode took {}ms (>50ms threshold)", decode_duration.as_millis());
                    }
                }
                
                #[cfg(feature = "debug")]
                let convert_start = if debug_enabled { Some(Instant::now()) } else { None };
                let rgba_img = img.to_rgba8();
                let (width, height) = rgba_img.dimensions();
                
                #[cfg(feature = "debug")]
                if let Some(convert_start_time) = convert_start {
                    let convert_duration = convert_start_time.elapsed();
                    if convert_duration.as_millis() > 30 {
                        eprintln!("[IMAGE PERF WARNING] RGBA conversion took {}ms (>30ms threshold)", convert_duration.as_millis());
                    }
                }
                
                #[cfg(feature = "debug")]
                let handle_start = if debug_enabled { Some(Instant::now()) } else { None };
                let handle = cosmic::iced::widget::image::Handle::from_rgba(
                    width,
                    height,
                    rgba_img.into_raw(),
                );
                
                #[cfg(feature = "debug")]
                if let Some(handle_start_time) = handle_start {
                    let handle_duration = handle_start_time.elapsed();
                    if handle_duration.as_millis() > 20 {
                        eprintln!("[IMAGE PERF WARNING] Handle creation took {}ms (>20ms threshold)", handle_duration.as_millis());
                    }
                }
                
                #[cfg(feature = "debug")]
                if debug_enabled {
                    eprintln!("[SNIPPER DEBUG] Successfully created image handle {width}x{height}");
                }
                
                Some(handle)
            } else {
                #[cfg(feature = "debug")]
                if debug_enabled {
                    eprintln!("[SNIPPER ERROR] Failed to decode image data");
                }
                None
            }
        } else {
            #[cfg(feature = "debug")]
            if debug_enabled {
                eprintln!("[SNIPPER ERROR] No 'primary' screen image found");
            }
            None
        };
        
        #[cfg(feature = "debug")]
        if let Some(processing_start_time) = image_processing_start {
            let total_processing = processing_start_time.elapsed();
            if total_processing.as_millis() > 100 {
                eprintln!("[IMAGE PERF WARNING] Total image processing took {}ms (>100ms threshold)", total_processing.as_millis());
            }
        }

        Self {
            selection: None,
            drag_mode: DragMode::None,
            drag_start: Point::ORIGIN,
            initial_selection: None,
            current_mouse: Point::ORIGIN,
            screen_images,
            screen_bounds,
            cached_image_handle,
            last_click_time: None,
            last_click_pos: None,
            remembered_selection: None,
            #[cfg(feature = "debug")]
            debug_enabled: std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok(),
            #[cfg(feature = "debug")]
            event_timestamps: Vec::new(),
            #[cfg(feature = "debug")]
            last_event_time: None,
            #[cfg(feature = "debug")]
            last_mouse_event: None,
            #[cfg(feature = "debug")]
            selection_changed_time: None,
            #[cfg(feature = "debug")]
            last_cache_clear: None,
            #[cfg(feature = "debug")]
            last_significant_selection: None,
        }
    }
    
    #[must_use] 
    pub fn new_with_memory(screen_images: HashMap<String, Vec<u8>>, screen_bounds: Rectangle, remembered_selection: Option<Rectangle>) -> Self {
        let mut state = Self::new(screen_images, screen_bounds);
        state.remembered_selection = remembered_selection;
        // If we have a remembered selection and it fits in the new screen bounds, restore it
        if let Some(remembered) = remembered_selection {
            if screen_bounds.contains(Point::new(remembered.x, remembered.y)) && 
               screen_bounds.contains(Point::new(remembered.x + remembered.width, remembered.y + remembered.height)) {
                state.selection = Some(remembered);
            }
        }
        state
    }

    pub fn selection(&self) -> Option<Rectangle> {
        self.selection
    }
    
    pub fn save_selection_to_memory(&mut self) {
        self.remembered_selection = self.selection;
    }
    
    pub fn get_remembered_selection(&self) -> Option<Rectangle> {
        self.remembered_selection
    }

    // Helper functions for drag mode detection
    const HANDLE_SIZE: f32 = 8.0;
    
    fn get_drag_mode(&self, point: Point) -> DragMode {
        if let Some(selection) = self.selection {
            let handle_size = Self::HANDLE_SIZE;
            
            // Check if click is inside selection for moving
            if selection.contains(point) {
                // Check if near edges for resizing (priority over moving)
                if point.x <= selection.x + handle_size && point.y <= selection.y + handle_size {
                    DragMode::ResizingTopLeft
                } else if point.x >= selection.x + selection.width - handle_size && point.y <= selection.y + handle_size {
                    DragMode::ResizingTopRight  
                } else if point.x >= selection.x + selection.width - handle_size && point.y >= selection.y + selection.height - handle_size {
                    DragMode::ResizingBottomRight
                } else if point.x <= selection.x + handle_size && point.y >= selection.y + selection.height - handle_size {
                    DragMode::ResizingBottomLeft
                } else if point.y <= selection.y + handle_size {
                    DragMode::ResizingTop
                } else if point.x >= selection.x + selection.width - handle_size {
                    DragMode::ResizingRight
                } else if point.y >= selection.y + selection.height - handle_size {
                    DragMode::ResizingBottom
                } else if point.x <= selection.x + handle_size {
                    DragMode::ResizingLeft
                } else {
                    // Inside but not near edges - moving
                    DragMode::Moving
                }
            } else {
                // Outside existing selection - create new one
                DragMode::Creating
            }
        } else {
            // No existing selection - create new one
            DragMode::Creating
        }
    }

    #[cfg(feature = "debug")]
    fn log_debug_event(&mut self, event_name: &str) {
        if self.debug_enabled {
            let now = Instant::now();
            let delay = self.last_event_time
                .map_or(0, |last| now.duration_since(last).as_millis());
            
            // Track when mouse events happen for frame timing
            if event_name.contains("Selection") {
                self.last_mouse_event = Some(now);
                
                // Track when selection changes for render timing
                if event_name.starts_with("StartSelection") || event_name.starts_with("UpdateSelection") {
                    self.selection_changed_time = Some(now);
                }
            }
            
            // Log event with timing
            eprintln!("[SNIPPER DEBUG] {event_name} (+{delay}ms)");
            
            self.event_timestamps.push((event_name.to_string(), now));
            self.last_event_time = Some(now);
            
            // Keep only last 20 events to prevent memory issues
            if self.event_timestamps.len() > 20 {
                self.event_timestamps.remove(0);
            }
        }
    }
    
    #[cfg(not(feature = "debug"))]
    fn log_debug_event(&mut self, _event_name: &str) {
        // Debug logging disabled at compile time
    }
    
    #[cfg(feature = "debug")]
    fn log_performance_warning(&self, operation: &str, duration: Duration) {
        if self.debug_enabled && duration.as_millis() > 50 {
            eprintln!("[SNIPPER PERF WARNING] {} took {}ms (>50ms threshold)", operation, duration.as_millis());
        }
    }
    
    #[cfg(not(feature = "debug"))]
    fn log_performance_warning(&self, _operation: &str, _duration: Duration) {
        // Performance logging disabled at compile time
    }
    
    fn should_update_cache(_new_selection: Option<Rectangle>) -> bool {
        // Always update cache when selection changes
        true
    }
    
    #[cfg(feature = "debug")]
    fn mark_cache_cleared(&mut self, new_selection: Option<Rectangle>) {
        let now = Instant::now();
        self.last_cache_clear = Some(now);
        self.last_significant_selection = new_selection;
        
        if self.debug_enabled {
            eprintln!("[CACHE DEBUG] Cache clear recorded at {now:?}");
        }
    }
    
    #[cfg(not(feature = "debug"))]
    fn mark_cache_cleared(&mut self, _new_selection: Option<Rectangle>) {
        // Cache debug logging disabled at compile time
    }
    
    
    
    #[cfg(feature = "debug")]
    fn reset_timing_after_completion(&mut self) {
        // Reset timing to prevent perpetual stale warnings
        self.selection_changed_time = None;
        self.last_mouse_event = None;
        self.last_cache_clear = None;
        
        if self.debug_enabled {
            eprintln!("[TIMING DEBUG] SnipperState timing reset after selection completion");
        }
    }
    
    #[cfg(not(feature = "debug"))]
    fn reset_timing_after_completion(&mut self) {
        // Timing debug disabled at compile time
    }
    
    #[allow(clippy::too_many_lines)]
    pub fn update(&mut self, message: SnipperMessage) -> Option<SnipperResult> {
        let update_start = Instant::now();
        
        let result = match message {
            SnipperMessage::StartSelection(point) => {
                self.log_debug_event(&format!("StartSelection at ({:.1}, {:.1})", point.x, point.y));
                
                // Detect double-click first
                let now = std::time::Instant::now();
                let is_double_click = if let (Some(last_time), Some(last_pos)) = (self.last_click_time, self.last_click_pos) {
                    now.duration_since(last_time).as_millis() < 500 && // Within 500ms
                    (point.x - last_pos.x).abs() < 5.0 && (point.y - last_pos.y).abs() < 5.0 // Within 5px
                } else {
                    false
                };
                
                self.last_click_time = Some(now);
                self.last_click_pos = Some(point);
                
                if is_double_click && self.selection.is_some() {
                    // Double-click detected - accept selection if inside
                    if let Some(selection) = self.selection {
                        if selection.contains(point) {
                            self.save_selection_to_memory();
                            self.reset_timing_after_completion();
                            return Some(SnipperResult::Selected(selection));
                        }
                    }
                }
                
                // Determine drag mode and start dragging
                self.drag_mode = self.get_drag_mode(point);
                self.drag_start = point;
                self.current_mouse = point;
                self.initial_selection = self.selection;
                
                if let DragMode::Creating = self.drag_mode {
                    self.selection = Some(Rectangle::new(point, Size::ZERO));
                } else {
                    // Moving or resizing - keep existing selection for now
                }
                None
            }
            SnipperMessage::UpdateSelection(point) => {
                self.log_debug_event(&format!("UpdateSelection to ({:.1}, {:.1})", point.x, point.y));
                
                self.current_mouse = point;
                
                if !matches!(self.drag_mode, DragMode::None) {
                    match self.drag_mode {
                        DragMode::Creating => {
                            let x = self.drag_start.x.min(point.x);
                            let y = self.drag_start.y.min(point.y);
                            let width = (self.drag_start.x - point.x).abs();
                            let height = (self.drag_start.y - point.y).abs();
                            self.selection = Some(Rectangle::new(Point::new(x, y), Size::new(width, height)));
                        }
                        DragMode::Moving => {
                            if let Some(initial) = self.initial_selection {
                                let delta_x = point.x - self.drag_start.x;
                                let delta_y = point.y - self.drag_start.y;
                                self.selection = Some(Rectangle::new(
                                    Point::new(initial.x + delta_x, initial.y + delta_y),
                                    initial.size()
                                ));
                            }
                        }
                        DragMode::ResizingTopLeft => {
                            if let Some(initial) = self.initial_selection {
                                let new_x = point.x;
                                let new_y = point.y;
                                let new_width = (initial.x + initial.width - new_x).max(10.0);
                                let new_height = (initial.y + initial.height - new_y).max(10.0);
                                self.selection = Some(Rectangle::new(
                                    Point::new(new_x, new_y),
                                    Size::new(new_width, new_height)
                                ));
                            }
                        }
                        // Add other resize modes as needed
                        _ => {}
                    }
                }
                None
            }
            SnipperMessage::EndSelection => {
                self.log_debug_event("EndSelection");
                // Stop dragging
                self.drag_mode = DragMode::None;
                None
            }
            SnipperMessage::AcceptSelection => {
                self.log_debug_event("AcceptSelection");
                // Accept current selection (double-click or Enter)
                if let Some(selection) = self.selection {
                    if selection.width > 10.0 && selection.height > 10.0 {
                        self.save_selection_to_memory();
                        self.reset_timing_after_completion();
                        return Some(SnipperResult::Selected(selection));
                    }
                }
                None
            }
            SnipperMessage::DoubleClick(point) => {
                // Accept selection if double-click is inside the selection
                if let Some(selection) = self.selection {
                    if selection.contains(point) {
                        self.save_selection_to_memory();
                        self.reset_timing_after_completion();
                        return Some(SnipperResult::Selected(selection));
                    }
                }
                None
            }
            SnipperMessage::CancelSelection => {
                self.log_debug_event("CancelSelection");
                self.reset_timing_after_completion();
                Some(SnipperResult::Cancelled)
            }
            SnipperMessage::KeyPressed(key) => {
                self.log_debug_event(&format!("KeyPressed: {key:?}"));
                match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    self.reset_timing_after_completion();
                    Some(SnipperResult::Cancelled)
                }
                keyboard::Key::Named(keyboard::key::Named::Enter) => {
                    // Use AcceptSelection for Enter key
                    return self.update(SnipperMessage::AcceptSelection);
                }
                _ => None,
                }
            }
        };
        
        // Log performance warning if update took too long
        let update_duration = update_start.elapsed();
        self.log_performance_warning("SnipperState::update", update_duration);
        
        result
    }
}

#[derive(Debug, Clone)]
pub enum SnipperResult {
    Selected(Rectangle),
    Cancelled,
}

pub struct Snipper {
    state: SnipperState,
    canvas_program: SelectionOnlyCanvas,
}

impl Snipper {
    #[must_use] 
    pub fn new(screen_images: HashMap<String, Vec<u8>>, screen_bounds: Rectangle) -> Self {
        let creation_start = Instant::now();
        let state = SnipperState::new(screen_images, screen_bounds);
        
        if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
            let creation_time = creation_start.elapsed();
            eprintln!("[SNIPPER DEBUG] Snipper::new took {}ms", creation_time.as_millis());
            if creation_time.as_millis() > 100 {
                eprintln!("[SNIPPER PERF WARNING] Snipper creation took {}ms (>100ms threshold)", creation_time.as_millis());
            }
        }
        
        Self {
            state,
            canvas_program: SelectionOnlyCanvas::new(None),
        }
    }
    
    #[must_use] 
    pub fn new_with_memory(screen_images: HashMap<String, Vec<u8>>, screen_bounds: Rectangle, remembered_selection: Option<Rectangle>) -> Self {
        let state = SnipperState::new_with_memory(screen_images, screen_bounds, remembered_selection);
        let initial_selection = state.selection();
        Self {
            state,
            canvas_program: SelectionOnlyCanvas::new(initial_selection),
        }
    }
    
    pub fn get_remembered_selection(&self) -> Option<Rectangle> {
        self.state.get_remembered_selection()
    }

    pub fn view(&self) -> cosmic::Element<'_, Message> {
        // STATIC background image - should never trigger redraws
        let background_image = if let Some(ref image_handle) = self.state.cached_image_handle {
            cosmic::widget::container(
                cosmic::widget::image(image_handle.clone())
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fill)
                    .content_fit(cosmic::iced::ContentFit::Fill), // Ensure full coverage
            )
            .width(cosmic::iced::Length::Fill)
            .height(cosmic::iced::Length::Fill)
        } else {
            cosmic::widget::container(cosmic::widget::text("Failed to load screenshot"))
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .style(|_theme| cosmic::iced::widget::container::Style {
                    background: Some(cosmic::iced::Color::from_rgba(0.2, 0.2, 0.2, 1.0).into()),
                    ..Default::default()
                })
        };

        // Use cached canvas program for efficient drawing - cache will only redraw when selection changes
        let overlay_element: cosmic::Element<Message> =
            cosmic::widget::canvas(&self.canvas_program)
                .width(cosmic::iced::Length::Fill)
                .height(cosmic::iced::Length::Fill)
                .into();

        // Stack: static image + cached dynamic overlay
        cosmic::widget::container(cosmic::iced::widget::stack![
            background_image, // Layer 1: Static, never redraws
            overlay_element   // Layer 2: Cached canvas, redraws only when selection changes
        ])
        .width(cosmic::iced::Length::Fill)
        .height(cosmic::iced::Length::Fill)
        .into()
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, message: SnipperMessage) -> Option<SnipperResult> {
        #[cfg(feature = "debug")]
        let update_start = Instant::now();
        let old_selection = self.state.selection;
        
        let result = self.state.update(message.clone());

        // Always update canvas program selection, but only clear cache when significant
        if self.state.selection != old_selection {
            self.canvas_program.selection = self.state.selection;
            
            // Only clear cache for significant changes
            if SnipperState::should_update_cache(self.state.selection) {
                #[cfg(feature = "debug")]
                let cache_start = Instant::now();
                self.canvas_program.clear_cache();
                
                // Record that we cleared the cache
                self.state.mark_cache_cleared(self.state.selection);
                
                #[cfg(feature = "debug")]
                if self.state.debug_enabled {
                    let cache_time = cache_start.elapsed();
                    if cache_time.as_millis() > 10 {
                        eprintln!("[SNIPPER PERF] Cache clear took {}ms", cache_time.as_millis());
                    }
                    eprintln!("[SNIPPER DEBUG] Selection updated, cache cleared - next frame should show visual change");
                }
            }
        }
        
        // Reset canvas timing when selection completes to stop perpetual pipeline warnings  
        if let Some(SnipperResult::Selected(_)) = result {
            self.canvas_program.reset_timing();
        } else if let Some(SnipperResult::Cancelled) = result {
            self.canvas_program.reset_timing();
        }
        
        #[cfg(feature = "debug")]
        if self.state.debug_enabled {
            let total_time = update_start.elapsed();
            if total_time.as_millis() > 20 {
                eprintln!("[SNIPPER PERF WARNING] Total Snipper::update took {}ms (>20ms)", total_time.as_millis());
            }
        }

        result
    }

    pub fn get_selection(&self) -> Option<Rectangle> {
        self.state.selection()
    }

    pub fn update_screenshot(
        &mut self,
        screen_images: HashMap<String, Vec<u8>>,
        screen_bounds: Rectangle,
    ) {
        self.update_screenshot_with_memory(screen_images, screen_bounds, None);
    }
    
    pub fn update_screenshot_with_memory(
        &mut self,
        screen_images: HashMap<String, Vec<u8>>,
        screen_bounds: Rectangle,
        remembered_selection: Option<Rectangle>,
    ) {
        // Update the cached image handle with new screenshot data
        self.state.cached_image_handle = if let Some(screenshot_data) = screen_images.get("primary")
        {
            if let Ok(img) = image::load_from_memory(screenshot_data) {
                let rgba_img = img.to_rgba8();
                let (width, height) = rgba_img.dimensions();
                Some(cosmic::iced::widget::image::Handle::from_rgba(
                    width,
                    height,
                    rgba_img.into_raw(),
                ))
            } else {
                None
            }
        } else {
            None
        };

        // Update screen data
        self.state.screen_images = screen_images;
        self.state.screen_bounds = screen_bounds;
        self.state.remembered_selection = remembered_selection;

        // Reset selection for new screenshot, but restore from memory if available
        if let Some(remembered) = remembered_selection {
            // Check if remembered selection fits in new screen bounds
            if screen_bounds.contains(Point::new(remembered.x, remembered.y)) && 
               screen_bounds.contains(Point::new(remembered.x + remembered.width, remembered.y + remembered.height)) {
                self.state.selection = Some(remembered);
                self.canvas_program.selection = Some(remembered);
            } else {
                self.state.selection = None;
                self.canvas_program.selection = None;
            }
        } else {
            self.state.selection = None;
            self.canvas_program.selection = None;
        }
        
        // Reset interaction state
        self.state.drag_mode = DragMode::None;
        self.state.initial_selection = None;
        self.state.last_click_time = None;
        self.state.last_click_pos = None;
        self.canvas_program.clear_cache();
    }

    pub fn subscription() -> cosmic::iced::Subscription<SnipperMessage> {
        // Handle keyboard events
        event::listen_with(|event, _status, _window_id| {
            match event {
                event::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                    Some(SnipperMessage::KeyPressed(key))
                }
                // Detect double-clicks for selection acceptance
                event::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    // TODO: Implement proper double-click detection with timing
                    // For now, we'll use a simple approach
                    None
                }
                _ => None,
            }
        })
    }
}

// Removed redundant Snipper canvas implementation - SelectionOnlyCanvas handles all dynamic drawing

// Canvas that only draws selection overlay - NO image drawing here
#[derive(Debug)]
pub struct SelectionOnlyCanvas {
    selection: Option<Rectangle>,
    // Canvas cache for efficient drawing
    cache: canvas::Cache,
    // Track when last selection change occurred for render timing
    last_selection_time: Option<Instant>,
}

impl SelectionOnlyCanvas {
    #[must_use] 
    pub fn new(selection: Option<Rectangle>) -> Self {
        Self {
            selection,
            cache: canvas::Cache::default(),
            last_selection_time: None,
        }
    }

    // Clear cache when selection changes
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.last_selection_time = Some(Instant::now());
        
        if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
            eprintln!("[CANVAS DEBUG] Cache cleared - selection changed, next draw will show new selection");
        }
    }
    
    // Reset timing after selection is complete to stop perpetual warnings
    pub fn reset_timing(&mut self) {
        self.last_selection_time = None;
        
        if std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok() {
            eprintln!("[CANVAS DEBUG] Canvas timing reset - no more pipeline warnings");
        }
    }
}

impl canvas::Program<Message, cosmic::Theme, cosmic::Renderer> for SelectionOnlyCanvas {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (cosmic::iced::event::Status, Option<Message>) {
        #[cfg(feature = "debug")]
        let debug_enabled = std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok();
        #[cfg(feature = "debug")]
        let event_start = if debug_enabled { Some(Instant::now()) } else { None };
        
        let result = match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                #[cfg(feature = "debug")]
                if debug_enabled {
                    eprintln!("[CANVAS DEBUG] ButtonPressed at cursor: {:?}", cursor.position());
                }
                if let Some(position) = cursor.position_in(bounds) {
                    (
                        cosmic::iced::event::Status::Captured,
                        Some(Message::SnipperMessage(SnipperMessage::StartSelection(
                            position,
                        ))),
                    )
                } else {
                    (cosmic::iced::event::Status::Ignored, None)
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                #[cfg(feature = "debug")]
                if debug_enabled {
                    eprintln!("[CANVAS DEBUG] ButtonReleased");
                }
                (
                    cosmic::iced::event::Status::Captured,
                    Some(Message::SnipperMessage(SnipperMessage::EndSelection)),
                )
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                #[cfg(feature = "debug")]
                if debug_enabled {
                    // Only log every 10th mouse move to avoid spam
                    static MOVE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                    if MOVE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed).is_multiple_of(10) {
                        eprintln!("[CANVAS DEBUG] CursorMoved to ({:.1}, {:.1})", position.x, position.y);
                    }
                }
                // Always capture mouse movements for responsive dragging
                (
                    cosmic::iced::event::Status::Captured,
                    Some(Message::SnipperMessage(SnipperMessage::UpdateSelection(
                        position,
                    ))),
                )
            }
            // Note: iced doesn't have built-in double-click detection in canvas events
            // We'll need to implement this at the subscription level
            _ => (cosmic::iced::event::Status::Ignored, None)
        };
        
        #[cfg(feature = "debug")]
        if let Some(event_start_time) = event_start {
            let event_duration = event_start_time.elapsed();
            if event_duration.as_micros() > 1000 { // 1ms threshold for event handling
                eprintln!("[CANVAS PERF WARNING] Event handling took {}μs (>1000μs threshold)", event_duration.as_micros());
            }
        }

        result
    }

    #[allow(clippy::too_many_lines)]
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        _theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        #[cfg(feature = "debug")]
        let debug_enabled = std::env::var("COSMIC_SCREENSHOT_DEBUG").is_ok();
        #[cfg(feature = "debug")]
        let now = Instant::now();
        #[cfg(feature = "debug")]
        let draw_start = if debug_enabled { Some(now) } else { None };
        
        
        #[cfg(feature = "debug")]
        if debug_enabled {
            eprintln!("[CANVAS DEBUG] Drawing frame");
        }
        
        // Use cache for efficient drawing - only redraws when cache is cleared
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            #[cfg(feature = "debug")]
            let frame_start = if debug_enabled { Some(Instant::now()) } else { None };
            if let Some(selection) = self.selection {
                let overlay_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

                // Efficient dark overlay rectangles (only draw what's needed)
                let rects = [
                    // Top
                    (selection.y > 0.0)
                        .then_some((Point::ORIGIN, Size::new(bounds.width, selection.y))),
                    // Bottom
                    (selection.y + selection.height < bounds.height).then_some((
                        Point::new(0.0, selection.y + selection.height),
                        Size::new(bounds.width, bounds.height - selection.y - selection.height),
                    )),
                    // Left
                    (selection.x > 0.0).then_some((
                        Point::new(0.0, selection.y),
                        Size::new(selection.x, selection.height),
                    )),
                    // Right
                    (selection.x + selection.width < bounds.width).then_some((
                        Point::new(selection.x + selection.width, selection.y),
                        Size::new(
                            bounds.width - selection.x - selection.width,
                            selection.height,
                        ),
                    )),
                ];

                // Draw overlay rectangles
                for rect in rects.iter().flatten() {
                    frame.fill_rectangle(rect.0, rect.1, overlay_color);
                }

                // Selection border (bright red)
                let border_color = Color::from_rgb(1.0, 0.0, 0.0);
                let border_width = 3.0; // Slightly thinner for performance

                let border_rects = [
                    (
                        Point::new(selection.x, selection.y),
                        Size::new(selection.width, border_width),
                    ),
                    (
                        Point::new(selection.x + selection.width - border_width, selection.y),
                        Size::new(border_width, selection.height),
                    ),
                    (
                        Point::new(selection.x, selection.y + selection.height - border_width),
                        Size::new(selection.width, border_width),
                    ),
                    (
                        Point::new(selection.x, selection.y),
                        Size::new(border_width, selection.height),
                    ),
                ];

                for (pos, size) in border_rects {
                    frame.fill_rectangle(pos, size, border_color);
                }

                // Corner handles (reduced to 4 for performance)
                let handle_size = 8.0; // Smaller for performance
                let handle_color = Color::from_rgb(1.0, 1.0, 1.0);
                let handles = [
                    Point::new(
                        selection.x - handle_size / 2.0,
                        selection.y - handle_size / 2.0,
                    ),
                    Point::new(
                        selection.x + selection.width - handle_size / 2.0,
                        selection.y - handle_size / 2.0,
                    ),
                    Point::new(
                        selection.x + selection.width - handle_size / 2.0,
                        selection.y + selection.height - handle_size / 2.0,
                    ),
                    Point::new(
                        selection.x - handle_size / 2.0,
                        selection.y + selection.height - handle_size / 2.0,
                    ),
                ];

                let handle_size_vec = Size::new(handle_size, handle_size);
                for handle_pos in handles {
                    frame.fill_rectangle(handle_pos, handle_size_vec, handle_color);
                    frame.stroke_rectangle(
                        handle_pos,
                        handle_size_vec,
                        canvas::Stroke::default()
                            .with_width(1.0)
                            .with_color(border_color),
                    );
                }
            } else {
                // No selection - single full overlay
                frame.fill_rectangle(
                    Point::ORIGIN,
                    bounds.size(),
                    Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                );
            }
            
            // Log frame rendering time if debugging enabled
            #[cfg(feature = "debug")]
            if let Some(frame_start_time) = frame_start {
                let frame_duration = frame_start_time.elapsed();
                if frame_duration.as_millis() > 16 { // 60fps = 16ms budget
                    eprintln!("[CANVAS PERF WARNING] Frame rendering took {}ms (>16ms for 60fps)", frame_duration.as_millis());
                } else if debug_enabled {
                    eprintln!("[CANVAS DEBUG] Frame content rendered in {}ms", frame_duration.as_millis());
                }
            }
        });
        
        // Log total draw time and event-to-render pipeline timing if debugging enabled
        #[cfg(feature = "debug")]
        if let Some(draw_start_time) = draw_start {
            let total_duration = draw_start_time.elapsed();
            if total_duration.as_millis() > 20 {
                eprintln!("[CANVAS PERF WARNING] Total draw() took {}ms (>20ms threshold)", total_duration.as_millis());
            }
            
            // Show event-to-render pipeline timing
            if let Some(selection_time) = self.last_selection_time {
                let event_to_render = draw_start_time.duration_since(selection_time).as_millis();
                if event_to_render > 50 {
                    eprintln!("[PIPELINE PERF WARNING] Selection change to render took {event_to_render}ms (>50ms)");
                } else if debug_enabled {
                    eprintln!("[PIPELINE DEBUG] Selection change to render: {event_to_render}ms");
                }
            }
        }

        vec![geometry]
    }
}

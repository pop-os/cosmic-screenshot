# cosmic-screenshot

A screenshot tool for Linux desktop environments with multi-backend support, GUI, CLI, D-Bus service, and library API.

## Features

### Core Functionality
- **Multi-backend screenshot system** with automatic fallback
- **Interactive region selection** with fullscreen overlay and snipper
- **All screenshot types**: Full desktop, current screen, window selection, interactive screen selection, and rectangular regions
- **COSMIC UI** with persistent settings and 360p thumbnails
- **Command-line interface** with comprehensive options
- **D-Bus service** for system integration
- **Library API** for use in other applications

### Screenshot Backends
- **KWin ScreenShot2**: Preferred backend with full feature support
- **Freedesktop Portal**: Fallback backend with workspace/interactive support
- **Automatic backend selection** with intelligent capability-based routing

### User Interface
- **COSMIC-native GUI** following official design standards
- **Persistent settings** using cosmic-config
- **Screenshot-on-startup** feature
- **Memory of selection areas** across sessions (optional)
- **Professional interaction model** with resize handles and keyboard shortcuts
- **Native file dialogs** with directory persistence

### Integration
- **Complete D-Bus API** (`com.system76.CosmicScreenshot`)
- **Desktop file** with multiple screenshot actions
- **System service** support with automatic activation
- **Multi-window architecture** with performance optimization
- **Cross-platform compatibility** (Linux desktop environments)

## Installation

### From Source

#### Prerequisites
- Rust (latest stable)
- `just` command runner
- System libraries: libxkbcommon, wayland, vulkan, mesa, fontconfig, freetype, X11

```bash
git clone <repository-url>
cd cosmic-screenshot
just build-release
```

#### Debian/Ubuntu
```bash
sudo apt install build-essential just pkg-config \
    libxkbcommon-dev libwayland-dev libvulkan-dev libgl-dev \
    mesa-common-dev libinput-dev libfontconfig-dev libfreetype6-dev \
    libx11-dev libxcursor-dev libxi-dev libxrandr-dev
```

#### Arch Linux
```bash
sudo pacman -S just pkg-config libxkbcommon wayland vulkan-loader \
    mesa libinput fontconfig freetype2 libx11 libxcursor libxi libxrandr
```

### Nix/NixOS
```bash
# Build with Nix
nix-build
```

## Usage

### GUI Application
```bash
# Launch the GUI
cosmic-screenshot gui

# Or simply
cosmic-screenshot
```

### Command Line Interface

#### Basic Screenshots
```bash
# Full desktop screenshot
cosmic-screenshot take --kind all

# Current screen only  
cosmic-screenshot take --kind screen

# Window selection
cosmic-screenshot take --kind window

# Interactive screen selection
cosmic-screenshot take --kind select

# Interactive region selection (launches GUI)
cosmic-screenshot take --kind region
```

#### Advanced Options
```bash
# Screenshot with 3 second delay
cosmic-screenshot take --kind all --delay 3000

# Save to clipboard
cosmic-screenshot take --kind all --clipboard

# Save to specific directory
cosmic-screenshot take --kind all --output-dir ~/Pictures/Screenshots

# Use specific backend
cosmic-screenshot take --kind all --backend kwin

# Combined options
cosmic-screenshot take --kind region --delay 2000 --clipboard --output-dir ~/Desktop
```

### Backend Management
```bash
# List available backends
cosmic-screenshot backends

# Test D-Bus functionality
cosmic-screenshot test-dbus

# Generate D-Bus XML interface definition
cosmic-screenshot generate-dbus-xml
```

### D-Bus Service
```bash
# Start the D-Bus service
cosmic-screenshot service

# Use from other applications via D-Bus
dbus-send --session --print-reply \
  --dest=com.system76.CosmicScreenshot \
  /com/system76/CosmicScreenshot \
  com.system76.CosmicScreenshot.take_screenshot \
  string:"all" uint32:0 boolean:false string:""
```

## D-Bus API Reference

### Service Information
- **Service**: `com.system76.CosmicScreenshot`
- **Object Path**: `/com/system76/CosmicScreenshot`
- **Interface**: `com.system76.CosmicScreenshot`

### Available Methods

#### `take_screenshot(kind, delay_ms, save_to_clipboard, save_dir) → result`
Take a screenshot with basic options.

#### `take_screenshot_with_backend(kind, delay_ms, save_to_clipboard, save_dir, backend) → result`
Take a screenshot with specific backend selection.

#### `get_available_backends() → backends`
List all available screenshot backends.

#### `supports_kind(kind) → supported`
Check if a screenshot type is supported.

#### `supports_kind_with_backend(kind, backend) → supported`
Check if a specific backend supports a screenshot type.

#### `get_backend_capabilities() → capabilities`
Get detailed capability matrix for all backends.

### Screenshot Types
- `"all"` - All screens
- `"screen"` - Current screen  
- `"window"` - Window under cursor
- `"select"` - Interactive screen selection
- `"region"` - Interactive region selection (use CLI, not supported via D-Bus)

### Backend Types
- `"auto"` - Automatic selection (default)
- `"kwin"` - KWin ScreenShot2 backend
- `"freedesktop"` - Freedesktop Portal backend

## Library Usage

Add to your `Cargo.toml`:
```toml
[dependencies]
cosmic-screenshot = { path = "path/to/cosmic-screenshot" }
```

Basic usage:
```rust
use cosmic_screenshot::screenshot::{ScreenshotManager, ScreenshotKind, ScreenshotOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ScreenshotManager::new();
    
    let options = ScreenshotOptions {
        kind: ScreenshotKind::AllScreens,
        delay_ms: 0,
        save_to_clipboard: false,
        save_dir: Some(PathBuf::from("~/Pictures")),
    };
    
    let result = manager.take_screenshot(&options).await?;
    println!("Screenshot saved to: {:?}", result.path);
    
    Ok(())
}
```

## Architecture

### Multi-Backend System
cosmic-screenshot uses a backend system that automatically detects available screenshot methods and routes requests to the most appropriate backend:

1. **KWin ScreenShot2**: Primary backend for KDE/Plasma environments
2. **Freedesktop Portal**: Universal fallback for all desktop environments
3. **Automatic Selection**: Intelligent routing based on availability and capabilities

### Performance Features
- Image handle caching
- Window reuse for GUI performance
- Canvas optimization
- Single tokio runtime architecture

### Settings
User preferences saved with cosmic-config:
- Screenshot type and backend selection
- Delay and clipboard settings
- Directory and selection area memory
- Screenshot-on-startup option

## Development

### Building
```bash
# Debug build
just build-debug

# Release build with debug features
cargo build --features debug --release

# Release build
just build-release

# Run with backtrace
just run

# Linter
just check
```

### Testing
```bash
# Test all backends
cosmic-screenshot test-dbus

# Test specific functionality
cargo test
```

### Contributing
1. Follow the existing code style and architecture
2. Test across different desktop environments
3. Update documentation as needed

## Troubleshooting

### KWin Backend Issues
If KWin screenshots fail:
```bash
# Check KWin D-Bus availability
qdbus org.kde.KWin /org/kde/KWin/ScreenShot2

# Force portal fallback
cosmic-screenshot take --kind all --backend freedesktop
```

### Permissions
Required access:
- D-Bus session bus
- Screen capture (via portal/KWin authorization)
- Output directory write permissions

### Debug Information
Enable debug logging:
```bash
COSMIC_SCREENSHOT_DEBUG=1 cosmic-screenshot [command]
```

## License

GPL-3.0-only - see [LICENSE](LICENSE) file for details.

## Dependencies

Built with [libcosmic](https://github.com/pop-os/libcosmic) UI toolkit.

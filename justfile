name := 'cosmic-screenshot'
export APPID := 'com.system76.CosmicScreenshot'

rootdir := ''
prefix := '/usr'

base-dir := absolute_path(clean(rootdir / prefix))

export INSTALL_DIR := base-dir / 'share'

cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name
bin-dst := base-dir / 'bin' / name

desktop := APPID + '.desktop'
desktop-src := 'resources' / desktop
desktop-dest := clean(rootdir / prefix) / 'share' / 'applications' / desktop

icons-src := 'resources' / 'icons' / 'hicolor'
icons-dst := clean(rootdir / prefix) / 'share' / 'icons' / 'hicolor'

dbus-service := APPID + '.service'
dbus-service-src := 'resources' / dbus-service
dbus-service-dst := clean(rootdir / prefix) / 'share' / 'dbus-1' / 'services' / dbus-service

dbus-interface := APPID + '.xml'
dbus-interface-src := 'resources' / dbus-interface
dbus-interface-dst := clean(rootdir / prefix) / 'share' / 'dbus-1' / 'interfaces' / dbus-interface

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# `cargo clean` and removes vendored dependencies
clean-dist: clean
    rm -rf .cargo vendor vendor.tar

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Run with debug logs
run *args:
    env RUST_LOG=debug RUST_BACKTRACE=full cargo run --release {{args}}

# Installs files
install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 {{desktop-src}} {{desktop-dest}}
    install -Dm0644 {{dbus-service-src}} {{dbus-service-dst}}
    install -Dm0644 {{dbus-interface-src}} {{dbus-interface-dst}}
    # Install library files for development package
    install -Dm0644 {{cargo-target-dir}}/release/libcosmic_screenshot.rlib {{base-dir}}/lib/libcosmic_screenshot.rlib
    install -Dm0755 {{cargo-target-dir}}/release/libcosmic_screenshot.so {{base-dir}}/lib/libcosmic_screenshot.so
    for size in `ls {{icons-src}}`; do \
        install -Dm0644 "{{icons-src}}/$size/apps/{{APPID}}.svg" "{{icons-dst}}/$size/apps/{{APPID}}.svg"; \
    done

# Uninstalls installed files
uninstall:
    rm {{bin-dst}}

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml \
        | head -n -1 > .cargo/config
    echo 'directory = "vendor"' >> .cargo/config
    tar pcf vendor.tar vendor
    rm -rf vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

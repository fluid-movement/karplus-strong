# Karplus-Strong. `just verify` is the gate.
#
#   verify   the gate and the checks inside it
#   listen   rendering audio
#   build    the Rack plugin, the header

target := "thumbv7em-none-eabihf"
tooling := justfile_directory() / ".tooling"

# The VCV Rack SDK. Override with `just rack_dir=/path/to/Rack-SDK vcv`.
rack_dir := tooling / "Rack-SDK"
rack_version := "2.6.4"

[private]
default:
    @just --list

# The gate. Everything that must be true before work counts as done.
[group('verify')]
verify: (fmt "check") lint test nostd rules shim scenarios listening-check
    @echo
    @echo "verify: green"

# Formatting: `just fmt` rewrites, `just fmt check` only reports.
[group('verify')]
fmt mode="write":
    cargo fmt --all {{ if mode == "check" { "-- --check" } else { "" } }}

# Clippy over every target, warnings are errors.
[group('verify')]
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Unit and property tests.
[group('verify')]
test:
    cargo test --workspace

# The core must build for the Daisy's M7 as well as the host. A `std`
# dependency anywhere in it breaks here.
[group('verify')]
nostd:
    cargo build -p karplus-core --release --target {{target}}

# The core's hard rules, checked rather than trusted: no f64, no unsafe, and
# no dependency but fathom-core.
[group('verify')]
rules:
    #!/usr/bin/env bash
    set -euo pipefail
    core=crates/karplus-core
    # Comments may talk about f64; code may not use it.
    if grep -rn --include='*.rs' -E '\bf64\b' $core/src | grep -vE '^[^:]+:[0-9]+:\s*//'; then
        echo "f64 in karplus-core"; exit 1
    fi
    grep -q '^#!\[forbid(unsafe_code)\]' $core/src/lib.rs || { echo "forbid(unsafe_code) missing"; exit 1; }
    grep -q '^#!\[no_std\]' $core/src/lib.rs || { echo "no_std missing"; exit 1; }
    deps=$(cargo tree -p karplus-core -e normal --depth 1 --prefix none | tail -n +2 | awk '{print $1}' | sort -u | tr '\n' ' ')
    [ "$deps" = "fathom-core " ] || { echo "karplus-core depends on: $deps"; exit 1; }
    echo "rules: ok"

# The C ABI, and the only part of the shim that contains arithmetic.
[group('verify')]
shim:
    cargo build --quiet --release -p karplus-ffi
    mkdir -p target
    c++ -std=c++11 -Wall -Wextra -Werror -o target/vcv-controls-test targets/vcv/test/controls_test.cpp
    ./target/vcv-controls-test

# Render every scenario and check its assertions.
[group('verify')]
scenarios:
    cargo run --quiet --release -p karplus-harness -- run

# Parse the listening set without rendering, so a rename cannot rot it.
[group('verify')]
listening-check:
    cargo run --quiet --release -p karplus-harness -- check --scenarios listening

# Every scenario and listening patch, as WAVs in listening/out/.
[group('listen')]
render:
    mkdir -p listening/out
    cargo run --quiet --release -p karplus-harness -- run --scenarios listening --wav listening/out
    cargo run --quiet --release -p karplus-harness -- run --wav listening/out
    @echo "wrote listening/out"

# Build the VCV Rack plugin and install it into Rack's plugin directory.
[group('build')]
vcv: sdk
    cd targets/vcv && make install RACK_DIR="{{rack_dir}}"
    @echo "installed — restart Rack to load it"

# Regenerate the committed C header from the Rust source.
[group('build')]
header:
    cargo run --quiet -p karplus-ffi --example gen_header

# Regenerate the panel graphic from res/panel.py.
[group('build')]
panel:
    python3 targets/vcv/res/panel.py

# Remove build output and rendered audio.
[group('build')]
clean:
    cargo clean
    cd targets/vcv && rm -rf build plugin.so plugin.dylib plugin.dll dist
    rm -rf listening/out

# Fetch the VCV Rack SDK into .tooling/, so `just vcv` needs no arguments.
[group('build')]
sdk:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -f "{{rack_dir}}/include/rack.hpp" ]; then
        echo "==> Rack SDK already at {{rack_dir}}"; exit 0
    fi
    case "$(uname -s)" in
        Darwin) os=mac ;; Linux) os=lin ;; MINGW*|MSYS*|CYGWIN*) os=win ;;
        *) echo "unknown OS; fetch the SDK from https://vcvrack.com/downloads" >&2; exit 1 ;;
    esac
    case "$(uname -m)" in arm64|aarch64) arch=arm64 ;; *) arch=x64 ;; esac
    url="https://vcvrack.com/downloads/Rack-SDK-{{rack_version}}-${os}-${arch}.zip"
    mkdir -p "{{tooling}}"
    curl -fL --progress-bar -o "{{tooling}}/rack-sdk.zip" "$url"
    rm -rf "{{rack_dir}}"
    unzip -q "{{tooling}}/rack-sdk.zip" -d "{{tooling}}"
    rm -f "{{tooling}}/rack-sdk.zip"
    test -f "{{rack_dir}}/include/rack.hpp"

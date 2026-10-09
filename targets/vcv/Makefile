# Karplus-Strong for VCV Rack.
#
# The C++ here is a shim. All of the sound is in the Rust staticlib this
# Makefile builds first and links against (docs/architecture.md).
#
#   RACK_DIR=/path/to/Rack-SDK make
#
RACK_DIR ?= ../../../Rack-SDK

# Where cargo puts the staticlib, and where the generated header lives.
WORKSPACE := ../..
FFI_INCLUDE := $(WORKSPACE)/crates/karplus-ffi/include
FFI_LIB_DIR := $(WORKSPACE)/target/release
FFI_LIB := $(FFI_LIB_DIR)/libkarplus_ffi.a

FLAGS += -I$(FFI_INCLUDE)
SOURCES += $(wildcard src/*.cpp)
LDFLAGS += $(FFI_LIB)

DISTRIBUTABLES += res plugin.json

include $(RACK_DIR)/plugin.mk

# Build the core before anything that links against it. Release profile: the
# instruction-count budget is measured there, and a debug build of the DSP is
# not worth listening to.
#
# Unconditional, because make cannot know what the staticlib depends on and a
# stale one links without complaint: Rack's plugin.mk resolves our symbols at
# dlopen, so a missing one is not a build error, it is a module that has
# quietly disappeared from Rack. Cargo decides whether there is work to do.
.PHONY: $(FFI_LIB)
$(FFI_LIB):
	cd $(WORKSPACE) && cargo build --release -p karplus-ffi

$(OBJECTS): $(FFI_LIB)

# Regenerate the committed header. `just verify` fails if it has drifted.
header:
	cd $(WORKSPACE) && cargo run -p karplus-ffi --example gen_header

.PHONY: header

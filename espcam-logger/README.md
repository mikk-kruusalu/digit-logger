# ESPCAM Logger

The camera code is copied from https://github.com/Kezii/esp32cam_rs, since I could not make it compile as a library for my project. The problem is that the main esp-idf-sys needs to link to the esp32-camera C library. Which is done by adding the following to Cargo.toml

```toml
[[package.metadata.esp-idf-sys.extra_components]]
component_dirs = "components/esp32-camera"
bindings_header = "components/bindings.h"
bindings_module = "camera"
```

Installation should be straight forward. Just follow the instructions on the official esp32 Rust documentation and use `cargo run`.

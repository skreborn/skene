# ui

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.125
cargo build --release --target wasm32-unknown-unknown -p counter
wasm-bindgen target/wasm32-unknown-unknown/release/counter.wasm \
    --out-dir examples/counter/dist --target web
python3 -m http.server --directory examples/counter 8080
```

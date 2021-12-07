<div align="center">

  <h1><code>gun-rs-wasm</code></h1>

  <strong>Rust & WASM port of <a href="https://github.com/amark/gun">Gun</a>.</strong>
  <p>For a non-wasm version, check out <a href="https://github.com/mmalmi/gun-rs">gun-rs</a>
  
  <a href="https://rusty-gun-demo.netlify.app/">Example</a> (<a href="https://github.com/mmalmi/rusty-gun-demo/">source</a>)

</div>

## Use

`npm install rusty-gun`

```
import { Node as Gun } from "rusty-gun"
const gun = new Gun('ws://localhost:8765/gun')
gun.get("profile").get("name").on((v,k) => console.log(k,v))
gun.get("profile").get("name").put("Satoshi")
```

You'll need to [load](https://developer.mozilla.org/en-US/docs/WebAssembly/Loading_and_running#using_fetch) rusty_gun_bg.wasm into the document first. It needs to be served using the mime type `application/wasm`.

## About

[**📚 Read this template tutorial! 📚**][template-docs]

This template is designed for compiling Rust libraries into WebAssembly and
publishing the resulting package to NPM.

Be sure to check out [other `wasm-pack` tutorials online][tutorials] for other
templates and usages of `wasm-pack`.

[tutorials]: https://rustwasm.github.io/docs/wasm-pack/tutorials/index.html
[template-docs]: https://rustwasm.github.io/docs/wasm-pack/tutorials/npm-browser-packages/index.html

## 🚴 Develop

### 🐑 Use `cargo generate` to Clone this Template

[Learn more about `cargo generate` here.](https://github.com/ashleygwilliams/cargo-generate)

```
cargo generate --git https://github.com/rustwasm/wasm-pack-template.git --name my-project
cd my-project
```

### 🛠️ Build with `wasm-pack build`

```
wasm-pack build
```

### 🔬 Test in Headless Browsers with `wasm-pack test`

```
wasm-pack test --headless --firefox
```

### 🎁 Publish to NPM with `wasm-pack publish`

```
wasm-pack publish
```

## 🔋 Batteries Included

* [`wasm-bindgen`](https://github.com/rustwasm/wasm-bindgen) for communicating
  between WebAssembly and JavaScript.
* [`console_error_panic_hook`](https://github.com/rustwasm/console_error_panic_hook)
  for logging panic messages to the developer console.
* [`wee_alloc`](https://github.com/rustwasm/wee_alloc), an allocator optimized
  for small code size.

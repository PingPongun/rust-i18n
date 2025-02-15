# Rust I18n

> 🎯 Let's make I18n things to easy!

Rust I18n is a crate for loading localized text from a set of (YAML, JSON or TOML) mapping files. The mappings are converted into data readable by Rust programs at compile time, and then localized text can be loaded by simply calling the provided `t!` macro.

Unlike other I18n libraries, Rust I18n's goal is to provide a simple and easy-to-use API.

The API of this crate is inspired by [ruby-i18n](https://github.com/ruby-i18n/i18n) and [Rails I18n](https://guides.rubyonrails.org/i18n.html).

>## Difference from [Crates.io/longbridgeapp/upstream version](https://github.com/longbridgeapp/rust-i18n)
>
>This fork has some important improvements mostly to extractor/generetor:
>
>### Extractor/generator (cargo-i18n)
>
>- Can extract translation-keys, even if t!() is “hidden” behind macros/derives
>- Genereted files are sorted in alphabetic order
>- Genereted files are configurable by Config.toml (version 1/2; yaml, json, toml)
>- cargo-i18n can be installed from git
>- Extract `DONE` marked translations from `TODO.*` files and move them to "done" file
>- Extract keys no longer present in code from "done" files and move them to `REMOVED.*` files
>- Keep all translations files sorted (even after manual edit)
>- Translation files can be converted between versions/file formats, by simply changing settings in Config.toml
>- Default translated text in `TODO.*` files is now taken from default locale "done" file (if present)
>- Extractor works correctly with trailing dots (eg. `t!("testing...")` would result in entry `testing... : testing...` instead `testing... : ""`)
>- Supports following syntax `t!( #[doc="Download falied. Contact support"] "error.download.desc")` would extract as `error.download.desc : Download falied. Contact support` (for default locale this would be already placed in done file)(this can be done with more elegant syntax, but this way code is still compatible with upstream crate)
>
>### Other (rust-i18n)
>
>Unless you need some of bellow features/changes, you can use crate from Crates.io in your project and only use extractor from this repo
>
>- `ToStringI18N` trait and derive macro for converting enum to translated string
>- macro `t!` includes #[allow(unused_doc_comment)], so if using doc comment to pass default value clippy is silent
>- dependencies:
>   - remove unused
>   - optimize
>   - uses more recent versions
>   - rust-i18n (lib) does not depend on extractor

## Extractor

We provided a `cargo i18n` command line tool for help you extract the untranslated texts from the source code.

Generated files can be configured in Cargo.toml:

- select file version
  1. each locale is written in separate file
  2. all locales in single file
- select file format (yaml, json, toml)

You can install it via:

```bash
cargo install --git "https://github.com/PingPongun/rust-i18n.git"  --bin cargo-i18n --features="extractor" rust-i18n
```

Then you get `cargo i18n` command.

`cargo i18n` internaly uses cargo to expand whole project, so if your project requires some untypical setup (eg. features) pass them through coresponding args (in most cases simple `cargo i18n` or `cargo i18n --all-features` is enough).

You may want to add this simple build script (`build.rs`) to ensure that all changes of translations will be immediately included in build:

```Rust
fn main() {
    println!("cargo:rerun-if-changed=translate");
    println!("cargo:rerun-if-changed=src");
}
```

It is not currently possible to invoke cargo-i18n from build script(results in deadlock).

For demo project see demo from [egui_struct](https://github.com/PingPongun/egui_struct)

### Extractor Config

💡 NOTE: `package.metadata.i18n` config section in Cargo.toml is just work for `cargo i18n` command, if you don't use that, you don't need this config.

```toml
[package.metadata.i18n]
# The available locales for your application, default: ["en"].
# available-locales = ["en", "zh-CN"]

# The default locale, default: "en".
# default-locale = "en"

# Path for your translations YAML file, default: "locales".
# This config for let `cargo i18n` command line tool know where to find your translations.
# You must keep this path same as the one you pass to method `rust_i18n::i18n!`.
# load-path = "locales"

# Choose file version to generate:
# 1 - single locale per file
# 2 - all locales in single file
# generate-version = 2

# Choose generated file extension (yaml/yml, json, toml)
# generate-extension = "yaml"
```

After running command `cargo i18n` the untranslated texts will be extracted and saved into `locales/TODO.en.yml` file.

After you finished translating file remove `TODO.` from its name. You can also mark single `TODO.en.yml` entries as translated by starting them with word `DONE`. Extractor then will find these entries and move them to file `en.yml`.

If keyword has been removed from code and it was already translated (in file `en.yml` or marked with `DONE`), it will be moved to file `REMOVED.en.yml`. You are free to remove `REMOVED.*` files, they have no meaning to i18n, they are only for user convinience.

```bash
$ cd your_project_root_directory
$ cargo i18n

Checking [en] and generating untranslated texts...
Found 1 new texts need to translate.
----------------------------------------
Writing to TODO.en.yml

Checking [fr] and generating untranslated texts...
Found 11 new texts need to translate.
----------------------------------------
Writing to TODO.fr.yml

Checking [zh-CN] and generating untranslated texts...
All thing done.

Checking [zh-HK] and generating untranslated texts...
Found 11 new texts need to translate.
----------------------------------------
Writing to TODO.zh-HK.yml
```

Run `cargo i18n -h` to see help.

<details>

<summary style="font-size:150%;">
    <b>
        rust-i18n usage (same as 
        <a href="https://github.com/longbridgeapp/rust-i18n">upstream</a>
        )
    </b>
</summary>

## Features

- Codegen on compile time for includes translations into binary.
- Global `t!` macro for loading localized text in everywhere.
- Use YAML (default), JSON or TOML format for mapping localized text, and support mutiple files merging.
- `cargo i18n` Command line tool for checking and extract untranslated texts into YAML files.
- Support all localized texts in one file, or split into difference files by locale.

## Usage

Add crate dependencies in your Cargo.toml and setup I18n config:

```toml
[dependencies]
rust-i18n = "2"
```

Load macro and init translations in `lib.rs` or `main.rs`:

```rs
// Load I18n macro, for allow you use `t!` macro in anywhere.
#[macro_use]
extern crate rust_i18n;

// Init translations for current crate.
i18n!("locales");

// Or just use `i18n!`, default locales path is: "locales" in current crate.
i18n!();

// Config fallback missing translations to "en" locale.
// Use `fallback` option to set fallback locale.
i18n!("locales", fallback = "en");
```

Or you can import by use directly:

```rs
// You must import in each files when you wants use `t!` macro.
use rust_i18n::t;

rust_i18n::i18n!("locales");

fn main() {
    println!("{}", t!("hello"));

    // Use `available_locales!` method to get all available locales.
    println!("{:?}", rust_i18n::available_locales!());
}
```

## Locale file

You can use `_version` key to specify the version of the locale file, and the default value is `1`.

### Split Localized Texts into Difference Files

> _version: 1

You can also split the each language into difference files, and you can choise (YAML, JSON, TOML), for example: `en.json`:

```bash
.
├── Cargo.lock
├── Cargo.toml
├── locales
│   ├── zh-CN.yml
│   ├── en.yml
└── src
│   └── main.rs
```

```yml
_version: 1
hello: "Hello world"
messages.hello: "Hello, %{name}"
```

Or use JSON or TOML format, just rename the file to `en.json` or `en.toml`, and the content is like this:

```json
{
  "_version": 1,
  "hello": "Hello world",
  "messages.hello": "Hello, %{name}"
}
```

```toml
hello = "Hello world"

[messages]
hello = "Hello, %{name}"
```

### All Localized Texts in One File

> _version: 2

Make sure all localized files (containing the localized mappings) are located in the `locales/` folder of the project root directory:

```bash
.
├── Cargo.lock
├── Cargo.toml
├── locales
│   ├── app.yml
│   ├── some-module.yml
└── src
│   └── main.rs
└── sub_app
│   └── locales
│   │   └── app.yml
│   └── src
│   │   └── main.rs
│   └── Cargo.toml
```

In the localized files, specify the localization keys and their corresponding values, for example, in `app.yml`:


```yml
_version: 2
hello:
  en: Hello world
  zh-CN: 你好世界
messages.hello:
  en: Hello, %{name}
  zh-CN: 你好，%{name}
```

This is useful when you use [GitHub Copilot](https://github.com/features/copilot), after you write a first translated text, then Copilot will auto generate other locale's translations for you.

<img src="https://user-images.githubusercontent.com/5518/262332592-7b6cf058-7ef4-4ec7-8dea-0aa3619ce6eb.gif" width="446" />

### Get Localized Strings in Rust

Import the `t!` macro from this crate into your current scope:

```rs
use rust_i18n::t;
```

Then, simply use it wherever a localized string is needed:

```rs
t!("hello");
// => "Hello world"

t!("hello", locale = "zh-CN");
// => "你好世界"

t!("messages.hello", name = "world");
// => "Hello, world"

t!("messages.hello", "name" => "world");
// => "Hello, world"

t!("messages.hello", locale = "zh-CN", name = "Jason", count = 2);
// => "你好，Jason (2)"

t!("messages.hello", locale = "zh-CN", "name" => "Jason", "count" => 3 + 2);
// => "你好，Jason (5)"
```

### Current Locale

You can use `rust_i18n::set_locale` to set the global locale at runtime, so that you don't have to specify the locale on each `t!` invocation.

```rs
rust_i18n::set_locale("zh-CN");

let locale = rust_i18n::locale();
assert_eq!(locale, "zh-CN");
```

### Extend Backend

Since v2.0.0 rust-i18n support extend backend for cusomize your translation implementation.

For example, you can use HTTP API for load translations from remote server:

```rs
use rust_i18n::Backend;

pub struct RemoteI18n {
    trs: IndexMap<String, IndexMap<String, String>>,
}

impl RemoteI18n {
    fn new() -> Self {
        // fetch translations from remote URL
        let response = reqwest::blocking::get("https://your-host.com/assets/locales.yml").unwrap();
        let trs = serde_yaml::from_str::<IndexMap<String, IndexMap<String, String>>>(&response.text().unwrap()).unwrap();

        return Self {
            trs
        };
    }
}

impl Backend for RemoteI18n {
    fn available_locales(&self) -> Vec<&str> {
        return self.trs.keys().collect();
    }

    fn translate(&self, locale: &str, key: &str) -> Option<&str> {
        // Write your own lookup logic here.
        // For example load from database
        return self.trs.get(locale)?.get(key);
    }
}
```

Now you can init rust_i18n by extend your own backend:

```rs
rust_i18n::i18n!("locales", backend = RemoteI18n::new());
```

This also will load local translates from ./locales path, but your own `RemoteI18n` will priority than it.

Now you call `t!` will lookup translates from your own backend first, if not found, will lookup from local files.

## Example

A minimal example of using rust-i18n can be found [here](https://github.com/longbridgeapp/rust-i18n/tree/main/examples).

## I18n Ally

I18n Ally is a VS Code extension for helping you translate your Rust project.

You can add [i18n-ally-custom-framework.yml](https://github.com/longbridgeapp/rust-i18n/blob/main/.vscode/i18n-ally-custom-framework.yml) to your project `.vscode` directory, and then use I18n Ally can parse `t!` marco to show translate text in VS Code editor.

## Debugging the Codegen Process

The `RUST_I18N_DEBUG` environment variable can be used to print out some debugging infos when code is being generated at compile time.

```bash
$ RUST_I18N_DEBUG=1 cargo build
```

## Benchmark

Benchmark `t!` method, result on Apple M1:

```bash
t                       time:   [100.91 ns 101.06 ns 101.24 ns]
t_with_args             time:   [495.56 ns 497.88 ns 500.64 ns]
```

The result `101 ns (0.0001 ms)` means if there have 10K translate texts, it will cost 1ms.

## License

MIT

</details>

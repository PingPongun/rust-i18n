use crate::extractor::Message;
use indexmap::IndexMap;
use rust_i18n_support::config::I18nConfig;
use rust_i18n_support::load_locales;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::io::prelude::*;
use std::path::Path;

type Translations = IndexMap<String, IndexMap<String, String>>;
/// Translations can be either IndexMap<locale, IndexMap<text_key, text>> or IndexMap<text_key, IndexMap<locale, text>>
/// this function changes between them
fn translations_transpose(i: &Translations) -> Translations {
    let mut out = Translations::new();
    i.iter().for_each(|(okey, oval)| {
        oval.iter().for_each(|(ikey, ival)| {
            out.entry(ikey.clone())
                .or_default()
                .insert(okey.clone(), ival.clone());
        })
    });
    out
}

#[derive(Serialize, Deserialize)]
struct FileVer1 {
    _version: usize,
    #[serde(flatten)]
    // #[serde(with = "indexmap::map::serde_seq")]
    translations: IndexMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct FileVer2 {
    _version: usize,
    #[serde(flatten)]
    // #[serde(with = "indexmap::map::serde_seq")]
    translations: Translations,
}

pub fn generate<'a, P: AsRef<Path>>(
    output: P,
    cfg: &I18nConfig,
    messages: impl IntoIterator<Item = &'a Message> + Clone,
) {
    // ~/work/my-project/locales
    let output_path = output.as_ref().display().to_string();

    let ignore_file_ndone = |fname: &str| fname.contains("TODO") || fname.contains("REMOVED");
    let ignore_file_ntodo = |fname: &str| !fname.contains("TODO");
    let ignore_file_nremoved = |fname: &str| !fname.contains("REMOVED");
    let mut data_done = load_locales(&output_path, ignore_file_ndone);
    let mut data_todo = load_locales(&output_path, ignore_file_ntodo);
    let mut data_removed = load_locales(&output_path, ignore_file_nremoved);

    update_todo_done_removed(
        &mut data_done,
        &mut data_todo,
        &mut data_removed,
        &cfg.default_locale,
        &IndexMap::new(),
        messages.clone(),
    );
    let data_done_default = data_done.get(&cfg.default_locale).unwrap().clone();
    let mut all_locales = cfg.available_locales.clone();
    all_locales.retain(|locale| locale != &cfg.default_locale);
    for locale in all_locales {
        update_todo_done_removed(
            &mut data_done,
            &mut data_todo,
            &mut data_removed,
            &locale,
            &data_done_default,
            messages.clone(),
        );
    }

    write_file(&output, "", &cfg, &data_done, &|_, _| ());
    write_file(&output, "TODO.", &cfg, &data_todo, &|count, filename| {
        eprintln!("Found {} new texts need to translate.", count);
        eprintln!("----------------------------------------");
        eprintln!("Writing to {}\n", filename);
    });
    write_file(
        &output,
        "REMOVED.",
        &cfg,
        &data_removed,
        &|count, filename| {
            eprintln!("Found {} unused texts to remove.", count);
            eprintln!("----------------------------------------");
            eprintln!("Writing them to {}\n", filename);
        },
    );
}

fn update_todo_done_removed<'a>(
    data_done: &mut Translations,
    data_todo: &mut Translations,
    data_removed: &mut Translations,
    locale: &String,
    default_val: &IndexMap<String, String>,
    messages: impl IntoIterator<Item = &'a Message> + Clone,
) {
    println!("Checking [{}] and generating untranslated texts...", locale);
    let list_done = data_done.entry(locale.clone()).or_default();
    let list_todo = data_todo.entry(locale.clone()).or_default();
    let list_removed = data_removed.entry(locale.clone()).or_default();

    let label_todo_done = "DONE";
    list_todo.into_iter().for_each(|(key, value)| {
        if let Some(stripped) = value.strip_prefix(label_todo_done) {
            //"TODO.*.yml" file entry, but marked with @DONE@, so move it to "Done" file and strip this prefix
            list_done.insert(key.clone(), stripped.trim_start().to_string());
        }
    });
    list_todo.clear();
    let mut list_done_to_removed = list_done.clone();
    list_done.clear();
    // TODO.en.yml
    for m in messages {
        if !m.locations.is_empty() {
            for _l in &m.locations {
                // TODO: write file and line as YAML comment
            }
        }

        if list_done_to_removed.contains_key(&m.key) {
            list_done.insert(
                m.key.clone(),
                list_done_to_removed.swap_remove(&m.key).unwrap(),
            );
            continue;
        }

        let value = if let Some(val) = default_val.get(&m.key) {
            //get value from default_locale DONE
            //this is usefull if value is longer text, while key was kept short
            val.clone()
        } else {
            m.key.split('.').last().unwrap_or_default().to_string()
        };

        list_todo.insert(m.key.clone(), value);
    }

    //move entries from DONE, that has not not be found in newly extracted ones, to REMOVED file
    list_removed.extend(list_done_to_removed.drain(..));
}

fn write_file<P: AsRef<Path>>(
    output: &P,
    filename_prefix: &str,
    cfg: &I18nConfig,
    translations: &Translations,
    msg: &dyn Fn(usize, &str),
) {
    match cfg.generate_version {
        1 => {
            for locale in &cfg.available_locales {
                let file_data = translations.get(locale).unwrap();
                if !file_data.is_empty() {
                    let mut file_data = file_data.clone();
                    file_data.sort_unstable_keys();
                    let file_data = FileVer1 {
                        _version: 1,
                        translations: file_data,
                    };
                    write_file_inner(
                        output,
                        file_data.translations.len(),
                        file_data,
                        cfg,
                        filename_prefix,
                        locale.as_str(),
                        msg,
                    )
                }
            }
        }
        2 => {
            let translations = translations_transpose(translations);
            if !translations.is_empty() {
                let mut file_data = translations.clone();
                file_data.sort_unstable_keys();
                let file_data = FileVer2 {
                    _version: 2,
                    translations: file_data,
                };
                write_file_inner(
                    output,
                    file_data.translations.len(),
                    file_data,
                    cfg,
                    filename_prefix,
                    "app",
                    msg,
                )
            }
        }
        _ => panic!("Generator does not support sellected version. Supported versions: [1, 2]"),
    }
}

fn write_file_inner<IN: serde::Serialize, P: AsRef<Path>>(
    output: &P,
    count: usize,
    file_data: IN,
    cfg: &I18nConfig,
    filename_prefix: &str,
    filename_mid: &str,
    msg: &dyn Fn(usize, &str),
) {
    let (file_string, file_ext) = format_data(&file_data, cfg.generate_extension.as_str());
    let mut filename = String::from(filename_prefix);
    filename.push_str(filename_mid);
    filename.push_str(file_ext);

    let output_file = std::path::Path::new(output.as_ref()).join(filename.clone());
    let mut output = ::std::fs::File::create(&output_file)
        .unwrap_or_else(|_| panic!("Unable to create {} file", &output_file.display()));
    msg(count, &filename);
    writeln!(output, "{}", file_string).expect("Write file error");
}
fn format_data<IN: serde::Serialize>(trs: &IN, format: &str) -> (String, &'static str) {
    match format {
        "json" => (serde_json::to_string_pretty(trs).unwrap(), ".json"),
        "toml" => (toml::to_string_pretty(trs).unwrap(), ".toml"),
        "yaml" | "yml" | _ => (
            {
                let text = serde_yaml::to_string(trs).unwrap();
                // Remove leading `---`
                text.trim_start_matches("---").trim_start().to_string()
            },
            ".yml",
        ),
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use indoc::indoc;

//     fn assert_eq_json(left: &str, right: &str) {
//         let left: serde_json::Value = serde_json::from_str(left).unwrap();
//         let right: serde_json::Value = serde_json::from_str(right).unwrap();
//         assert_eq!(left, right);
//     }

//     #[test]
//     fn test_convert_text() {
//         let mut trs = Translations::new();
//         let format = "json";

//         let result = convert_text(&trs, format);
//         let expect = r#"
//         {
//             "_version": 2
//         }
//         "#;
//         assert_eq_json(&result, &expect);

//         trs.insert("hello".to_string(), {
//             let mut map = IndexMap::new();
//             map.insert("en".to_string(), "Hello".to_string());
//             map.insert("zh".to_string(), "你好".to_string());
//             map
//         });

//         let result = convert_text(&trs, format);
//         let expect = r#"
//         {
//             "_version": 2,
//             "hello": {
//                 "en": "Hello",
//                 "zh": "你好"
//             }
//         }
//         "#;
//         assert_eq_json(&result, &expect);

//         let format = "yaml";
//         let result = convert_text(&trs, format);
//         let expect = indoc! {r#"
//         _version: 2
//         hello:
//           en: Hello
//           zh: 你好
//         "#};
//         assert_eq!(&result, &expect);

//         let format = "toml";
//         let result = convert_text(&trs, format);
//         let expect = indoc! {r#"
//         _version = 2

//         [hello]
//         en = "Hello"
//         zh = "你好"
//         "#};
//         assert_eq!(&result, &expect);
//     }
// }

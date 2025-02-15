use anyhow::Error;
use indexmap::IndexMap;
use proc_macro2::{Delimiter, Literal, TokenStream, TokenTree};
use quote::ToTokens;
use std::path::PathBuf;

pub type Results = IndexMap<String, Message>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: std::path::PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    pub key: String,
    pub val: Option<String>,
    pub index: usize,
    pub locations: Vec<Location>,
}

impl Message {
    fn new(key: &str, index: usize, val: Option<String>) -> Self {
        Self {
            key: key.to_owned(),
            val,
            index,
            locations: vec![],
        }
    }
}

static METHOD_NAME_MACRO: &str = "t";
static METHOD_NAME_FN: &str = "_rust_i18n_translate";

#[allow(clippy::ptr_arg)]
pub fn extract(results: &mut Results, path: &PathBuf, source: &str) -> Result<(), Error> {
    let mut ex = Extractor { results, path };

    let file = syn::parse_file(source)
        .unwrap_or_else(|_| panic!("Failed to parse file, file: {}", path.display()));
    let stream = file.into_token_stream();
    ex.invoke(stream)
}

#[allow(dead_code)]
struct Extractor<'a> {
    results: &'a mut Results,
    path: &'a PathBuf,
}

impl<'a> Extractor<'a> {
    fn invoke(&mut self, stream: TokenStream) -> Result<(), Error> {
        let mut token_iter = stream.into_iter().peekable();

        while let Some(token) = token_iter.next() {
            match token {
                TokenTree::Group(group) => self.invoke(group.stream())?,
                TokenTree::Ident(ident) => {
                    let mut is_macro = false;
                    if let Some(TokenTree::Punct(punct)) = token_iter.peek() {
                        if punct.to_string() == "!" {
                            is_macro = true;
                            token_iter.next();
                        }
                    }

                    if ident == METHOD_NAME_MACRO && is_macro {
                        if let Some(TokenTree::Group(group)) = token_iter.peek() {
                            self.take_message_macro(group.stream());
                        }
                    } else if ident == METHOD_NAME_FN {
                        if let Some(TokenTree::Group(group)) = token_iter.peek() {
                            self.take_message_fn(group.stream());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn take_message_inner(&mut self, lit: Literal, val: Option<String>) {
        if let Some(key) = literal_to_string(&lit) {
            let message_key = format_message_key(&key);

            let index = self.results.len();
            let message = self
                .results
                .entry(message_key.clone())
                .or_insert_with(|| Message::new(&message_key, index, val));

            let span = lit.span();
            let line = span.start().line;
            if line > 0 {
                message.locations.push(Location {
                    file: self.path.clone(),
                    line,
                });
            }
        }
    }

    fn take_message_macro(&mut self, stream: TokenStream) {
        let mut token_iter = stream.into_iter();

        let literal = if let Some(TokenTree::Literal(literal)) = token_iter.next() {
            literal
        } else {
            return;
        };
        self.take_message_inner(literal, None);
    }
    fn take_message_fn(&mut self, stream: TokenStream) {
        let mut token_iter = stream.into_iter();

        #[derive(PartialEq)]
        enum DocCommnetState {
            None,
            Hash,
            Group,
        }
        let mut state = DocCommnetState::None;
        let mut literal = None;
        let mut comment = None;
        while let Some(tok) = token_iter.next() {
            match tok {
                TokenTree::Punct(x) if x.as_char() == '#' && state != DocCommnetState::Hash => {
                    state = DocCommnetState::Hash
                }
                TokenTree::Group(group)
                    if state == DocCommnetState::Hash
                        && group.delimiter() == Delimiter::Bracket =>
                {
                    let mut i = group.stream().into_iter();
                    let mut doc = true;
                    if let Some(TokenTree::Ident(ident)) = i.next() {
                        doc &= ident.to_string() == "doc"
                    } else {
                        doc = false
                    };
                    if let Some(TokenTree::Punct(punct)) = i.next() {
                        doc &= punct.as_char() == '='
                    } else {
                        doc = false
                    };
                    if doc {
                        if let Some(TokenTree::Literal(lit)) = i.next() {
                            let lit = lit
                                .to_string()
                                .trim()
                                .trim_start_matches("r\"")
                                .trim_end_matches('\"')
                                .trim_start_matches('\"')
                                .trim()
                                .to_string();
                            if let Some(cmt) = comment {
                                comment = Some(cmt + "\r\n" + lit.as_str())
                            } else {
                                comment = Some(lit)
                            }
                        } else {
                            doc = false
                        };
                    }

                    if !doc {
                        state = DocCommnetState::None
                    } else {
                        state = DocCommnetState::Group
                    }
                }
                TokenTree::Literal(lit) => {
                    if state != DocCommnetState::Group {
                        comment = None;
                    }
                    //take last literal in stream
                    literal = Some(lit);
                    state = DocCommnetState::None
                }
                _ => state = DocCommnetState::None,
            }
        }
        if let Some(literal) = literal {
            self.take_message_inner(literal, comment);
        }
    }
}

fn literal_to_string(lit: &proc_macro2::Literal) -> Option<String> {
    match syn::parse_str::<syn::LitStr>(&lit.to_string()) {
        Ok(lit) => Some(lit.value()),
        Err(_) => None,
    }
}

fn format_message_key(key: &str) -> String {
    let re = regex::Regex::new(r"\s+").unwrap();
    let key = re.replace_all(key, " ").into_owned();
    key.trim().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    macro_rules! build_messages {
        {$(($key:tt, $val:expr, $($line:tt),+)),+} => {{
            let mut results = Vec::<Message>::new();
            $(
                let message = Message {
                    key: $key.into(),
                    val: $val,
                    locations: vec![
                        $(
                            Location {
                                file: PathBuf::from_str("hello.rs").unwrap(),
                                line: $line
                            },
                        )+
                    ],
                    index: 0,
                };
                results.push(message);
            )+

            results
        }}
    }

    #[test]
    fn test_format_message_key() {
        assert_eq!(format_message_key("Hello world"), "Hello world".to_owned());
        assert_eq!(format_message_key("\n    "), "".to_owned());
        assert_eq!(format_message_key("\n    "), "".to_owned());
        assert_eq!(format_message_key("\n    hello"), "hello".to_owned());
        assert_eq!(format_message_key("\n    hello\n"), "hello".to_owned());
        assert_eq!(format_message_key("\n    hello\n    "), "hello".to_owned());
        assert_eq!(
            format_message_key("\n    hello\n    world"),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key("\n    hello\n    world\n\n"),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key("\n    hello\n    world\n    "),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key("    hello\n    world\n    "),
            "hello world".to_owned()
        );
        assert_eq!(
            format_message_key(
                r#"Use YAML for mapping localized text, 
            and support mutiple YAML files merging."#
            ),
            "Use YAML for mapping localized text, and support mutiple YAML files merging."
                .to_owned()
        );
    }

    #[test]
    fn test_extract() {
        let source = include_str!("example.test.rs");
        let stream = proc_macro2::TokenStream::from_str(source).unwrap();

        let expected = build_messages![
            ("hello", None, 4),
            ("views.message.title", None, 5),
            ("views.message.description", None, 7),
            (
                "Use YAML for mapping localized text, and support mutiple YAML files merging.",
                None,
                11,
                14
            ),
            (
                "The table below describes some of those behaviours.",
                None,
                18,
                20
            ),
            ("Unfolded.test1.test", Some("TESTTT".to_string()), 25)
        ];

        let mut results = IndexMap::new();

        let mut ex = Extractor {
            results: &mut results,
            path: &"hello.rs".to_owned().into(),
        };

        ex.invoke(stream).unwrap();

        let mut messages: Vec<_> = ex.results.values().collect();
        messages.sort_by_key(|m| m.index);
        assert_eq!(expected.len(), messages.len());

        for (expected_message, actually_message) in expected.iter().zip(messages) {
            let mut actually_message = actually_message.clone();
            actually_message.index = 0;

            assert_eq!(*expected_message, actually_message);
        }
    }
}

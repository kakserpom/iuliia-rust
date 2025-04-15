#[macro_use]
extern crate include_dir;
extern crate regex;

use include_dir::Dir;
use lazy_static::lazy_static;
use regex::Regex;

use serde::Deserialize;
use std::collections::HashMap;
use std::iter::once;

const SCHEMA_DIR: Dir = include_dir!("./iuliia");
const DUMMY_SYMBOL: &str = "$";

/// Describe struct of transliterate schema
#[derive(Deserialize, Debug)]
pub struct Schema {
    name: String,
    description: String,
    url: String,
    mapping: Option<HashMap<String, String>>,
    prev_mapping: Option<HashMap<String, String>>,
    next_mapping: Option<HashMap<String, String>>,
    ending_mapping: Option<HashMap<String, String>>,
    samples: Option<Vec<Vec<String>>>,
}

impl Schema {
    /// Return Schema object by schema name
    pub fn for_name(s: &str) -> Schema {
        let schema_file = SCHEMA_DIR
            .get_file(format!("{}{}", s, ".json"))
            .unwrap_or_else(|| panic!("There are no schema with name {}", s));
        serde_json::from_str(schema_file.contents_utf8().expect("contents_utf8() failed"))
            .expect("Schema deserialization error")
    }

    pub fn get_pref(&self, s: &str) -> Option<&String> {
        self.prev_mapping
            .as_ref()
            .and_then(|x| x.get(&s.replace(DUMMY_SYMBOL, "").to_lowercase()))
    }

    pub fn get_next(&self, s: &str) -> Option<&String> {
        self.next_mapping
            .as_ref()
            .and_then(|x| x.get(&s.replace(DUMMY_SYMBOL, "").to_lowercase()))
    }

    pub fn get_letter(&self, s: &str) -> Option<&String> {
        self.mapping
            .as_ref()
            .and_then(|x| x.get(&s.replace(DUMMY_SYMBOL, "").to_lowercase()))
    }

    pub fn get_ending(&self, s: &str) -> Option<&String> {
        self.ending_mapping
            .as_ref()
            .and_then(|x| x.get(&s.to_lowercase()))
    }
}

/// Transliterate a slice of str using name of schema to `String`
///
/// ```
/// assert_eq!(iuliia_rust::parse_by_schema_name("Юлия", "wikipedia"), "Yuliya")
/// ```
///
pub fn parse_by_schema_name(s: &str, schema_name: &str) -> String {
    let schema = Schema::for_name(schema_name);
    parse_by_schema(s, &schema)
}

/// Transliterate a slice of str using `Schema` to `String`
///
/// ```
///
/// let input = "Юлия, съешь ещё этих мягких французских булок из Йошкар-Олы, да выпей алтайского чаю";
/// let expected = "Yuliya, syesh yeshchyo etikh myagkikh frantsuzskikh bulok iz Yoshkar-Oly, da vypey altayskogo chayu";
/// let schema = iuliia_rust::Schema::for_name("wikipedia");
///
/// let transliterated_word = iuliia_rust::parse_by_schema(&input, &schema);
///
/// assert_eq!(transliterated_word, expected)
/// ```
///
pub fn parse_by_schema(s: &str, schema: &Schema) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"\b").expect("Failed to compile regex");
    }
    RE.split(s)
        .map(|word| parse_word_by_schema(word, schema))
        .collect()
}

pub fn parse_word_by_schema(s: &str, schema: &Schema) -> String {
    let word_by_letters: Vec<String> = s.chars().map(|char| char.to_string()).collect::<Vec<_>>();
    //Parse ending
    let ending = parse_ending(&word_by_letters, schema);
    let (parsed_end, word_without_ending) = if let Some(ending) = ending {
        (
            ending.translate,
            word_by_letters[..ending.ending_start].to_vec(),
        )
    } else {
        (String::new(), word_by_letters)
    };

    //Add dummy symbols for window function
    //Parse each letter
    once(DUMMY_SYMBOL.into())
        .chain(word_without_ending)
        .chain(once(DUMMY_SYMBOL.into()))
        .collect::<Vec<_>>()
        .windows(3)
        .map(|letter_with_neighbors| parse_letter(letter_with_neighbors, schema))
        .chain(once(parsed_end))
        .collect::<String>()
}

fn parse_ending(s: &[String], schema: &Schema) -> Option<Ending> {
    let length = s.len();
    if length < 3 {
        None
    } else if let Some(matched) = schema.get_ending(&s[length - 1..].concat()) {
        Some(Ending {
            translate: propagate_case_from_source(matched, &s[length - 1..].concat(), false),
            ending_start: length - 1,
        })
    } else {
        schema
            .get_ending(&s[length - 2..].concat())
            .map(|matched| Ending {
                translate: propagate_case_from_source(matched, &s[length - 2..].concat(), false),
                ending_start: length - 2,
            })
    }
}

struct Ending {
    translate: String,
    ending_start: usize,
}

/// Find letter transliteration with steps priority(apply higher):
/// 1. prefix parse
/// 2. postfix parse
/// 3. letter parse
/// 4. use input letter
fn parse_letter(letter_with_neighbors: &[String], schema: &Schema) -> String {
    let letter: String = letter_with_neighbors[1..2].concat();
    propagate_case_from_source(
        schema
            .get_pref(&letter_with_neighbors[..2].concat())
            .or_else(|| schema.get_next(&letter_with_neighbors[1..].concat()))
            .or_else(|| schema.get_letter(&letter))
            .unwrap_or(&letter),
        &letter,
        true,
    )
}

fn propagate_case_from_source(
    result: &String,
    source_letter: &str,
    only_first_symbol: bool,
) -> String {
    // Determinate case of letter
    if !source_letter.chars().any(|letter| letter.is_uppercase()) {
        result.to_owned()
    } else if only_first_symbol {
        let mut c = result.chars();
        if let Some(f) = c.next() {
            f.to_uppercase().collect::<String>() + c.as_str()
        } else {
            String::new()
        }
    } else {
        result.to_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use crate::{parse_by_schema, Schema};

    #[test]
    fn schema_test() {
        let schema = Schema::for_name("ala_lc");
        assert_eq!(schema.name, "ala_lc")
    }

    #[test]
    fn simple_word_test() {
        //Given
        let test_words = vec!["б", "пол"];
        let expected_words = vec!["b", "pol"];
        let schema = Schema::for_name("wikipedia");

        //When
        let transliterated_words: Vec<String> = test_words
            .iter()
            .map(|word| parse_by_schema(&word, &schema))
            .collect();

        //Then
        assert_eq!(transliterated_words, expected_words)
    }

    #[test]
    fn prefix_word_test() {
        //Given
        let test_words = vec!["ель"];
        let expected_words = vec!["yel"];
        let schema = Schema::for_name("wikipedia");

        //When
        let transliterated_words: Vec<String> = test_words
            .iter()
            .map(|word| parse_by_schema(&word, &schema))
            .collect();

        //Then
        assert_eq!(transliterated_words, expected_words)
    }

    #[test]
    fn postfix_word_test() {
        //Given
        let test_words = vec!["бульон"];
        let expected_words = vec!["bulyon"];
        let schema = Schema::for_name("wikipedia");

        //When
        let transliterated_words: Vec<String> = test_words
            .iter()
            .map(|word| parse_by_schema(&word, &schema))
            .collect();

        //Then
        assert_eq!(transliterated_words, expected_words)
    }

    #[test]
    fn test_letter_case() {
        //Given
        let test_words = vec!["ноГа", "Рука"];
        let expected_words = vec!["noGa", "Ruka"];
        let schema = Schema::for_name("wikipedia");

        //When
        let transliterated_words: Vec<String> = test_words
            .iter()
            .map(|word| parse_by_schema(&word, &schema))
            .collect();

        //Then
        assert_eq!(transliterated_words, expected_words)
    }

    #[test]
    fn test_ending() {
        //Given
        let test_words = vec!["хороший"];
        let expected_words = vec!["khoroshy"];
        let schema = Schema::for_name("wikipedia");

        //When
        let transliterated_words: Vec<String> = test_words
            .iter()
            .map(|word| parse_by_schema(&word, &schema))
            .collect();

        //Then
        assert_eq!(transliterated_words, expected_words)
    }

    #[test]
    fn test_sentence() {
        //Given
        let test_words = vec![
            "Юлия, съешь ещё этих мягких французских булок из Йошкар-Олы, да выпей алтайского чаю",
            "ВЕЛИКИЙ",
        ];
        let expected_words = vec!["Yuliya, syesh yeshchyo etikh myagkikh frantsuzskikh bulok iz Yoshkar-Oly, da vypey altayskogo chayu", "VELIKY"];
        let schema = Schema::for_name("wikipedia");

        //When
        let transliterated_words: Vec<String> = test_words
            .iter()
            .map(|word| parse_by_schema(&word, &schema))
            .collect();

        //Then
        assert_eq!(transliterated_words, expected_words)
    }
}

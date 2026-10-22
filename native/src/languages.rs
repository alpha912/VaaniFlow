//! Language codes currently exposed by Sarvam's speech and translation APIs.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
}

pub const LANGUAGES: &[Language] = &[
    Language {
        code: "as-IN",
        name: "Assamese",
    },
    Language {
        code: "bn-IN",
        name: "Bengali",
    },
    Language {
        code: "brx-IN",
        name: "Bodo",
    },
    Language {
        code: "doi-IN",
        name: "Dogri",
    },
    Language {
        code: "en-IN",
        name: "English",
    },
    Language {
        code: "gu-IN",
        name: "Gujarati",
    },
    Language {
        code: "hi-IN",
        name: "Hindi",
    },
    Language {
        code: "kn-IN",
        name: "Kannada",
    },
    Language {
        code: "ks-IN",
        name: "Kashmiri",
    },
    Language {
        code: "kok-IN",
        name: "Konkani",
    },
    Language {
        code: "mai-IN",
        name: "Maithili",
    },
    Language {
        code: "ml-IN",
        name: "Malayalam",
    },
    Language {
        code: "mni-IN",
        name: "Manipuri",
    },
    Language {
        code: "mr-IN",
        name: "Marathi",
    },
    Language {
        code: "ne-IN",
        name: "Nepali",
    },
    Language {
        code: "or-IN",
        name: "Odia",
    },
    Language {
        code: "pa-IN",
        name: "Punjabi",
    },
    Language {
        code: "sa-IN",
        name: "Sanskrit",
    },
    Language {
        code: "sat-IN",
        name: "Santali",
    },
    Language {
        code: "sd-IN",
        name: "Sindhi",
    },
    Language {
        code: "ta-IN",
        name: "Tamil",
    },
    Language {
        code: "te-IN",
        name: "Telugu",
    },
    Language {
        code: "ur-IN",
        name: "Urdu",
    },
];

pub fn language_name(code: &str) -> &'static str {
    match code {
        "auto" => "Auto-detect",
        "same" => "Same as source",
        other => LANGUAGES
            .iter()
            .find(|language| language.code == other)
            .map_or("Choose language", |language| language.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_over_twenty_languages_and_keeps_all_codes_unique() {
        assert!(LANGUAGES.len() >= 22);
        for (index, language) in LANGUAGES.iter().enumerate() {
            assert!(!LANGUAGES[..index]
                .iter()
                .any(|previous| previous.code == language.code));
        }
    }
}

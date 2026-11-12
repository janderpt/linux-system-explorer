use serde::{Deserialize, Serialize};
use std::{cell::Cell, collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    System,
    Portuguese,
    English,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    Portuguese,
    English,
}

impl Language {
    pub fn resolve(self) -> Locale {
        match self {
            Self::System => system_locale(),
            Self::Portuguese => Locale::Portuguese,
            Self::English => Locale::English,
        }
    }
}

fn supported(value: &str) -> Option<Locale> {
    let language = value
        .split(['_', '-', '.', '@'])
        .next()?
        .to_ascii_lowercase();
    match language.as_str() {
        "pt" => Some(Locale::Portuguese),
        "en" => Some(Locale::English),
        _ => None,
    }
}

fn detect(mut variable: impl FnMut(&str) -> Option<String>) -> Locale {
    let base = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| variable(key).filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| "C".into());
    let neutral = base.split(['.', '@']).next().unwrap_or("C");
    if neutral.eq_ignore_ascii_case("C") || neutral.eq_ignore_ascii_case("POSIX") {
        return Locale::English;
    }
    if let Some(preferred) = variable("LANGUAGE") {
        for language in preferred.split(':') {
            if let Some(locale) = supported(language.trim()) {
                return locale;
            }
        }
    }
    supported(&base).unwrap_or(Locale::English)
}

#[cfg(not(test))]
fn system_locale() -> Locale {
    static SYSTEM: OnceLock<Locale> = OnceLock::new();
    *SYSTEM.get_or_init(|| detect(|key| std::env::var(key).ok()))
}

// UI tests start with a deterministic system language, independently of the host/session.
#[cfg(test)]
fn system_locale() -> Locale {
    Locale::Portuguese
}

thread_local! {
    static ACTIVE: Cell<Locale> = Cell::new(system_locale());
}

pub fn current() -> Locale {
    ACTIVE.with(Cell::get)
}
pub fn set_locale(locale: Locale) {
    ACTIVE.with(|active| active.set(locale));
}

#[derive(Deserialize)]
struct Translation {
    pt: String,
    en: String,
}

fn catalog() -> &'static BTreeMap<String, Translation> {
    static MESSAGES: OnceLock<BTreeMap<String, Translation>> = OnceLock::new();
    MESSAGES.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/messages.json"))
            .expect("invalid embedded translation catalog")
    })
}

pub fn tr(source: &'static str) -> &'static str {
    let Some(translation) = catalog().get(source) else {
        return source;
    };
    match current() {
        Locale::Portuguese => &translation.pt,
        Locale::English => &translation.en,
    }
}

pub fn message(source: &'static str, arguments: &[String]) -> String {
    let template = tr(source);
    let mut result = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}').map(|end| start + end) else {
            result.push_str(&rest[start..]);
            return result;
        };
        let field = &rest[start + 1..end];
        if let Some(argument) = field
            .parse::<usize>()
            .ok()
            .and_then(|index| arguments.get(index))
        {
            result.push_str(argument);
        } else {
            result.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}

pub fn plural(one: &'static str, many: &'static str, count: usize) -> String {
    message(if count == 1 { one } else { many }, &[count.to_string()])
}

pub fn decimal(value: f64) -> String {
    let number = format!("{value:.1}");
    match current() {
        Locale::Portuguese => number.replace('.', ","),
        Locale::English => number,
    }
}

pub fn date_format() -> &'static str {
    match current() {
        Locale::Portuguese => "%d/%m/%Y %H:%M",
        Locale::English => "%Y-%m-%d %H:%M",
    }
}

#[cfg(test)]
pub fn with_locale<R>(locale: Locale, action: impl FnOnce() -> R) -> R {
    struct Restore(Locale);
    impl Drop for Restore {
        fn drop(&mut self) {
            set_locale(self.0);
        }
    }
    let _restore = Restore(current());
    set_locale(locale);
    action()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_languages_and_environment_priority() {
        let resolve = |vars: &[(&str, &str)]| {
            detect(|key| {
                vars.iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, value)| value.to_string())
            })
        };
        assert_eq!(resolve(&[("LANG", "pt_BR.UTF-8")]), Locale::Portuguese);
        assert_eq!(resolve(&[("LANG", "pt-PT")]), Locale::Portuguese);
        assert_eq!(
            resolve(&[
                ("LC_ALL", "en_GB.UTF-8"),
                ("LC_MESSAGES", "pt_BR"),
                ("LANG", "pt_BR")
            ]),
            Locale::English
        );
        assert_eq!(
            resolve(&[("LC_ALL", ""), ("LC_MESSAGES", "pt_PT"), ("LANG", "en_US")]),
            Locale::Portuguese
        );
        assert_eq!(
            resolve(&[("LANG", "de_DE"), ("LANGUAGE", "de:pt_BR:en")]),
            Locale::Portuguese
        );
        assert_eq!(
            resolve(&[("LANG", "pt_BR"), ("LANGUAGE", "en:pt")]),
            Locale::English
        );
        for locale in ["C", "C.UTF-8", "POSIX", "de_DE.UTF-8"] {
            assert_eq!(resolve(&[("LANG", locale)]), Locale::English);
        }
        assert_eq!(
            resolve(&[("LANG", "C"), ("LANGUAGE", "pt_BR")]),
            Locale::English
        );
        assert_eq!(resolve(&[]), Locale::English);
    }

    #[test]
    fn catalog_preserves_placeholders_and_translates_values_without_touching_paths() {
        let fields = |text: &str| -> Vec<String> {
            let mut fields: Vec<_> = text
                .split('{')
                .skip(1)
                .map(|part| part.split('}').next().unwrap().to_owned())
                .collect();
            fields.sort();
            fields
        };
        for (source, translation) in catalog() {
            assert!(
                !translation.pt.is_empty() && !translation.en.is_empty(),
                "{source}"
            );
            assert_eq!(fields(&translation.pt), fields(&translation.en), "{source}");
        }
        with_locale(Locale::English, || {
            assert_eq!(tr("Configurações"), "Settings");
            let path = "/tmp/Nova pasta/{0}/Configurações";
            assert_eq!(
                message("Local: {}", &[path.to_owned()]),
                format!("Location: {path}")
            );
            assert_eq!(plural("{} item", "{} itens", 1), "1 item");
            assert_eq!(plural("{} item", "{} itens", 2), "2 items");
            assert_eq!(decimal(1.5), "1.5");
            assert_eq!(crate::filesystem::format_size(1536), "1.5 KiB");
            assert_eq!(date_format(), "%Y-%m-%d %H:%M");
        });
        with_locale(Locale::Portuguese, || {
            assert_eq!(tr("Configurações"), "Configurações");
            assert_eq!(crate::filesystem::format_size(1536), "1,5 KiB");
            assert_eq!(date_format(), "%d/%m/%Y %H:%M");
        });
    }

    #[test]
    fn all_static_translation_calls_have_catalog_entries() {
        let sources = [
            include_str!("app.rs"),
            include_str!("main.rs"),
            include_str!("network.rs"),
            include_str!("network/gvfs.rs"),
            include_str!("app/network.rs"),
            include_str!("local_shares.rs"),
            include_str!("app/local_shares.rs"),
            include_str!("app/local_shares/sidebar_cache.rs"),
            include_str!("settings.rs"),
            include_str!("columns.rs"),
            include_str!("filesystem.rs"),
            include_str!("icons.rs"),
            include_str!("sidebar.rs"),
            include_str!("deletions.rs"),
            include_str!("transfers.rs"),
            include_str!("settings/sidebar_ui.rs"),
            include_str!("app/breadcrumbs.rs"),
            include_str!("app/deletions.rs"),
            include_str!("app/folder_picker.rs"),
            include_str!("app/sidebar.rs"),
            include_str!("app/table.rs"),
            include_str!("app/toolbar.rs"),
            include_str!("app/transfers.rs"),
            include_str!("app/trash.rs"),
            include_str!("app/empty_trash.rs"),
            include_str!("app/open_with.rs"),
            include_str!("app/rename.rs"),
            include_str!("renames.rs"),
            include_str!("app/drag_drop.rs"),
            include_str!("app/toast.rs"),
            include_str!("native_dnd.rs"),
            include_str!("native_dnd/x11.rs"),
            include_str!("applications.rs"),
            include_str!("trashbin.rs"),
            include_str!("app/preview.rs"),
            include_str!("app/properties.rs"),
            include_str!("preview/archive.rs"),
        ];
        for source in sources {
            for function in [
                "crate::i18n::tr(",
                "crate::i18n::message(",
                "crate::i18n::plural(",
            ] {
                for call in source.split(function).skip(1) {
                    let literal = call.trim_start();
                    if !literal.starts_with('"') {
                        continue;
                    }
                    let mut escaped = false;
                    let end = literal
                        .char_indices()
                        .skip(1)
                        .find_map(|(index, ch)| {
                            if escaped {
                                escaped = false;
                                return None;
                            }
                            if ch == '\\' {
                                escaped = true;
                                return None;
                            }
                            (ch == '"').then_some(index)
                        })
                        .unwrap();
                    let key: String = serde_json::from_str(&literal[..=end]).unwrap();
                    assert!(catalog().contains_key(&key), "missing translation: {key}");
                }
            }
        }
    }
}

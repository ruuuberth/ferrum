//! Ajustes del navegador que sobreviven al cierre de la app.
//!
//! En Android la carpeta privada se obtiene de `AndroidApp::internal_data_path`.
//! En escritorio no hay carpeta configurada todavía: el ajuste vive solo en
//! memoria y se pierde al cerrar, igual que el historial actual.

use std::path::PathBuf;
use std::str::FromStr;

use blitz_traits::net::Url;

/// Motor usado cuando el texto de la barra no es una URL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchEngine {
    #[default]
    DuckDuckGoHtml,
    Google,
}

impl SearchEngine {
    /// Todos los motores, en el orden en que se muestran en Ajustes.
    pub const ALL: [SearchEngine; 2] = [Self::DuckDuckGoHtml, Self::Google];

    /// Clave estable que se guarda en disco. No la cambies sin migrar.
    pub const fn key(self) -> &'static str {
        match self {
            Self::DuckDuckGoHtml => "duckduckgo",
            Self::Google => "google",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::DuckDuckGoHtml => "DuckDuckGo HTML",
            Self::Google => "Google",
        }
    }

    /// Construye la URL de resultados. `query_pairs_mut` codifica espacios,
    /// acentos y caracteres especiales.
    pub fn search_url(self, query: &str) -> Url {
        let base = match self {
            Self::DuckDuckGoHtml => "https://html.duckduckgo.com/html/",
            Self::Google => "https://www.google.com/search",
        };

        #[allow(clippy::expect_used)]
        let mut url = Url::parse(base).expect("las URLs base son constantes válidas");
        url.query_pairs_mut().append_pair("q", query);
        url
    }
}

impl FromStr for SearchEngine {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "duckduckgo" => Ok(Self::DuckDuckGoHtml),
            "google" => Ok(Self::Google),
            _ => Err(()),
        }
    }
}

/// Lee y escribe el ajuste en un archivo de texto.
///
/// Si no hay ruta (escritorio, tests) o el archivo falla, se usa el valor por
/// defecto. Igual que el historial, un fallo de disco nunca rompe la app.
#[derive(Clone)]
pub struct SettingsStore {
    path: Option<PathBuf>,
}

impl SettingsStore {
    pub fn for_current_app() -> Self {
        Self {
            path: data_dir().map(|dir| dir.join("search-engine.txt")),
        }
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    pub fn load(&self) -> SearchEngine {
        let Some(path) = self.path.as_ref() else {
            return SearchEngine::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return SearchEngine::default();
        };
        text.trim().parse().unwrap_or_default()
    }

    pub fn save(&self, engine: SearchEngine) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("settings: no se pudo crear {}: {e}", parent.display());
                return;
            }
        }
        if let Err(e) = std::fs::write(path, engine.key()) {
            tracing::warn!("settings: no se pudo escribir {}: {e}", path.display());
        }
    }
}

/// Carpeta privada de datos de la app en Android.
fn data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        dioxus_native::current_android_app().internal_data_path()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_las_claves_conocidas() {
        assert_eq!(
            "duckduckgo".parse::<SearchEngine>(),
            Ok(SearchEngine::DuckDuckGoHtml)
        );
        assert_eq!("google".parse::<SearchEngine>(), Ok(SearchEngine::Google));
    }

    #[test]
    fn clave_desconocida_es_error() {
        assert_eq!("bing".parse::<SearchEngine>(), Err(()));
    }

    #[test]
    fn por_defecto_duckduckgo() {
        assert_eq!(SearchEngine::default(), SearchEngine::DuckDuckGoHtml);
    }

    #[test]
    fn codifica_la_consulta() {
        let url = SearchEngine::DuckDuckGoHtml.search_url("qué es rust");
        assert_eq!(
            url.as_str(),
            "https://html.duckduckgo.com/html/?q=qu%C3%A9+es+rust"
        );
    }

    #[test]
    fn google_usa_search() {
        let url = SearchEngine::Google.search_url("rust lang");
        assert_eq!(url.as_str(), "https://www.google.com/search?q=rust+lang");
    }

    #[test]
    fn guarda_y_carga() {
        let path = std::env::temp_dir().join(format!(
            "ferrum-settings-{}-{:?}.txt",
            std::process::id(),
            std::thread::current().id()
        ));
        let store = SettingsStore::at(path.clone());

        assert_eq!(store.load(), SearchEngine::DuckDuckGoHtml);
        store.save(SearchEngine::Google);
        assert_eq!(store.load(), SearchEngine::Google);

        let _ = std::fs::remove_file(path);
    }
}

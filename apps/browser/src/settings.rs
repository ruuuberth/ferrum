//! Ajustes del navegador que sobreviven al cierre de la app.
//!
//! Viven en la tabla `settings` de la misma base SQLite que el historial. La
//! carpeta privada la decide `main.rs` al abrir el `BrowserStore`.

use std::str::FromStr;

use blitz_traits::net::Url;
use browser_persistence::BrowserStore;

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

/// Clave con la que el motor se guarda en la tabla `settings`.
const SEARCH_ENGINE_KEY: &str = "search_engine";

/// Lee el motor guardado; si no existe o no se reconoce, DuckDuckGo HTML.
pub fn load_search_engine(store: &BrowserStore) -> SearchEngine {
    store
        .get_setting(SEARCH_ENGINE_KEY)
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

/// Guarda el motor elegido. Un fallo de disco solo se registra en el log.
pub fn save_search_engine(store: &BrowserStore, engine: SearchEngine) {
    store.set_setting(SEARCH_ENGINE_KEY, engine.key());
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
}

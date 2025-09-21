//! Integration tests for the Take It Easy library public API

use take_it_easy::{
    servers::{WebUiServer, WebUiConfig},
    VERSION, NAME, DESCRIPTION,
    TakeItEasyError, Result,
};

#[test]
fn test_library_metadata() {
    assert!(!VERSION.is_empty());
    assert_eq!(NAME, "take_it_easy");
    assert!(!DESCRIPTION.is_empty());
}

#[test]
fn test_error_types() {
    let game_error = TakeItEasyError::Game("test game error".to_string());
    assert!(matches!(game_error, TakeItEasyError::Game(_)));

    let ai_error = TakeItEasyError::Ai("test ai error".to_string());
    assert!(matches!(ai_error, TakeItEasyError::Ai(_)));

    let server_error = TakeItEasyError::Server("test server error".to_string());
    assert!(matches!(server_error, TakeItEasyError::Server(_)));
}

#[test]
fn test_web_ui_server_creation() {
    let config = WebUiConfig {
        port: 8080,
        host: "127.0.0.1".to_string(),
    };

    let _server = WebUiServer::new(config.clone());
    // Test que le serveur se crée sans erreur
    // Note: la méthode config() a été supprimée lors du nettoyage du code
}

#[test]
fn test_result_type_alias() {
    let success: Result<i32> = Ok(42);
    assert!(success.is_ok());
    assert_eq!(success.unwrap(), 42);

    let failure: Result<i32> = Err(TakeItEasyError::Game("test".to_string()));
    assert!(failure.is_err());
}

#[test]
fn test_server_configs() {
    // Test WebUiConfig default
    let web_config = WebUiConfig::default();
    assert_eq!(web_config.port, 51051);
    assert_eq!(web_config.host, "0.0.0.0");
}
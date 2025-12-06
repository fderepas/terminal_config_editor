mod config_edit;

fn main() {
    assert!(config_edit::get_string("config.json", "player1") == Some("alice".to_string()));
    assert!(config_edit::get_string("config.json", "log_level") == Some("info".to_string()));
    assert!(config_edit::get_string("config.json", "does-not-exist").is_none());
    assert!(config_edit::get_option("config.json", "log_level") == Some(2));
    assert!(config_edit::get_option("config.json", "foobar").is_none());
    config_edit::terminal_edit_json("config.json");
}

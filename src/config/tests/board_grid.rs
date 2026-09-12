use crate::config::{BoardGridConfig, BoardGridKindConfig, Config};
use crate::domain::{BoardGrid, BoardGridKind};

#[test]
fn board_grid_config_defaults_and_modes_round_trip() {
    assert_eq!(
        toml::from_str::<BoardGridConfig>("").unwrap(),
        BoardGridConfig::default()
    );
    for (name, kind) in [
        ("none", BoardGridKind::None),
        ("cartesian", BoardGridKind::Cartesian),
        ("isometric", BoardGridKind::Isometric),
        ("isometric-dots", BoardGridKind::IsometricDots),
    ] {
        let parsed: BoardGridConfig =
            toml::from_str(&format!("kind = '{name}'\nspacing = 20")).unwrap();
        assert_eq!(BoardGrid::from(parsed), BoardGrid::new(kind, 20));
        let encoded = toml::to_string(&parsed).unwrap();
        assert_eq!(parsed, toml::from_str(&encoded).unwrap());
    }
    for invalid in ["kind = 'diagonal'", "spacing = 2.5", "spacing = '40'"] {
        assert!(
            toml::from_str::<BoardGridConfig>(invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn board_grid_config_clamps_spacing_and_disables_transparent_patterns() {
    let mut config: Config = toml::from_str("[[boards.items]]\nid = 'whiteboard'\nname = 'Paper'\nbackground = [1.0,1.0,1.0]\ngrid = { kind = 'cartesian', spacing = -20 }\n[[boards.items]]\nid = 'transparent'\nname = 'Overlay'\ngrid = {kind = 'isometric', spacing = 900}").unwrap();
    config.validate_and_clamp();
    let boards = config.boards.unwrap();
    assert_eq!(boards.items[0].grid.spacing, 8);
    assert_eq!(boards.items[0].grid.kind, BoardGridKindConfig::Cartesian);
    assert_eq!(boards.items[1].grid.spacing, 200);
    assert_eq!(boards.items[1].grid.kind, BoardGridKindConfig::None);
}

#[test]
fn board_grid_runtime_mapping_preserves_new_session_templates() {
    let mut config = crate::config::BoardsConfig::default();
    config.items[1].grid = BoardGridConfig {
        kind: BoardGridKindConfig::IsometricDots,
        spacing: 20,
    };
    let mut boards = crate::input::BoardManager::from_config(config);
    assert_eq!(
        boards.board_states()[1].spec.grid,
        BoardGrid::new(BoardGridKind::IsometricDots, 20)
    );
    assert!(boards.create_board());
    assert_eq!(
        boards.active_board().spec.grid.kind,
        BoardGridKind::IsometricDots
    );
    assert_eq!(boards.to_config().items[1].grid.spacing, 20);
}

#[test]
#[cfg(feature = "config-schema")]
fn board_grid_schema_exposes_supported_patterns_and_spacing_bounds() {
    let schema = Config::json_schema();
    let grid = &schema["$defs"]["BoardGridConfig"];
    assert_eq!(grid["properties"]["spacing"]["minimum"], 8);
    assert_eq!(grid["properties"]["spacing"]["maximum"], 200);
    assert_eq!(
        schema["$defs"]["BoardGridKindConfig"]["enum"],
        serde_json::json!(["none", "cartesian", "isometric", "isometric-dots"])
    );
}

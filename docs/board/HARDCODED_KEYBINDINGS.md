# Hardcoded Keybindings Limitation

## Current Status

Board mode keybindings are currently hardcoded in the source code and cannot be customized through the configuration file.

## Hardcoded Keybindings

The following keybindings are fixed in `src/input/state.rs`:

| Action | Keybinding | Location |
|--------|-----------|----------|
| Toggle Whiteboard | `Ctrl+W` | `src/input/state.rs:255` |
| Toggle Blackboard | `Ctrl+B` | `src/input/state.rs:260` |
| Return to Transparent | `Ctrl+Shift+T` | `src/input/state.rs:265` |

## Why Hardcoded?

The current keybinding system uses direct character matching with modifier flags:

```rust
'w' | 'W' if self.modifiers.ctrl && self.board_config.enabled => {
    log::info!("Ctrl+W pressed - toggling whiteboard mode");
    self.switch_board_mode(BoardMode::Whiteboard);
}
```

Making these configurable would require:

1. **Key parsing complexity**: Converting config strings (e.g., `"Ctrl+W"`) into keyboard codes
2. **Validation**: Ensuring no conflicts with existing drawing keybindings
3. **Platform differences**: Wayland/X11 key code variations
4. **Modifier handling**: Supporting combinations like Ctrl, Shift, Alt, Super

## Workaround

If you need different keybindings, you can:

1. **Modify the source code** directly in `src/input/state.rs`
2. **Use external tools** like keybinding managers to remap keys before they reach hyprmarker
3. **Submit a feature request** for configurable keybindings

## Conflict Analysis

The current hardcoded choices avoid conflicts:

- `Ctrl+W` / `Ctrl+B`: No conflict (plain W/B already used for White/Black colors)
- `Ctrl+Shift+T`: No conflict with existing shortcuts

## Future Implementation

To make board mode keybindings configurable, the following work would be needed:

1. Add keybinding config section:
   ```toml
   [board.keybindings]
   whiteboard = "Ctrl+W"
   blackboard = "Ctrl+B"
   transparent = "Ctrl+Shift+T"
   ```

2. Implement key parser that handles:
   - Modifier parsing (Ctrl, Shift, Alt, Super)
   - Character to key code mapping
   - Validation and conflict detection

3. Update input handling to use parsed keybindings instead of hardcoded matches

4. Add runtime validation to prevent overlaps with:
   - Drawing tool keys (F, L, R, A, E, T, M)
   - Color keys (R, G, B, Y, O, P, W, K)
   - Action keys (U, C, Q, F10)

## Related Files

- `src/input/state.rs`: Main input handling with hardcoded board keybindings
- `src/config/types.rs`: Configuration structures (where keybinding config would go)
- `docs/CONFIG.md`: User documentation for configuration options

## See Also

- [CONFIG.md](../CONFIG.md) - Full configuration documentation
- [README.md](../../README.md) - User guide with keybinding reference

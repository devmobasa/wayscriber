# Translation catalogs

One TOML file per language, named by its code: `zh-CN.toml` is Simplified
Chinese. Keys are the English source strings the UI draws, values are what the
language shows instead.

```toml
"Boards" = "画板"
"Saved {} of {}" = "已保存 {} / {}"
```

- Only values are translated. Never change a key; it is the lookup key.
- Keep the number of `{}` placeholders equal to the key's. Order may differ.
- An entry whose value equals its key is dead weight and fails the guard.
- The file is ordered by key, case-insensitively.

These files are checked, and missing entries are reported, by:

```sh
cargo test --test repository_guards translations
```

See `docs/I18N.md` for the whole workflow, and `src/i18n.rs` for how a catalog
is loaded. A copy of a catalog placed in
`$XDG_DATA_HOME/wayscriber/locales/<code>.toml` (or the directory
`WAYSCRIBER_LOCALE_DIR` names) merges over the embedded one, which is how a
translation is corrected without rebuilding.

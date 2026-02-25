# Visual and theme consistency

When changing colors, contrast, or any visual matter in the TUI:

- Apply the change to **all** predefined color themes (Default Dark, Nord, Dracula, Green Terminal, Monokai) in `src/tui/theme.rs`, not only the default.
- Ensure new themes or palette roles are defined in every theme.
- Check that body/dim/heading colors remain easy on the eyes (no pure white on dark backgrounds).

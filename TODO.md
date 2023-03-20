# TODO

- [x] Use effective serialize-deserialize routine for json::Value generation
- [x] Remove `Default` requirements from `config_it::Template`
- [x] Support for `config-it` crate renaming import (look for `proc-macro-crate`)
- [x] Customizable archive group representation other than `~(tilde)` prefix

- [ ] Special type support
  - `enum SpecialType` 
    - such as `FileSelect`, `DirSelect`, `ColorPick`, etc ...
    - `SpecialType` implements `trait Speical` ... verifies `#[config(special)]` flag validity
    - the dashboard customizes behavior by parsing given json as `SpecialType` when flag is set

# TODO-MONITOR

- [ ] Blueprint for monitoring UX/UI


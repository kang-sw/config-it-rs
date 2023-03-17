## 0.?
- [ ] JsonSchema generation support (using [schemars](https://crates.io/crates/schemars))

## 0.5.0
- [x] Deprecate `check_elem_update`, new API which have clearer name
- [x] New docs

## 0.4.2
- [x] Fix import routine of `Storage`

## 0.4.1
- [x] Default rmp_serde serialization policy update -> struct_as_tuple

## 0.4.0

**BRAKING CHANGES**
- [x] Change `Group::check_update` to require mutability.
- [x] Add new method: `Group::watch_update_with_event_broadcast`

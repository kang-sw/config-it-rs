# Concept

- The `Source` mandates all authorities of configurations to `Relay`
  - There are 3 levels of access: `View` / `Modify` / `Admin`
  - Each levels are assigned to each storage, with own key string
  - Only admin can browse the debug trace output of the program
- `Relay` requires apikey for `Source` registration
- `Source` registers itself to `Relay` during runtime
  - `Source` registers its path, such as `a/b.c/d`
  - `Relay` set `Source Rule`, which are applied to path pattern (simple GLOB)
  - `Source Rule` defines access level
    - Rule `MyRule` has `Admin` access to `Sources` with path `my_site/**`
    - `Source Rule` can be assigned to each `Root User`
    - `User` may assigned to another `Root User`
- Each path for `Source` is unique and consistent
  - Every log trace will be recorded
  - Any edition / access will be recorded

## `Source <-> Relay`

## `Relay <-> Client`

- Login -> Get session token
- Get list of accessible `Source`s
- Get information of given `Source`
- Open `Configs` stream to `Source`
- Open `Tracing` stream to `Source`
- Send commit to `Source` path

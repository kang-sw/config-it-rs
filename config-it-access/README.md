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

- `POST /api/s/register/<path>`
  - Returns HTTP stream of `Modification Request`
    - modification id, path, new value
  - As long as the stream is alive, the session will be retained.
  - Every server-issued commands will be received from given stream input.
- `[POST|PATCH|DELETE] /api/s/prop/<path>`
  - `POST` - Uploads single `PropDesc`
    - path, access level, description (markdown), default value, constraints, editor, ...
  - `PATCH` - Notifies update on property, with `PropUpdate`
    - path, [optional] modification id, [optional] new value or error
  - `DELETE` - Deletes single property, with path.
- `POST /api/s/log/<path>`
  - Flushes log message in internal trace format:
    - Span-New: span id, parent span id, values
      - unknown parent will be auto-generated
    - Span-Delete: span id
    - Event: belonging span id, content

## `Relay <-> Client`

- Login -> Get session token
- Get list of accessible `Source`s
- Get information of given `Source`
- Open `Configs` stream to `Source`
- Open `Tracing` stream to `Source`
- Send commit to `Source` path

# Vizia source pin and local resize fixes

Source: https://github.com/robbert-vdh/vizia
Revision: e3fab5530cda9cb90f679508d9f058bd62189d36 (patched-2024-05-06).
License: MIT; license files retained in vizia/.

Local changes:
- vizia_core/context/event.rs: set_user_scale_factor immediately updates effective
  renderer DPI (system scale × user zoom), then requests reflow/re-layout.
- vizia_baseview/application.rs: a uniform resize updates DPI, canvas dimensions,
  canonical base window size and geometry notification in the same frame.

NIH-plug excludes the nested vendor workspace and nih_plug_vizia uses a path
dependency here, so rebuilds do not rely on an edited Cargo download cache.

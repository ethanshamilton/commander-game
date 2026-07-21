# Mission Screen

The mission screen is the application mode where the tactical simulation is played. It owns the screen lifecycle and UI shell around the live battlefield.

The screen is composed from focused internal modules for the HUD, menu, plan panel, selected-unit panel, AI diagnostics, and performance overlay. Mission and soldier instantiation live outside the screen layer; the screen only schedules setup and teardown.

Selected-unit UI updates are split by invalidation domain: core information follows relevant unit/resource changes, debug chrome follows panel state, and trace/belief text is formatted only while its section is open. Text and layout components are written only when their displayed value actually changes.

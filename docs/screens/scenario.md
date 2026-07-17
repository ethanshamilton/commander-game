# Scenario Screen

The scenario screen is the application mode where the tactical simulation is played. It owns the screen lifecycle and UI shell around the live battlefield.

Selected-unit UI updates are split by invalidation domain: core information follows relevant unit/resource changes, debug chrome follows panel state, and trace/belief text is formatted only while its section is open. Text and layout components are written only when their displayed value actually changes.

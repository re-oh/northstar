---
trigger: always_on
---

### Logging Standards (Loggie)
* **Primary Logger:** exclusively use the `Loggie` global singleton for all debug output. Never use `print()` for game logic.
* **Domain Management:**
    1.  Before logging, ALWAYS read `res://debug/debug.gd` to identify valid logging domains.
    2.  domains are defined in the variable `_loggie_domains`.
    3.  If no relevant domain exists, you must explicitly propose adding a new domain for the log statement.

* **Syntax:** Use `Loggie.debug(domain, message)` for verbose data and `Loggie.error(domain, message)` for critical failures.

In general try and log as much as possible but only what is really needed.

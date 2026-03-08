---
category: fixed
changeset-operations: major
---
Breaking: `Git2Provider::new()` now takes a `project_root` argument and returns `Result`. The provider validates all operations against this root and reuses the repository handle across calls instead of reopening it.

---
title: Troubleshooting
nav_order: 9
---

# Troubleshooting

- apistos panics with `path name regex`: run `cargo update -p regex`.
- A route answers 500 `INTERNAL_ERROR` without details: the message is logged with the [`log`](https://docs.rs/log) crate, install a logger such as `env_logger::init()` to see it.
- sqlx fails to compile with `strip`, see [#5](https://github.com/ctaque/octopux/issues/5).

Something else? [Open an issue](https://github.com/ctaque/octopux/issues).

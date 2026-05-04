# CVVC.  Git, but in Rust

This project started as a Rust exercise, to reproduce [Write Yourself A Git](https://wyag.thb.lt/) but in Rust.  It's since grown well beyond that.

The current level of functionality is documented in the [`Functionality.md`](docs/Functionality.md) file.  Any behaviour which differs from that should be considered a bug.

The goal of this project is to write an alternative to the git CLI, making my own personal workflow smoother by, for example, making the default options of commands the ones I use in my own day-to-day.  The corollary to this is that functionality I don't or rarely use is very low priority for implementation.

One minor goal was that the name of the CLI command should be shorter and/or easier to type than `git`, which is why the CLI tool built by the CVVC binary crate is called `cv`.

This project should be compatible with a local on-disk Git repository, as long as that repository only uses the file versions and Git extensions documented as supported in `Functionality.md`.  If the Git CLI fails to read a repository that has been touched by `cv`, that should also be considered a bug, and a severe one at that.

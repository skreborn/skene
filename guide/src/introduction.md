# Introduction

Skene builds client-side apps in Rust for the web and for the terminal. A web page
renders on the server, reads and navigates with JavaScript off, and hydrates in place
once its bundle arrives. A terminal app behaves as a browser does wherever the terminal
can express it, and asks the terminal for every capability beyond a plain emulator,
using it only once the terminal's reply proves it.

Each platform is a crate of its own, with a vocabulary typed to exactly that platform:
`skene-gui` measures in logical pixels, `skene-tui` in cells, and what a platform
cannot draw does not exist to call. What every platform shares with identical semantics
lives once beneath them, in `skene-foundation`: the scene, event dispatch, the controls'
behaviors and their keys, accessibility, the style model, design tokens, motion and
layout. So an app is a crate of logic and state with a shell per platform, and a
component library is a core of colors, sizes and logic with a crate per platform.

The chapters build an app from `skene new` up: views and their layout, state, styling
and themes, controls, what the web and the terminal each add, tests, and a component
library of your own. Every example in this guide is compiled against the current
crates. The API reference is each crate's rustdoc, and `DESIGN.md` in the repository
describes the architecture.

# Labyrinth

Labyrinth is a virtual version of [the classic labyrinth marble game](https://en.wikipedia.org/wiki/Labyrinth_(marble_game)) written in Rust.

[![Labyrinth gif](labyrinth.gif)](https://youtu.be/EFMEzvK4WF0)

## Build

Just [install the latest version of Rust](https://www.rust-lang.org/tools/install) and do

```
cargo build --release
```

in the project main directory.

## Modules

### `game`

Implements the core game logic and physics. Takes no stance on how the game is presented or how user input is given. Note that even though Labyrinth is a 3D game, the physics in `game` module are 2D.

### `graphics`

Implements a scene graph based 3D graphics engine using the [wgpu-rs](https://github.com/gfx-rs/wgpu-rs) library. Not specific to Labyrinth, but could in principle be used for other purposes also.

### `util`

"Everything else": the main game loop, user input, creating textures, animation and helper functions.

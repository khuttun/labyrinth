# Labyrinth

Labyrinth is a virtual version of [the classic labyrinth marble game](https://en.wikipedia.org/wiki/Labyrinth_(marble_game)) written in Rust.

[![Labyrinth gif](labyrinth.gif)](https://youtu.be/EFMEzvK4WF0)

## Build

First, compile the shaders by installing [glslangValidator](https://www.khronos.org/opengles/sdk/tools/Reference-Compiler/), and running

```
glslangValidator -V -o src/default.frag.spv -e main src/default.frag
glslangValidator -V -o src/default.vert.spv -e main src/default.vert
glslangValidator -V -o src/shadow.vert.spv -e main src/shadow.vert
```

in the project main directory.

Then, [install Rust](https://www.rust-lang.org/tools/install), and do

```
cargo build --release
```

in the project main directory.

## Modules

### `game`

Implements the core game logic and physics. Takes no stance on how the game is presented or how user input is given. Note that even though Labyrinth is a 3D game, the physics in `game` module are 2D.

### `graphics`

Implements a scene graph based 3D graphics engine using the [wgpu-rs](https://github.com/gfx-rs/wgpu-rs) library. Not specific to Labyrinth, could in principle be used for other purposes also.

Everything else (the main game loop, user input, creating textures, animation, helper functions...) is implemented in lib.rs.

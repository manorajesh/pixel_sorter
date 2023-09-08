# pixel_sorter
![Build Status](https://github.com/manorajesh/mandelbrot-rs/actions/workflows/MacOS.yml/badge.svg)
![Build Status](https://github.com/manorajesh/mandelbrot-rs/actions/workflows/Linux.yml/badge.svg)
![Build Status](https://github.com/manorajesh/mandelbrot-rs/actions/workflows/Windows.yml/badge.svg)

Rustily interact with the mandelbrot set *__with the GPU!__*

![Demo Picture](https://github.com/manorajesh/mandelbrot-rs/blob/wgpu/images/demo1.png)

## Installation
```shell
git clone https://github.com/manorajesh/mandelbrot-rs.git && cd mandelbrot-rs
cargo run --release
```

## Usage
At startup, the mouse is captured and can be used to translate the viewport with the scroll whell being used to, you guessed it, zoom.
Press the spacebar to toggle mouse captivity. Use the `up` and `down` arrow keys to increase and decrease the iteration count
(i.e. detail) of the mandelbrot respectively. If you want to leave, press `esc`.

## Why
Idea popped into my mind during a creative lull. Seemed easy enough while also pushing me along my graphics journey.

#### Important Code
The `fs_main` fragment shader in `shader.wgsl` is what colors each pixel. Those familiar with [this](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set) Wikipedia page will recognize the method.

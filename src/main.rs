use pixel_sorter::run;
use pollster;

fn main() {
    pollster::block_on(run());
}
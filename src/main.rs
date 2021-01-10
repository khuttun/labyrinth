fn main() {
    #[cfg(not(target_os = "android"))]
    labyrinth::start_app();
}

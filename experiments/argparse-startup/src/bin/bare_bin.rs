fn main() {
    let n = std::env::args_os().skip(1).count();
    std::process::exit(if n == 0 { 2 } else { 0 });
}

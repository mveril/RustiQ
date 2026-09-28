pub(crate) fn print(source: &str) {
    print!("{source}");
    if !source.ends_with('\n') {
        println!();
    }
    println!();
}

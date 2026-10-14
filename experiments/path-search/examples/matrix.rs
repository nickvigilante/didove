//! Prints the edge-case matrix as Markdown.

fn main() {
    for row in path_search::scenarios::run_all() {
        println!("### {}", row.name);
        println!("- walk : {:?}", row.walk);
        for p in &row.walk_probes {
            println!(
                "    #{} entry={:?} -> {:?}{}",
                p.index,
                p.entry,
                p.outcome,
                p.same_file_as
                    .map(|i| format!(" (same file as #{i})"))
                    .unwrap_or_default()
            );
        }
        println!("- which: {:?}", row.which);
        println!("- naive: {:?}", row.naive);
        if !row.which_errors.is_empty() {
            println!(
                "    nonfatal errors (no path attached): {:?}",
                row.which_errors
            );
        }
    }
}

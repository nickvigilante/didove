mod common;

use common::Fx;
use didove::chain::{End, resolve};

#[test]
fn a_regular_file_has_no_hops() {
    let fx = Fx::new();
    let f = fx.exe("bin/tool");
    let c = resolve(&f);
    assert!(c.hops.is_empty());
    assert_eq!(
        (c.end, c.real_path.as_deref()),
        (End::File, Some(f.as_path()))
    );
}

#[test]
fn relative_then_absolute_hops() {
    let fx = Fx::new();
    let real = fx.exe("cellar/tool-1.0/bin/tool");
    fx.link("opt/tool", &real); // absolute target
    fx.link("bin/tool", "../opt/tool"); // relative target
    let c = resolve(&fx.p("bin/tool"));
    assert_eq!(c.end, End::File);
    assert_eq!(c.real_path.as_deref(), Some(real.as_path()));
    let hops: Vec<_> = c
        .hops
        .iter()
        .map(|h| {
            (
                h.path.clone(),
                h.target.clone(),
                h.resolved.clone(),
                h.broken,
            )
        })
        .collect();
    assert_eq!(
        hops,
        [
            (
                fx.p("bin/tool"),
                "../opt/tool".into(),
                fx.p("opt/tool"),
                false
            ),
            (fx.p("opt/tool"), real.clone(), real, false),
        ]
    );
}

#[test]
fn a_symlinked_directory_is_resolved_in_real_path_but_is_not_a_hop() {
    let fx = Fx::new();
    let real = fx.exe("usr/bin/tool");
    fx.link("bin", "usr/bin");
    let c = resolve(&fx.p("bin/tool"));
    assert!(c.hops.is_empty());
    assert_eq!(c.real_path.as_deref(), Some(real.as_path()));
}

#[test]
fn broken_link_is_marked_and_keeps_where_it_was_heading() {
    let fx = Fx::new();
    fx.link("bin/tool", "../gone/tool");
    let start = fx.p("bin/tool");
    let c = resolve(&start);
    assert_eq!((c.end, c.real_path.clone()), (End::Broken, None));
    assert_eq!(c.hops.len(), 1);
    assert!(c.hops[0].broken);
    assert_eq!(
        c.final_path(&start),
        fx.p("gone/tool"),
        "provenance can still read the target"
    );
}

#[test]
fn chain_that_breaks_on_the_second_hop() {
    let fx = Fx::new();
    fx.link("opt/tool", "nowhere");
    fx.link("bin/tool", fx.p("opt/tool"));
    let c = resolve(&fx.p("bin/tool"));
    assert_eq!(c.end, End::Broken);
    assert_eq!(
        c.hops.iter().map(|h| h.broken).collect::<Vec<_>>(),
        [false, true]
    );
}

#[test]
fn a_cycle_is_reported_not_followed_forever() {
    let fx = Fx::new();
    fx.link("a/tool", fx.p("b/tool"));
    fx.link("b/tool", fx.p("a/tool"));
    let c = resolve(&fx.p("a/tool"));
    assert_eq!((c.end, c.real_path), (End::Loop, None));
    assert_eq!(c.hops.len(), 40);
}

#[test]
fn a_missing_start_is_broken_with_no_hops() {
    let fx = Fx::new();
    let c = resolve(&fx.p("nothing"));
    assert_eq!((c.end, c.hops.len()), (End::Broken, 0));
}

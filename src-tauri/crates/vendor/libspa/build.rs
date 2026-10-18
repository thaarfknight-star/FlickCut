fn main() {
    // FlickCut patch: the original build.rs also compiled tests/pod.c, which
    // is only used by libspa's own test suite (and isn't vendored). The
    // system dependency probe below is all downstream crates need.
    system_deps::Config::new()
        .probe()
        .expect("Cannot find libspa");
}
